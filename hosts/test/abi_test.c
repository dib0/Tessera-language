/*
 * abi_test.c - loads a Tessera application library with dlopen and exercises
 * the C ABI (see hosts/include/tessera.h).
 *
 * Expects the test stub application in hosts/test/app.rs (or a compiled
 * Tessera program with the same behaviour):
 *   path ending in /notfound -> 404, /fail -> 500, otherwise 201 + x-tessera-test: yes and
 *   an echo body.
 *
 * Build: gcc -O2 -Wall -Wextra -I../include -o abi_test abi_test.c -ldl -lpthread
 * Run:   ./abi_test /path/to/libapp.so
 */
#define TESSERA_NO_PROTOTYPES
#include "tessera.h"

#include <dlfcn.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static tessera_abi_version_fn p_version;
static tessera_handle_fn p_handle;
static tessera_free_fn p_free;

static int failures = 0;
static int checks = 0;

#define CHECK(cond, ...)                                     \
    do {                                                     \
        checks++;                                            \
        if (!(cond)) {                                       \
            failures++;                                      \
            fprintf(stderr, "FAIL %s:%d: ", __FILE__, __LINE__); \
            fprintf(stderr, __VA_ARGS__);                    \
            fprintf(stderr, "\n");                           \
        }                                                    \
    } while (0)

typedef struct {
    const char *name;
    const char *value;
} header;

static uint8_t *build_request(const char *method, const char *path, const char *query,
                              const char *remote, const header *headers, size_t nheaders,
                              const uint8_t *body, size_t body_len, size_t *out_len)
{
    size_t size = 4 + tessera_field_size(strlen(method)) + tessera_field_size(strlen(path)) +
                  tessera_field_size(strlen(query)) + tessera_field_size(strlen(remote)) + 4 +
                  tessera_field_size(body_len);
    size_t i;
    for (i = 0; i < nheaders; i++) {
        size += tessera_field_size(strlen(headers[i].name)) + tessera_field_size(strlen(headers[i].value));
    }
    uint8_t *buf = malloc(size);
    if (!buf) {
        perror("malloc");
        exit(2);
    }
    uint8_t *p = buf;
    p = tessera_put_u32(p, TESSERA_WIRE_VERSION);
    p = tessera_put_bytes(p, method, strlen(method));
    p = tessera_put_bytes(p, path, strlen(path));
    p = tessera_put_bytes(p, query, strlen(query));
    p = tessera_put_bytes(p, remote, strlen(remote));
    p = tessera_put_u32(p, (uint32_t)nheaders);
    for (i = 0; i < nheaders; i++) {
        p = tessera_put_lower(p, headers[i].name, strlen(headers[i].name));
        p = tessera_put_bytes(p, headers[i].value, strlen(headers[i].value));
    }
    p = tessera_put_bytes(p, body, body_len);
    if ((size_t)(p - buf) != size) {
        fprintf(stderr, "internal size mismatch\n");
        exit(2);
    }
    *out_len = size;
    return buf;
}

typedef struct {
    int valid;
    uint32_t status;
    char header_value[256]; /* value of x-tessera-test */
    char content_type[256];
    const uint8_t *body;
    size_t body_len;
    uint8_t *raw;
    size_t raw_len;
} response;

static void copy_str(char *dst, size_t cap, const uint8_t *src, size_t n)
{
    if (n >= cap) n = cap - 1;
    memcpy(dst, src, n);
    dst[n] = 0;
}

/* Calls tessera_handle and parses the response. Returns the call's result. */
static int call(const uint8_t *req, size_t req_len, response *out)
{
    memset(out, 0, sizeof *out);
    int rc = p_handle(req, req_len, &out->raw, &out->raw_len);
    if (rc != 0) return rc;
    tessera_reader r;
    tessera_reader_init(&r, out->raw, out->raw_len);
    uint32_t version = tessera_read_u32(&r);
    out->status = tessera_read_u32(&r);
    uint32_t count = tessera_read_u32(&r);
    uint32_t i;
    for (i = 0; r.ok && i < count; i++) {
        size_t nl, vl;
        const uint8_t *n = tessera_read_bytes(&r, &nl);
        const uint8_t *v = tessera_read_bytes(&r, &vl);
        if (!r.ok) break;
        if (nl == 14 && memcmp(n, "x-tessera-test", 14) == 0) copy_str(out->header_value, sizeof out->header_value, v, vl);
        if (nl == 12 && memcmp(n, "content-type", 12) == 0) copy_str(out->content_type, sizeof out->content_type, v, vl);
    }
    out->body = tessera_read_bytes(&r, &out->body_len);
    out->valid = r.ok && version == TESSERA_WIRE_VERSION && r.pos == r.len;
    return rc;
}

