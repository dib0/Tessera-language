/*
 * ngx_http_tessera_module - runs a Tessera web application built as a shared
 * library (`tessera1 build app.tes --library -o libapp.so`) inside nginx.
 *
 *   http {
 *       tessera_env TESSERA_TIME_LIMIT_MS 5000;   # http level, set at worker start
 *       thread_pool tessera threads=16;           # main level (optional)
 *       server {
 *           location /app/ {
 *               tessera /srv/app/libapp.so;
 *               tessera_max_body 10m;             # default 10m, larger -> 413
 *               client_max_body_size 10m;         # nginx's own limit also applies
 *               tessera_thread_pool tessera;      # default "default" (32 threads)
 *           }
 *       }
 *   }
 *
 * Build: ./configure --with-threads --add-module=/path/to/hosts/nginx
 *        (or --add-dynamic-module=... and load_module).
 *
 * The libraries are opened with dlopen in every worker at init_process, after
 * the tessera_env variables are set. A library with a wrong ABI version is
 * rejected and logged; its locations answer 500.
 *
 * tessera_handle blocks until the application has finished. When nginx is
 * built with thread support (--with-threads, NGX_THREADS) the call runs in a
 * thread pool task and the response is sent from the completion handler, so
 * the event loop is never blocked by the application. WITHOUT thread support
 * the call is made directly and blocks the whole worker process (all of its
 * connections) for the duration of each request; use enough worker_processes
 * or, better, build with --with-threads.
 *
 * Request bodies are read with ngx_http_read_client_request_body; bodies that
 * nginx buffered to a temporary file are read back from that file in the
 * event loop before the task is posted.
 */

#include <ngx_config.h>
#include <ngx_core.h>
#include <ngx_http.h>

#if (NGX_THREADS)
#include <ngx_thread_pool.h>
#endif

#define TESSERA_NO_PROTOTYPES
#include "tessera.h"

#include <dlfcn.h>
#include <stdlib.h>

#define TESSERA_DEFAULT_MAX_BODY (10 * 1024 * 1024)

typedef struct {
    ngx_str_t           path;          /* NUL-terminated */
    void               *dl;
    tessera_handle_fn   handle;
    tessera_free_fn     free_response;
    const char         *error;
} ngx_http_tessera_lib_t;

typedef struct {
    ngx_array_t         libs;          /* of ngx_http_tessera_lib_t * */
    ngx_array_t         env;           /* of ngx_keyval_t */
} ngx_http_tessera_main_conf_t;

typedef struct {
    ngx_http_tessera_lib_t  *lib;
    off_t                    max_body;
#if (NGX_THREADS)
    ngx_str_t                pool_name;
    ngx_thread_pool_t       *pool;
#endif
} ngx_http_tessera_loc_conf_t;

/* Per-request state; lives in r->pool. */
typedef struct {
    ngx_http_request_t      *request;
    ngx_http_tessera_lib_t  *lib;
    uint8_t                 *req;
    size_t                   req_len;
    uint8_t                 *resp;     /* owned by the library until freed */
    size_t                   resp_len;
    int32_t                  rc;
} ngx_http_tessera_ctx_t;

static char *ngx_http_tessera(ngx_conf_t *cf, ngx_command_t *cmd, void *conf);
static char *ngx_http_tessera_env(ngx_conf_t *cf, ngx_command_t *cmd, void *conf);
static char *ngx_http_tessera_thread_pool(ngx_conf_t *cf, ngx_command_t *cmd, void *conf);
static void *ngx_http_tessera_create_main_conf(ngx_conf_t *cf);
static void *ngx_http_tessera_create_loc_conf(ngx_conf_t *cf);
static char *ngx_http_tessera_merge_loc_conf(ngx_conf_t *cf, void *parent, void *child);
static ngx_int_t ngx_http_tessera_init_process(ngx_cycle_t *cycle);
static ngx_int_t ngx_http_tessera_handler(ngx_http_request_t *r);

