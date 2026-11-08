/*
 * mod_tessera - Apache httpd 2.4 content handler for Tessera web applications
 *               built as shared libraries (`tessera1 build app.tes --library`).
 *
 * Configuration:
 *   LoadModule tessera_module modules/mod_tessera.so
 *   TesseraEnv TESSERA_TIME_LIMIT_MS 5000        # server-wide, set at child init
 *   <Location /app>
 *       SetHandler tessera
 *       TesseraApplication /srv/app/libapp.so    # server or directory scope
 *       TesseraMaxBody 10485760                  # bytes, default 10 MiB -> 413
 *   </Location>
 *
 * Libraries are opened with dlopen in every child process at child_init (so a
 * graceful restart picks up a replaced .so) after the TesseraEnv variables are
 * set. The ABI version is checked; failures are written to the error log and
 * requests for that application get a 500.
 *
 * tessera_handle blocks until the application returns. That is fine in
 * Apache (prefork, worker and event MPMs all run handlers on a dedicated
 * thread/process); every call runs the app on its own thread in the library.
 *
 * Build (with apxs):  apxs -c -I../include mod_tessera.c
 *   or:  gcc -shared -fPIC -O2 -I../include -I<apache2 include> -I<apr include> \
 *            -o mod_tessera.so mod_tessera.c -ldl
 */
#define TESSERA_NO_PROTOTYPES
#include "tessera.h"

#include "httpd.h"
#include "http_config.h"
#include "http_core.h"
#include "http_log.h"
#include "http_protocol.h"
#include "http_request.h"
#include "ap_config.h"
#include "apr_strings.h"
#include "apr_tables.h"
#include "apr_uri.h"
#include "util_filter.h"

#include <dlfcn.h>
#include <stdlib.h>

#define TESSERA_DEFAULT_MAX_BODY ((apr_off_t)10 * 1024 * 1024)

module AP_MODULE_DECLARE_DATA tessera_module;

#ifdef APLOG_USE_MODULE
APLOG_USE_MODULE(tessera);
#endif

/* One application library. Allocated from pconf at configuration time and
 * filled in at child_init (each child has its own copy after fork). */
typedef struct {
    const char *path;
    void *dl;
    tessera_handle_fn handle;
    tessera_free_fn free_response;
    const char *error; /* why it could not be loaded, NULL if fine */
} tessera_lib;

typedef struct {
    tessera_lib *lib;  /* NULL = not set */
    apr_off_t max_body; /* -1 = not set */
} tessera_dir_conf;

/* Registry of all libraries named in the configuration and of TesseraEnv
 * variables. Both live in pconf and are reset when pconf is cleared (graceful
 * restart re-reads the configuration). */
static apr_array_header_t *tessera_libs = NULL;  /* of tessera_lib* */
static apr_array_header_t *tessera_env = NULL;   /* of const char* pairs */

static apr_status_t tessera_reset_registry(void *unused)
{
    (void)unused;
    tessera_libs = NULL;
    tessera_env = NULL;
    return APR_SUCCESS;
}

static void tessera_ensure_registry(apr_pool_t *pconf)
{
    if (tessera_libs == NULL) {
        tessera_libs = apr_array_make(pconf, 4, sizeof(tessera_lib *));
        tessera_env = apr_array_make(pconf, 8, sizeof(const char *));
        apr_pool_cleanup_register(pconf, NULL, tessera_reset_registry, apr_pool_cleanup_null);
    }
}

static void *tessera_create_dir_conf(apr_pool_t *p, char *dir)
{
    tessera_dir_conf *c = apr_pcalloc(p, sizeof *c);
    (void)dir;
    c->lib = NULL;
    c->max_body = -1;
    return c;
}

static void *tessera_merge_dir_conf(apr_pool_t *p, void *basev, void *addv)
{
    tessera_dir_conf *base = basev, *add = addv;
    tessera_dir_conf *c = apr_pcalloc(p, sizeof *c);
    c->lib = add->lib ? add->lib : base->lib;
    c->max_body = add->max_body >= 0 ? add->max_body : base->max_body;
    return c;
}