static void release(response *resp)
{
    p_free(resp->raw, resp->raw_len);
    resp->raw = NULL;
}

static int body_contains(const response *resp, const char *needle)
{
    size_t n = strlen(needle);
    size_t i;
    if (resp->body_len < n) return 0;
    for (i = 0; i + n <= resp->body_len; i++) {
        if (memcmp(resp->body + i, needle, n) == 0) return 1;
    }
    return 0;
}

static void test_normal(void)
{
    header h[] = {{"Host", "example.test"}, {"X-Custom", "abc 123"}, {"Content-Type", "text/plain"}};
    size_t len;
    uint8_t *req = build_request("POST", "/hello%20world", "a=1&b=2", "192.0.2.7", h, 3,
                                 (const uint8_t *)"hi there", 8, &len);
    response resp;
    int rc = call(req, len, &resp);
    CHECK(rc == 0, "normal: rc=%d", rc);
    CHECK(resp.valid, "normal: response not well-formed");
    CHECK(resp.status == 201, "normal: status=%u", resp.status);
    CHECK(strcmp(resp.header_value, "yes") == 0, "normal: x-tessera-test='%s'", resp.header_value);
    CHECK(strncmp(resp.content_type, "text/plain", 10) == 0, "normal: content-type='%s'", resp.content_type);
    CHECK(body_contains(&resp, "method=POST\n"), "normal: method missing");
    CHECK(body_contains(&resp, "path=/hello%20world\n"), "normal: raw path missing");
    CHECK(body_contains(&resp, "query=a=1&b=2\n"), "normal: query missing");
    CHECK(body_contains(&resp, "remote=192.0.2.7\n"), "normal: remote missing");
    CHECK(body_contains(&resp, "x-custom=abc 123\n"), "normal: lower-cased header lookup failed");
    CHECK(body_contains(&resp, "body-length=8\nbody=hi there"), "normal: body echo missing");
    release(&resp);
    free(req);
}

static void test_empty_body(void)
{
    size_t len;
    uint8_t *req = build_request("GET", "/", "", "::1", NULL, 0, NULL, 0, &len);
    response resp;
    int rc = call(req, len, &resp);
    CHECK(rc == 0, "empty: rc=%d", rc);
    CHECK(resp.valid, "empty: response not well-formed");
    CHECK(resp.status == 201, "empty: status=%u", resp.status);
    CHECK(body_contains(&resp, "body-length=0\nbody="), "empty: body-length");
    release(&resp);
    free(req);
}

static void test_large_body(void)
{
    size_t n = 5 * 1024 * 1024, i, len;
    uint8_t *body = malloc(n);
    for (i = 0; i < n; i++) body[i] = (uint8_t)(i * 31 + 7);
    uint8_t *req = build_request("PUT", "/upload", "", "127.0.0.1", NULL, 0, body, n, &len);
    response resp;
    int rc = call(req, len, &resp);
    CHECK(rc == 0, "large: rc=%d", rc);
    CHECK(resp.valid, "large: response not well-formed");
    CHECK(resp.status == 201, "large: status=%u", resp.status);
    CHECK(body_contains(&resp, "body-length=5242880\n"), "large: body-length");
    CHECK(resp.body_len >= n && memcmp(resp.body + resp.body_len - n, body, n) == 0,
          "large: echoed body differs");
    release(&resp);
    free(req);
    free(body);
}

static void test_status_codes(void)
{
    size_t len;
    response resp;
    uint8_t *req = build_request("GET", "/notfound", "", "127.0.0.1", NULL, 0, NULL, 0, &len);
    CHECK(call(req, len, &resp) == 0, "404: rc");
    CHECK(resp.valid && resp.status == 404, "404: status=%u", resp.status);
    release(&resp);
    free(req);

    req = build_request("GET", "/fail", "", "127.0.0.1", NULL, 0, NULL, 0, &len);
    CHECK(call(req, len, &resp) == 0, "fail: rc");
    CHECK(resp.valid && resp.status == 500, "fail: status=%u (expected 500 from runtime error)", resp.status);
    release(&resp);
    free(req);
}