static ngx_command_t ngx_http_tessera_commands[] = {

    { ngx_string("tessera"),
      NGX_HTTP_LOC_CONF | NGX_HTTP_LIF_CONF | NGX_CONF_TAKE1,
      ngx_http_tessera,
      NGX_HTTP_LOC_CONF_OFFSET,
      0,
      NULL },

    { ngx_string("tessera_env"),
      NGX_HTTP_MAIN_CONF | NGX_CONF_TAKE2,
      ngx_http_tessera_env,
      NGX_HTTP_MAIN_CONF_OFFSET,
      0,
      NULL },

    { ngx_string("tessera_max_body"),
      NGX_HTTP_MAIN_CONF | NGX_HTTP_SRV_CONF | NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1,
      ngx_conf_set_off_slot,
      NGX_HTTP_LOC_CONF_OFFSET,
      offsetof(ngx_http_tessera_loc_conf_t, max_body),
      NULL },

    { ngx_string("tessera_thread_pool"),
      NGX_HTTP_MAIN_CONF | NGX_HTTP_SRV_CONF | NGX_HTTP_LOC_CONF | NGX_CONF_TAKE1,
      ngx_http_tessera_thread_pool,
      NGX_HTTP_LOC_CONF_OFFSET,
      0,
      NULL },

      ngx_null_command
};

static ngx_http_module_t ngx_http_tessera_module_ctx = {
    NULL,                                  /* preconfiguration */
    NULL,                                  /* postconfiguration */
    ngx_http_tessera_create_main_conf,     /* create main configuration */
    NULL,                                  /* init main configuration */
    NULL,                                  /* create server configuration */
    NULL,                                  /* merge server configuration */
    ngx_http_tessera_create_loc_conf,      /* create location configuration */
    ngx_http_tessera_merge_loc_conf        /* merge location configuration */
};

ngx_module_t ngx_http_tessera_module = {
    NGX_MODULE_V1,
    &ngx_http_tessera_module_ctx,          /* module context */
    ngx_http_tessera_commands,             /* module directives */
    NGX_HTTP_MODULE,                       /* module type */
    NULL,                                  /* init master */
    NULL,                                  /* init module */
    ngx_http_tessera_init_process,         /* init process */
    NULL,                                  /* init thread */
    NULL,                                  /* exit thread */
    NULL,                                  /* exit process */
    NULL,                                  /* exit master */
    NGX_MODULE_V1_PADDING
};

/* ---- configuration ---- */

static void *
ngx_http_tessera_create_main_conf(ngx_conf_t *cf)
{
    ngx_http_tessera_main_conf_t  *mcf;

    mcf = ngx_pcalloc(cf->pool, sizeof(ngx_http_tessera_main_conf_t));
    if (mcf == NULL) {
        return NULL;
    }
    if (ngx_array_init(&mcf->libs, cf->pool, 4, sizeof(ngx_http_tessera_lib_t *)) != NGX_OK
        || ngx_array_init(&mcf->env, cf->pool, 4, sizeof(ngx_keyval_t)) != NGX_OK)
    {
        return NULL;
    }
    return mcf;
}

static void *
ngx_http_tessera_create_loc_conf(ngx_conf_t *cf)
{
    ngx_http_tessera_loc_conf_t  *lcf;

    lcf = ngx_pcalloc(cf->pool, sizeof(ngx_http_tessera_loc_conf_t));
    if (lcf == NULL) {
        return NULL;
    }
    lcf->max_body = NGX_CONF_UNSET;
    return lcf;
}

static char *
ngx_http_tessera_merge_loc_conf(ngx_conf_t *cf, void *parent, void *child)
{
    ngx_http_tessera_loc_conf_t  *prev = parent;
    ngx_http_tessera_loc_conf_t  *conf = child;

    ngx_conf_merge_off_value(conf->max_body, prev->max_body, TESSERA_DEFAULT_MAX_BODY);
    if (conf->max_body > 0xFFFFFF00) {
        conf->max_body = 0xFFFFFF00;   /* wire format uses u32 lengths */
    }

#if (NGX_THREADS)
    if (conf->pool_name.data == NULL) {
        conf->pool_name = prev->pool_name;
    }
    /* Resolve (and if needed create) the pool for locations that run an
     * application. "default" is created implicitly with 32 threads; any other
     * name must be declared with thread_pool at main level. */
    if (conf->lib != NULL) {
        conf->pool = ngx_thread_pool_add(cf, conf->pool_name.data ? &conf->pool_name : NULL);
        if (conf->pool == NULL) {
            return NGX_CONF_ERROR;
        }
    }
#endif

    return NGX_CONF_OK;
}