/* ---- directives ---- */

static const char *cmd_application(cmd_parms *cmd, void *dconf, const char *path)
{
    tessera_dir_conf *c = dconf;
    int i;
    tessera_ensure_registry(cmd->pool);
    path = ap_server_root_relative(cmd->pool, path);
    if (path == NULL) {
        return "TesseraApplication: invalid path";
    }
    for (i = 0; i < tessera_libs->nelts; i++) {
        tessera_lib *l = APR_ARRAY_IDX(tessera_libs, i, tessera_lib *);
        if (strcmp(l->path, path) == 0) {
            c->lib = l;
            return NULL;
        }
    }
    c->lib = apr_pcalloc(cmd->pool, sizeof(tessera_lib));
    c->lib->path = path;
    APR_ARRAY_PUSH(tessera_libs, tessera_lib *) = c->lib;
    return NULL;
}

static const char *cmd_env(cmd_parms *cmd, void *dconf, const char *name, const char *value)
{
    const char *err = ap_check_cmd_context(cmd, GLOBAL_ONLY);
    (void)dconf;
    if (err != NULL) {
        return err;
    }
    if (*name == '\0' || strchr(name, '=') != NULL) {
        return "TesseraEnv: invalid variable name";
    }
    tessera_ensure_registry(cmd->pool);
    APR_ARRAY_PUSH(tessera_env, const char *) = apr_pstrdup(cmd->pool, name);
    APR_ARRAY_PUSH(tessera_env, const char *) = apr_pstrdup(cmd->pool, value);
    return NULL;
}

static const char *cmd_max_body(cmd_parms *cmd, void *dconf, const char *arg)
{
    tessera_dir_conf *c = dconf;
    char *end;
    apr_off_t v;
    (void)cmd;
    if (apr_strtoff(&v, arg, &end, 10) != APR_SUCCESS || *end != '\0' || v < 0) {
        return "TesseraMaxBody: expected a number of bytes";
    }
    c->max_body = v;
    return NULL;
}

static const command_rec tessera_cmds[] = {
    AP_INIT_TAKE1("TesseraApplication", cmd_application, NULL, RSRC_CONF | ACCESS_CONF,
                  "Path of the Tessera application shared library (libapp.so)"),
    AP_INIT_TAKE2("TesseraEnv", cmd_env, NULL, RSRC_CONF,
                  "Environment variable for Tessera applications: NAME VALUE"),
    AP_INIT_TAKE1("TesseraMaxBody", cmd_max_body, NULL, RSRC_CONF | ACCESS_CONF,
                  "Maximum request body size in bytes (default 10485760)"),
    {NULL}
};

/* ---- library loading ---- */

static int tessera_post_config(apr_pool_t *pconf, apr_pool_t *plog, apr_pool_t *ptemp, server_rec *s)
{
    int i;
    (void)pconf;
    (void)plog;
    (void)ptemp;
    if (tessera_libs == NULL) {
        return OK;
    }
    /* Early, friendly check in the parent; the real dlopen happens per child. */
    for (i = 0; i < tessera_libs->nelts; i++) {
        tessera_lib *l = APR_ARRAY_IDX(tessera_libs, i, tessera_lib *);
        apr_finfo_t fi;
        if (apr_stat(&fi, l->path, APR_FINFO_TYPE, ptemp) != APR_SUCCESS || fi.filetype != APR_REG) {
            ap_log_error(APLOG_MARK, APLOG_WARNING, 0, s,
                         "tessera: application library %s does not exist (yet)", l->path);
        }
    }
    return OK;
}