static void test_garbage(void)
{
    response resp;
    /* random-ish bytes */
    uint8_t junk[64];
    size_t i;
    for (i = 0; i < sizeof junk; i++) junk[i] = (uint8_t)(i * 97 + 13);
    CHECK(call(junk, sizeof junk, &resp) == 0, "garbage: rc");
    CHECK(resp.valid && resp.status >= 400 && resp.status < 600, "garbage: status=%u", resp.status);
    release(&resp);

    /* wrong version, otherwise valid */
    size_t len;
    uint8_t *req = build_request("GET", "/", "", "127.0.0.1", NULL, 0, NULL, 0, &len);
    req[0] = 99;
    CHECK(call(req, len, &resp) == 0, "version: rc");
    CHECK(resp.valid && resp.status >= 400 && resp.status < 600, "version: status=%u", resp.status);
    release(&resp);

    /* truncated request: claims a huge string length */
    req[0] = 1;
    tessera_put_u32(req + 4, 0xFFFFFFF0u);
    CHECK(call(req, len, &resp) == 0, "truncated: rc");
    CHECK(resp.valid && resp.status >= 400 && resp.status < 600, "truncated: status=%u", resp.status);
    release(&resp);
    free(req);

    /* empty input with NULL pointer */
    CHECK(call(NULL, 0, &resp) == 0, "empty input: rc");
    CHECK(resp.valid && resp.status >= 400 && resp.status < 600, "empty input: status=%u", resp.status);
    release(&resp);
}

static void test_invalid_arguments(void)
{
    uint8_t *out = (uint8_t *)1;
    size_t out_len = 99;
    uint8_t dummy[4] = {0};
    CHECK(p_handle(dummy, 4, NULL, &out_len) == TESSERA_ERR_ARGUMENT, "NULL response pointer accepted");
    CHECK(p_handle(dummy, 4, &out, NULL) == TESSERA_ERR_ARGUMENT, "NULL length pointer accepted");
    CHECK(p_handle(NULL, 10, &out, &out_len) == TESSERA_ERR_ARGUMENT, "NULL request with length accepted");
    CHECK(out == NULL && out_len == 0, "out-parameters not cleared on error");
    p_free(NULL, 0); /* must be a no-op */
}

#define THREADS 16
#define PER_THREAD 25

static void *worker(void *arg)
{
    long id = (long)arg;
    int i, bad = 0;
    for (i = 0; i < PER_THREAD; i++) {
        char query[64], expect[96];
        snprintf(query, sizeof query, "t=%ld&i=%d", id, i);
        snprintf(expect, sizeof expect, "query=%s\n", query);
        size_t len;
        uint8_t *req = build_request("GET", "/thread", query, "127.0.0.1", NULL, 0, NULL, 0, &len);
        response resp;
        if (call(req, len, &resp) != 0 || !resp.valid || resp.status != 201 || !body_contains(&resp, expect)) bad++;
        if (resp.raw) release(&resp);
        free(req);
    }
    return (void *)(long)bad;
}

static void test_concurrency(void)
{
    pthread_t t[THREADS];
    long i, bad = 0;
    for (i = 0; i < THREADS; i++) pthread_create(&t[i], NULL, worker, (void *)i);
    for (i = 0; i < THREADS; i++) {
        void *r;
        pthread_join(t[i], &r);
        bad += (long)r;
    }
    CHECK(bad == 0, "concurrency: %ld of %d requests wrong", bad, THREADS * PER_THREAD);
}

int main(int argc, char **argv)
{
    if (argc != 2) {
        fprintf(stderr, "usage: %s /path/to/libapp.so\n", argv[0]);
        return 2;
    }
    void *lib = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!lib) {
        fprintf(stderr, "dlopen: %s\n", dlerror());
        return 2;
    }
    p_version = (tessera_abi_version_fn)dlsym(lib, "tessera_abi_version");
    p_handle = (tessera_handle_fn)dlsym(lib, "tessera_handle");
    p_free = (tessera_free_fn)dlsym(lib, "tessera_free");
    if (!p_version || !p_handle || !p_free) {
        fprintf(stderr, "missing symbol: %s\n", dlerror());
        return 2;
    }
    CHECK(p_version() == TESSERA_ABI_VERSION, "abi version %d", p_version());

    test_normal();
    test_empty_body();
    test_large_body();
    test_status_codes();
    test_garbage();
    test_invalid_arguments();
    test_concurrency();

    printf("%d checks, %d failures\n", checks, failures);
    if (failures == 0) printf("ABI TEST PASSED\n");
    return failures == 0 ? 0 : 1;
}