static char *
ngx_http_tessera(ngx_conf_t *cf, ngx_command_t *cmd, void *conf)
{
    ngx_http_tessera_loc_conf_t   *lcf = conf;
    ngx_http_tessera_main_conf_t  *mcf;
    ngx_http_core_loc_conf_t      *clcf;
    ngx_http_tessera_lib_t       **libp, *lib;
    ngx_str_t                     *value, path;
    ngx_uint_t                     i;

    if (lcf->lib != NULL) {
        return "is duplicate";
    }

    value = cf->args->elts;
    path = value[1];
    if (ngx_conf_full_name(cf->cycle, &path, 0) != NGX_OK) {
        return NGX_CONF_ERROR;
    }

    mcf = ngx_http_conf_get_module_main_conf(cf, ngx_http_tessera_module);
    libp = mcf->libs.elts;
    for (i = 0; i < mcf->libs.nelts; i++) {
        if (libp[i]->path.len == path.len
            && ngx_strncmp(libp[i]->path.data, path.data, path.len) == 0)
        {
            lcf->lib = libp[i];
            break;
        }
    }

    if (lcf->lib == NULL) {
        lib = ngx_pcalloc(cf->pool, sizeof(ngx_http_tessera_lib_t));
        if (lib == NULL) {
            return NGX_CONF_ERROR;
        }
        lib->path.len = path.len;
        lib->path.data = ngx_pnalloc(cf->pool, path.len + 1);
        if (lib->path.data == NULL) {
            return NGX_CONF_ERROR;
        }
        ngx_memcpy(lib->path.data, path.data, path.len);
        lib->path.data[path.len] = '\0';

        libp = ngx_array_push(&mcf->libs);
        if (libp == NULL) {
            return NGX_CONF_ERROR;
        }
        *libp = lib;
        lcf->lib = lib;
    }

    clcf = ngx_http_conf_get_module_loc_conf(cf, ngx_http_core_module);
    clcf->handler = ngx_http_tessera_handler;

    (void) cmd;
    return NGX_CONF_OK;
}

static char *
ngx_http_tessera_env(ngx_conf_t *cf, ngx_command_t *cmd, void *conf)
{
    ngx_http_tessera_main_conf_t  *mcf = conf;
    ngx_str_t                     *value;
    ngx_keyval_t                  *kv;

    value = cf->args->elts;
    if (value[1].len == 0 || ngx_strlchr(value[1].data, value[1].data + value[1].len, '=') != NULL) {
        return "has an invalid variable name";
    }

    kv = ngx_array_push(&mcf->env);
    if (kv == NULL) {
        return NGX_CONF_ERROR;
    }
    /* Directive arguments are NUL-terminated by the configuration parser. */
    kv->key = value[1];
    kv->value = value[2];

    (void) cmd;
    return NGX_CONF_OK;
}

static char *
ngx_http_tessera_thread_pool(ngx_conf_t *cf, ngx_command_t *cmd, void *conf)
{
    ngx_http_tessera_loc_conf_t  *lcf = conf;
    ngx_str_t                    *value = cf->args->elts;

#if (NGX_THREADS)
    if (lcf->pool_name.data != NULL) {
        return "is duplicate";
    }
    lcf->pool_name = value[1];
#else
    (void) lcf;
    ngx_conf_log_error(NGX_LOG_WARN, cf, 0,
                       "\"tessera_thread_pool %V\" ignored: nginx was built without "
                       "--with-threads, applications run in the worker and block it",
                       &value[1]);
#endif

    (void) cmd;
    return NGX_CONF_OK;
}

/* ---- worker start: environment + dlopen ---- */