static void tessera_load(tessera_lib *l, apr_pool_t *p, server_rec *s)
{
    tessera_abi_version_fn version;
    int32_t v;
    const char *err;

    l->dl = dlopen(l->path, RTLD_NOW | RTLD_LOCAL);
    if (l->dl == NULL) {
        err = dlerror();
        l->error = apr_psprintf(p, "dlopen failed: %s", err ? err : "unknown error");
        goto fail;
    }
    version = (tessera_abi_version_fn)dlsym(l->dl, "tessera_abi_version");
    l->handle = (tessera_handle_fn)dlsym(l->dl, "tessera_handle");
    l->free_response = (tessera_free_fn)dlsym(l->dl, "tessera_free");
    if (version == NULL || l->handle == NULL || l->free_response == NULL) {
        l->error = "not a Tessera application library (missing tessera_* symbols)";
        goto fail;
    }
    v = version();
    if (v != TESSERA_ABI_VERSION) {
        l->error = apr_psprintf(p, "ABI version %d, this module supports %d", (int)v, TESSERA_ABI_VERSION);
        goto fail;
    }
    return;

fail:
    l->handle = NULL;
    l->free_response = NULL;
    /* Keep a failed library mapped (no dlclose): unloading Rust libraries is
     * not reliable and the process is short-lived relative to config. */
    ap_log_error(APLOG_MARK, APLOG_ERR, 0, s, "tessera: cannot load %s: %s", l->path, l->error);
}

static void tessera_child_init(apr_pool_t *pchild, server_rec *s)
{
    int i;
    if (tessera_libs == NULL) {
        return;
    }
    /* Workers have not started yet, so setenv is safe here. */
    for (i = 0; i + 1 < tessera_env->nelts; i += 2) {
        setenv(APR_ARRAY_IDX(tessera_env, i, const char *),
               APR_ARRAY_IDX(tessera_env, i + 1, const char *), 1);
    }
    for (i = 0; i < tessera_libs->nelts; i++) {
        tessera_load(APR_ARRAY_IDX(tessera_libs, i, tessera_lib *), pchild, s);
    }
}

/* ---- request handling ---- */

typedef struct {
    size_t size;
    int count;
} header_size_acc;

static int header_size_cb(void *rec, const char *key, const char *value)
{
    header_size_acc *acc = rec;
    acc->size += tessera_field_size(strlen(key)) + tessera_field_size(strlen(value));
    acc->count++;
    return 1;
}

static int header_write_cb(void *rec, const char *key, const char *value)
{
    uint8_t **p = rec;
    *p = tessera_put_lower(*p, key, strlen(key));
    *p = tessera_put_bytes(*p, value, strlen(value));
    return 1;
}

/* Reads the whole request body into pool memory. Returns OK or an HTTP status. */
static int read_body(request_rec *r, apr_off_t max_body, char **out, apr_size_t *out_len)
{
    int rc;
    apr_size_t len = 0, cap = 0;
    char *buf = NULL;

    *out = NULL;
    *out_len = 0;
    rc = ap_setup_client_block(r, REQUEST_CHUNKED_DECHUNK);
    if (rc != OK) {
        return rc;
    }
    if (r->remaining > max_body) {
        ap_log_rerror(APLOG_MARK, APLOG_INFO, 0, r, "tessera: request body of %" APR_OFF_T_FMT
                      " bytes exceeds TesseraMaxBody %" APR_OFF_T_FMT, r->remaining, max_body);
        return HTTP_REQUEST_ENTITY_TOO_LARGE;
    }
    if (!ap_should_client_block(r)) {
        return OK;
    }
    /* +1 so a body of exactly Content-Length bytes never triggers a grow. */
    cap = r->remaining > 0 ? (apr_size_t)r->remaining + 1 : 8192;
    buf = apr_palloc(r->pool, cap);
    for (;;) {
        long n;
        if (len == cap) {
            char *bigger;
            if ((apr_off_t)cap > max_body) {
                return HTTP_REQUEST_ENTITY_TOO_LARGE;
            }
            bigger = apr_palloc(r->pool, cap * 2);
            memcpy(bigger, buf, len);
            buf = bigger;
            cap *= 2;
        }
        n = ap_get_client_block(r, buf + len, cap - len);
        if (n == 0) {
            break;
        }
        if (n < 0) {
            ap_log_rerror(APLOG_MARK, APLOG_INFO, 0, r, "tessera: error reading request body");
            return HTTP_BAD_REQUEST;
        }
        len += (apr_size_t)n;
        if ((apr_off_t)len > max_body) {
            return HTTP_REQUEST_ENTITY_TOO_LARGE;
        }
    }
    *out = buf;
    *out_len = len;
    return OK;
}