static void
ngx_http_tessera_load(ngx_http_tessera_lib_t *lib, ngx_log_t *log)
{
    tessera_abi_version_fn  version;
    int32_t                 v;
    const char             *err;

    lib->dl = dlopen((const char *) lib->path.data, RTLD_NOW | RTLD_LOCAL);
    if (lib->dl == NULL) {
        err = dlerror();
        ngx_log_error(NGX_LOG_ALERT, log, 0, "tessera: dlopen(\"%V\") failed: %s",
                      &lib->path, err ? err : "unknown error");
        lib->error = "dlopen failed";
        return;
    }

    version = (tessera_abi_version_fn) dlsym(lib->dl, "tessera_abi_version");
    lib->handle = (tessera_handle_fn) dlsym(lib->dl, "tessera_handle");
    lib->free_response = (tessera_free_fn) dlsym(lib->dl, "tessera_free");
    if (version == NULL || lib->handle == NULL || lib->free_response == NULL) {
        ngx_log_error(NGX_LOG_ALERT, log, 0,
                      "tessera: \"%V\" is not a Tessera application library", &lib->path);
        lib->error = "missing tessera_* symbols";
        goto fail;
    }

    v = version();
    if (v != TESSERA_ABI_VERSION) {
        ngx_log_error(NGX_LOG_ALERT, log, 0,
                      "tessera: \"%V\" has ABI version %d, this module supports %d",
                      &lib->path, (int) v, TESSERA_ABI_VERSION);
        lib->error = "incompatible ABI version";
        goto fail;
    }
    return;

fail:
    /* Not dlclose'd: unloading Rust libraries is not reliable. */
    lib->handle = NULL;
    lib->free_response = NULL;
}

static ngx_int_t
ngx_http_tessera_init_process(ngx_cycle_t *cycle)
{
    ngx_http_tessera_main_conf_t  *mcf;
    ngx_http_tessera_lib_t       **libp;
    ngx_keyval_t                  *kv;
    ngx_uint_t                     i;

    mcf = ngx_http_cycle_get_module_main_conf(cycle, ngx_http_tessera_module);
    if (mcf == NULL) {
        return NGX_OK;
    }

    /* No worker threads exist yet, so setenv is safe here. */
    kv = mcf->env.elts;
    for (i = 0; i < mcf->env.nelts; i++) {
        if (setenv((const char *) kv[i].key.data, (const char *) kv[i].value.data, 1) != 0) {
            ngx_log_error(NGX_LOG_ALERT, cycle->log, ngx_errno, "tessera: setenv(\"%V\") failed",
                          &kv[i].key);
        }
    }

    libp = mcf->libs.elts;
    for (i = 0; i < mcf->libs.nelts; i++) {
        ngx_http_tessera_load(libp[i], cycle->log);
    }

    /* A broken library does not stop the worker: its locations answer 500. */
    return NGX_OK;
}

/* ---- request handling ---- */

static void
ngx_http_tessera_cleanup(void *data)
{
    ngx_http_tessera_ctx_t  *ctx = data;

    if (ctx->resp != NULL) {
        ctx->lib->free_response(ctx->resp, ctx->resp_len);
        ctx->resp = NULL;
    }
}

/* Raw path: the request line's URI up to '?', still percent-encoded. */
static ngx_str_t
ngx_http_tessera_raw_path(ngx_http_request_t *r)
{
    ngx_str_t  path;
    u_char    *q;

    path = r->unparsed_uri;
    q = ngx_strlchr(path.data, path.data + path.len, '?');
    if (q != NULL) {
        path.len = q - path.data;
    }
    if (path.len == 0) {
        path = r->uri;   /* should not happen; r->uri is decoded */
    }
    return path;
}

/* Builds the wire request in ctx->req. Returns NGX_OK or an HTTP status. */
static ngx_int_t
ngx_http_tessera_build_request(ngx_http_request_t *r, ngx_http_tessera_ctx_t *ctx, off_t max_body)
{
    ngx_list_part_t  *part;
    ngx_table_elt_t  *h;
    ngx_chain_t      *cl;
    ngx_buf_t        *b;
    ngx_str_t         path, remote;
    ngx_uint_t        i, nheaders;
    size_t            size, hsize;
    off_t             body_len, n;
    uint8_t          *p;
    ssize_t           got;

    path = ngx_http_tessera_raw_path(r);
    remote = r->connection->addr_text;

    body_len = 0;
    if (r->request_body != NULL) {
        for (cl = r->request_body->bufs; cl != NULL; cl = cl->next) {
            b = cl->buf;
            body_len += b->in_file ? b->file_last - b->file_pos : b->last - b->pos;
        }
    }
    if (body_len > max_body) {
        ngx_log_error(NGX_LOG_INFO, r->connection->log, 0,
                      "tessera: request body of %O bytes exceeds tessera_max_body %O",
                      body_len, max_body);
        return NGX_HTTP_REQUEST_ENTITY_TOO_LARGE;
    }

    nheaders = 0;
    hsize = 0;
    part = &r->headers_in.headers.part;
    h = part->elts;
    for (i = 0; /* void */; i++) {
        if (i >= part->nelts) {
            if (part->next == NULL) {
                break;
            }
            part = part->next;
            h = part->elts;
            i = 0;
            if (part->nelts == 0) {
                continue;
            }
        }
        if (h[i].hash == 0) {
            continue;   /* deleted */
        }
        hsize += tessera_field_size(h[i].key.len) + tessera_field_size(h[i].value.len);
        nheaders++;
    }

    size = 4 + tessera_field_size(r->method_name.len) + tessera_field_size(path.len)
           + tessera_field_size(r->args.len) + tessera_field_size(remote.len) + 4 + hsize
           + tessera_field_size((size_t) body_len);

    ctx->req = ngx_pnalloc(r->pool, size);
    if (ctx->req == NULL) {
        return NGX_HTTP_INTERNAL_SERVER_ERROR;
    }

    p = tessera_put_u32(ctx->req, TESSERA_WIRE_VERSION);
    p = tessera_put_bytes(p, r->method_name.data, r->method_name.len);
    p = tessera_put_bytes(p, path.data, path.len);
    p = tessera_put_bytes(p, r->args.data, r->args.len);
    p = tessera_put_bytes(p, remote.data, remote.len);
    p = tessera_put_u32(p, (uint32_t) nheaders);

    part = &r->headers_in.headers.part;
    h = part->elts;
    for (i = 0; /* void */; i++) {
        if (i >= part->nelts) {
            if (part->next == NULL) {
                break;
            }
            part = part->next;
            h = part->elts;
            i = 0;
            if (part->nelts == 0) {
                continue;
            }
        }
        if (h[i].hash == 0) {
            continue;
        }
        p = tessera_put_lower(p, (const char *) h[i].key.data, h[i].key.len);
        p = tessera_put_bytes(p, h[i].value.data, h[i].value.len);
    }

    p = tessera_put_u32(p, (uint32_t) body_len);
    if (r->request_body != NULL) {
        for (cl = r->request_body->bufs; cl != NULL; cl = cl->next) {
            b = cl->buf;
            if (b->in_file) {
                /* Body buffered to a temporary file by nginx. */
                n = b->file_last - b->file_pos;
                got = ngx_read_file(b->file, p, (size_t) n, b->file_pos);
                if (got != (ssize_t) n) {
                    ngx_log_error(NGX_LOG_ERR, r->connection->log, 0,
                                  "tessera: reading buffered request body failed");
                    return NGX_HTTP_INTERNAL_SERVER_ERROR;
                }
                p += n;
            } else {
                p = ngx_cpymem(p, b->pos, b->last - b->pos);
            }
        }
    }

    if ((size_t) (p - ctx->req) != size) {
        ngx_log_error(NGX_LOG_ALERT, r->connection->log, 0, "tessera: request size mismatch");
        return NGX_HTTP_INTERNAL_SERVER_ERROR;
    }
    ctx->req_len = size;
    return NGX_OK;
}

static ngx_int_t
ngx_http_tessera_is_hop_by_hop(const uint8_t *name, size_t n)
{
    static const char *const skip[] = { "content-length", "content-type", "transfer-encoding",
                                        "connection", "keep-alive", "upgrade", "status", NULL };
    ngx_uint_t  i;

    for (i = 0; skip[i] != NULL; i++) {
        if (ngx_strlen(skip[i]) == n && ngx_strncasecmp((u_char *) skip[i], (u_char *) name, n) == 0) {
            return 1;
        }
    }
    return 0;
}