static int is_hop_by_hop(const uint8_t *name, size_t n)
{
    static const char *const skip[] = {"content-length", "transfer-encoding", "connection",
                                       "keep-alive", "upgrade", "status", NULL};
    int i;
    for (i = 0; skip[i] != NULL; i++) {
        if (strlen(skip[i]) == n && strncasecmp(skip[i], (const char *)name, n) == 0) {
            return 1;
        }
    }
    return 0;
}

/* Raw (still percent-encoded) path, as the application expects. Note that
 * r->parsed_uri.path cannot be used: the core unescapes r->uri in place and
 * r->uri shares that buffer. So take it from the request line instead. */
static const char *raw_path(request_rec *r)
{
    const char *u = r->unparsed_uri;
    apr_uri_t parsed;
    if (u == NULL || *u == '\0') {
        return "/";
    }
    if (*u == '/') {
        return apr_pstrndup(r->pool, u, strcspn(u, "?#"));
    }
    /* absolute-form ("http://host/path?q") or "*" */
    if (strcmp(u, "*") != 0 && apr_uri_parse(r->pool, u, &parsed) == APR_SUCCESS && parsed.path != NULL) {
        return parsed.path;
    }
    return strcmp(u, "*") == 0 ? "*" : "/";
}

static int tessera_handler(request_rec *r)
{
    tessera_dir_conf *conf;
    tessera_lib *lib;
    apr_off_t max_body;
    char *body;
    apr_size_t body_len;
    const char *path, *query, *remote;
    header_size_acc acc = {0, 0};
    size_t req_len;
    uint8_t *req, *p, *resp = NULL;
    size_t resp_len = 0;
    int rc;
    tessera_reader rd;
    uint32_t version, status, count, i;
    size_t out_body_len;
    const uint8_t *out_body;
    apr_bucket_brigade *bb;

    if (r->handler == NULL || strcmp(r->handler, "tessera") != 0) {
        return DECLINED;
    }
    conf = ap_get_module_config(r->per_dir_config, &tessera_module);
    lib = conf->lib;
    if (lib == NULL) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r, "tessera: SetHandler tessera without TesseraApplication");
        return HTTP_INTERNAL_SERVER_ERROR;
    }
    if (lib->handle == NULL) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r, "tessera: application %s is not loaded: %s", lib->path,
                      lib->error ? lib->error : "not initialised");
        return HTTP_INTERNAL_SERVER_ERROR;
    }
    max_body = conf->max_body >= 0 ? conf->max_body : TESSERA_DEFAULT_MAX_BODY;
    if (max_body > 0xFFFFFF00) {
        max_body = 0xFFFFFF00; /* wire format uses u32 lengths */
    }

    rc = read_body(r, max_body, &body, &body_len);
    if (rc != OK) {
        return rc;
    }

    path = raw_path(r);
    query = r->args ? r->args : "";
    remote = r->useragent_ip ? r->useragent_ip : (r->connection->client_ip ? r->connection->client_ip : "");

    apr_table_do(header_size_cb, &acc, r->headers_in, NULL);
    req_len = 4 + tessera_field_size(strlen(r->method)) + tessera_field_size(strlen(path)) +
              tessera_field_size(strlen(query)) + tessera_field_size(strlen(remote)) + 4 + acc.size +
              tessera_field_size(body_len);
    req = apr_palloc(r->pool, req_len);
    p = tessera_put_u32(req, TESSERA_WIRE_VERSION);
    p = tessera_put_bytes(p, r->method, strlen(r->method));
    p = tessera_put_bytes(p, path, strlen(path));
    p = tessera_put_bytes(p, query, strlen(query));
    p = tessera_put_bytes(p, remote, strlen(remote));
    p = tessera_put_u32(p, (uint32_t)acc.count);
    apr_table_do(header_write_cb, &p, r->headers_in, NULL);
    p = tessera_put_bytes(p, body, body_len);
    if ((size_t)(p - req) != req_len) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r, "tessera: internal error building request");
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    rc = lib->handle(req, req_len, &resp, &resp_len);
    if (rc != TESSERA_OK) {
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r, "tessera: tessera_handle failed with code %d", rc);
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    tessera_reader_init(&rd, resp, resp_len);
    version = tessera_read_u32(&rd);
    status = tessera_read_u32(&rd);
    count = tessera_read_u32(&rd);
    if (!rd.ok || version != TESSERA_WIRE_VERSION || status < 100 || status > 999) {
        lib->free_response(resp, resp_len);
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r, "tessera: malformed response from application");
        return HTTP_INTERNAL_SERVER_ERROR;
    }
    for (i = 0; rd.ok && i < count; i++) {
        size_t nl, vl;
        const uint8_t *n = tessera_read_bytes(&rd, &nl);
        const uint8_t *v = tessera_read_bytes(&rd, &vl);
        if (!rd.ok) {
            break;
        }
        if (nl == 12 && strncasecmp((const char *)n, "content-type", 12) == 0) {
            ap_set_content_type(r, apr_pstrmemdup(r->pool, (const char *)v, vl));
        }
        else if (!is_hop_by_hop(n, nl)) {
            apr_table_add(r->headers_out, apr_pstrmemdup(r->pool, (const char *)n, nl),
                          apr_pstrmemdup(r->pool, (const char *)v, vl));
        }
    }
    out_body = tessera_read_bytes(&rd, &out_body_len);
    if (!rd.ok) {
        lib->free_response(resp, resp_len);
        ap_log_rerror(APLOG_MARK, APLOG_ERR, 0, r, "tessera: malformed response from application");
        return HTTP_INTERNAL_SERVER_ERROR;
    }

    r->status = (int)status;
    ap_set_content_length(r, (apr_off_t)out_body_len);

    /* Transient bucket: filters that keep the data past ap_pass_brigade must
     * copy it (bucket setaside), so the buffer can be freed right after. */
    bb = apr_brigade_create(r->pool, r->connection->bucket_alloc);
    if (out_body_len > 0 && !r->header_only) {
        APR_BRIGADE_INSERT_TAIL(bb, apr_bucket_transient_create((const char *)out_body, out_body_len,
                                                                 r->connection->bucket_alloc));
    }
    APR_BRIGADE_INSERT_TAIL(bb, apr_bucket_eos_create(r->connection->bucket_alloc));
    rc = ap_pass_brigade(r->output_filters, bb);
    apr_brigade_cleanup(bb);
    lib->free_response(resp, resp_len);
    if (rc != APR_SUCCESS) {
        ap_log_rerror(APLOG_MARK, APLOG_DEBUG, rc, r, "tessera: writing response failed");
        return AP_FILTER_ERROR;
    }
    return OK;
}

static void tessera_register_hooks(apr_pool_t *p)
{
    (void)p;
    ap_hook_post_config(tessera_post_config, NULL, NULL, APR_HOOK_MIDDLE);
    ap_hook_child_init(tessera_child_init, NULL, NULL, APR_HOOK_MIDDLE);
    ap_hook_handler(tessera_handler, NULL, NULL, APR_HOOK_MIDDLE);
}

module AP_MODULE_DECLARE_DATA tessera_module = {
    STANDARD20_MODULE_STUFF,
    tessera_create_dir_conf, /* create per-dir config */
    tessera_merge_dir_conf,  /* merge per-dir config */
    NULL,                    /* create per-server config */
    NULL,                    /* merge per-server config */
    tessera_cmds,            /* directives */
    tessera_register_hooks   /* register hooks */
#ifdef AP_MODULE_FLAG_NONE
    , AP_MODULE_FLAG_NONE    /* flags (httpd >= 2.4.27) */
#endif
};