/* Sends the application's response. Returns a value for ngx_http_finalize_request. */
static ngx_int_t
ngx_http_tessera_send_response(ngx_http_request_t *r, ngx_http_tessera_ctx_t *ctx)
{
    tessera_reader    rd;
    uint32_t          version, status, count, i;
    const uint8_t    *name, *value, *body;
    size_t            nl, vl, body_len;
    ngx_table_elt_t  *h;
    ngx_buf_t        *b;
    ngx_chain_t       out;
    ngx_int_t         rc;

    if (ctx->rc != TESSERA_OK || ctx->resp == NULL) {
        ngx_log_error(NGX_LOG_ERR, r->connection->log, 0,
                      "tessera: tessera_handle failed with code %d", (int) ctx->rc);
        return NGX_HTTP_INTERNAL_SERVER_ERROR;
    }

    tessera_reader_init(&rd, ctx->resp, ctx->resp_len);
    version = tessera_read_u32(&rd);
    status = tessera_read_u32(&rd);
    count = tessera_read_u32(&rd);
    if (!rd.ok || version != TESSERA_WIRE_VERSION || status < 100 || status > 999) {
        goto malformed;
    }

    /* Header strings point into the response buffer, which stays alive until
     * the request pool is destroyed (see ngx_http_tessera_cleanup). */
    for (i = 0; i < count; i++) {
        name = tessera_read_bytes(&rd, &nl);
        value = tessera_read_bytes(&rd, &vl);
        if (!rd.ok) {
            goto malformed;
        }
        if (nl == 12 && ngx_strncasecmp((u_char *) name, (u_char *) "content-type", 12) == 0) {
            r->headers_out.content_type.data = (u_char *) value;
            r->headers_out.content_type.len = vl;
            r->headers_out.content_type_len = vl;
            r->headers_out.content_type_lowcase = NULL;
            continue;
        }
        if (ngx_http_tessera_is_hop_by_hop(name, nl)) {
            continue;
        }
        h = ngx_list_push(&r->headers_out.headers);
        if (h == NULL) {
            return NGX_HTTP_INTERNAL_SERVER_ERROR;
        }
        h->hash = 1;
#if (nginx_version >= 1023000)
        h->next = NULL;
#endif
        h->key.data = (u_char *) name;
        h->key.len = nl;
        h->value.data = (u_char *) value;
        h->value.len = vl;
        h->lowcase_key = NULL;
    }

    body = tessera_read_bytes(&rd, &body_len);
    if (!rd.ok) {
        goto malformed;
    }

    r->headers_out.status = status;
    r->headers_out.content_length_n = body_len;

    rc = ngx_http_send_header(r);
    if (rc == NGX_ERROR || rc > NGX_OK || r->header_only) {
        return rc;
    }

    b = ngx_calloc_buf(r->pool);
    if (b == NULL) {
        return NGX_ERROR;
    }
    if (body_len > 0) {
        b->pos = (u_char *) body;
        b->last = (u_char *) body + body_len;
        b->memory = 1;   /* read-only memory owned by the library */
    }
    b->last_buf = (r == r->main) ? 1 : 0;
    b->last_in_chain = 1;

    out.buf = b;
    out.next = NULL;
    return ngx_http_output_filter(r, &out);

malformed:
    ngx_log_error(NGX_LOG_ERR, r->connection->log, 0, "tessera: malformed response from application");
    return NGX_HTTP_INTERNAL_SERVER_ERROR;
}

#if (NGX_THREADS)

/* Runs in a thread pool thread: only touches ctx, never the request. */
static void
ngx_http_tessera_thread_handler(void *data, ngx_log_t *log)
{
    ngx_http_tessera_ctx_t  *ctx = data;

    (void) log;
    ctx->rc = ctx->lib->handle(ctx->req, ctx->req_len, &ctx->resp, &ctx->resp_len);
}

/* Runs in the event loop when the task has finished (modelled on
 * ngx_http_upstream_thread_event_handler). */
static void
ngx_http_tessera_thread_done(ngx_event_t *ev)
{
    ngx_http_request_t      *r = ev->data;
    ngx_connection_t        *c = r->connection;
    ngx_http_tessera_ctx_t  *ctx;

    ngx_http_set_log_request(c->log, r);

    r->main->blocked--;
    r->aio = 0;

#if (NGX_HTTP_V2)
    if (r->stream) {
        c->write->ready = 1;
        c->write->active = 0;
    }
#endif

    if (r->done || r->main->terminated) {
        /* Client went away / request was terminated meanwhile. The response
         * buffer is released by the pool cleanup. */
        c->write->handler(c->write);
        return;
    }

    ctx = ngx_http_get_module_ctx(r, ngx_http_tessera_module);
    ngx_http_finalize_request(r, ngx_http_tessera_send_response(r, ctx));
    ngx_http_run_posted_requests(c);
}

#endif

/* Called when the whole request body has been read. */
static void
ngx_http_tessera_body_handler(ngx_http_request_t *r)
{
    ngx_http_tessera_loc_conf_t  *lcf;
    ngx_http_tessera_ctx_t       *ctx;
    ngx_pool_cleanup_t           *cln;
    ngx_int_t                     rc;
#if (NGX_THREADS)
    ngx_thread_task_t            *task;
#endif

    lcf = ngx_http_get_module_loc_conf(r, ngx_http_tessera_module);

    ctx = ngx_pcalloc(r->pool, sizeof(ngx_http_tessera_ctx_t));
    if (ctx == NULL) {
        ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
        return;
    }
    ctx->request = r;
    ctx->lib = lcf->lib;
    ngx_http_set_ctx(r, ctx, ngx_http_tessera_module);

    rc = ngx_http_tessera_build_request(r, ctx, lcf->max_body);
    if (rc != NGX_OK) {
        ngx_http_finalize_request(r, rc);
        return;
    }

    /* Make sure the response buffer is returned to the library whatever happens. */
    cln = ngx_pool_cleanup_add(r->pool, 0);
    if (cln == NULL) {
        ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
        return;
    }
    cln->handler = ngx_http_tessera_cleanup;
    cln->data = ctx;

#if (NGX_THREADS)
    if (lcf->pool != NULL) {
        task = ngx_thread_task_alloc(r->pool, 0);
        if (task == NULL) {
            ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
            return;
        }
        task->ctx = ctx;
        task->handler = ngx_http_tessera_thread_handler;
        task->event.data = r;
        task->event.handler = ngx_http_tessera_thread_done;

        if (r->main->blocked >= NGX_HTTP_MAX_BLOCKED) {
            ngx_log_error(NGX_LOG_ERR, r->connection->log, 0, "tessera: too many operations pending");
            ngx_http_finalize_request(r, NGX_HTTP_INTERNAL_SERVER_ERROR);
            return;
        }
        if (ngx_thread_task_post(lcf->pool, task) != NGX_OK) {
            ngx_http_finalize_request(r, NGX_HTTP_SERVICE_UNAVAILABLE);
            return;
        }
        r->main->blocked++;
        r->aio = 1;
        return;   /* finished in ngx_http_tessera_thread_done */
    }
#endif

    /* No thread pool: blocks this worker until the application returns. */
    ctx->rc = ctx->lib->handle(ctx->req, ctx->req_len, &ctx->resp, &ctx->resp_len);
    ngx_http_finalize_request(r, ngx_http_tessera_send_response(r, ctx));
}

static ngx_int_t
ngx_http_tessera_handler(ngx_http_request_t *r)
{
    ngx_http_tessera_loc_conf_t  *lcf;
    ngx_int_t                     rc;

    lcf = ngx_http_get_module_loc_conf(r, ngx_http_tessera_module);
    if (lcf->lib == NULL) {
        return NGX_DECLINED;
    }
    if (lcf->lib->handle == NULL) {
        ngx_log_error(NGX_LOG_ERR, r->connection->log, 0, "tessera: application \"%V\" is not loaded: %s",
                      &lcf->lib->path, lcf->lib->error ? lcf->lib->error : "not initialised");
        return NGX_HTTP_INTERNAL_SERVER_ERROR;
    }
    if (r->headers_in.content_length_n > lcf->max_body) {
        ngx_log_error(NGX_LOG_INFO, r->connection->log, 0,
                      "tessera: request body of %O bytes exceeds tessera_max_body %O",
                      r->headers_in.content_length_n, lcf->max_body);
        return NGX_HTTP_REQUEST_ENTITY_TOO_LARGE;
    }

    rc = ngx_http_read_client_request_body(r, ngx_http_tessera_body_handler);
    if (rc >= NGX_HTTP_SPECIAL_RESPONSE) {
        return rc;
    }
    return NGX_DONE;
}
