/*
 * tessera.h - C ABI of a Tessera web application built as a shared library
 *             (`tessera1 build app.tes --library -o libapp.so`).
 *
 * Exported symbols (implemented in runtime/adapter.rs):
 *
 *   int32_t tessera_abi_version(void);
 *       Returns TESSERA_ABI_VERSION (1). Hosts must refuse a library that
 *       returns anything else.
 *
 *   int32_t tessera_handle(const uint8_t *request, size_t request_len,
 *                          uint8_t **response, size_t *response_len);
 *       Runs the application for one wire-format request (below). Returns 0
 *       on success and stores a library-allocated response buffer in
 *       *response / *response_len. Application failures (runtime error,
 *       time/memory limit, undecodable request) are reported as an HTTP
 *       status (500/400/...) inside a normal response, not as a nonzero
 *       return. Nonzero means: 1 = invalid arguments (NULL pointers),
 *       2 = internal failure; *response is then NULL. The call blocks until
 *       the application finishes. It is thread-safe: every call runs the
 *       application on its own thread and calls share no state.
 *
 *   void tessera_free(uint8_t *response, size_t response_len);
 *       Releases a response buffer. Pass exactly the pointer and length
 *       returned by tessera_handle; free each buffer once. NULL is a no-op.
 *
 * Wire format (all integers are uint32 little-endian; "str"/"bytes" are a
 * uint32 length followed by that many bytes; strings are UTF-8):
 *
 *   request:  u32 version (=1)
 *             str method            e.g. "GET"
 *             str path              raw, still percent-encoded, no query
 *             str query             without the leading '?', may be empty
 *             str remote_addr       client IP address as text
 *             u32 header_count
 *             header_count x (str name, str value)   names lower-cased
 *             bytes body
 *
 *   response: u32 version (=1)
 *             u32 status
 *             u32 header_count
 *             header_count x (str name, str value)
 *             bytes body
 *
 * The static inline helpers below (writer/reader) are optional conveniences
 * used by the bundled Apache, nginx and IIS modules and by the ABI test.
 */
#ifndef TESSERA_H
#define TESSERA_H

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

#define TESSERA_ABI_VERSION 1
#define TESSERA_WIRE_VERSION 1

#define TESSERA_OK 0
#define TESSERA_ERR_ARGUMENT 1
#define TESSERA_ERR_INTERNAL 2

typedef int32_t (*tessera_abi_version_fn)(void);
typedef int32_t (*tessera_handle_fn)(const uint8_t *request, size_t request_len,
                                     uint8_t **response, size_t *response_len);
typedef void (*tessera_free_fn)(uint8_t *response, size_t response_len);

#ifndef TESSERA_NO_PROTOTYPES
int32_t tessera_abi_version(void);
int32_t tessera_handle(const uint8_t *request, size_t request_len,
                       uint8_t **response, size_t *response_len);
void tessera_free(uint8_t *response, size_t response_len);
#endif

/* ---- writer: compute the size first, then fill a buffer of that size ---- */

/* Size of a "str"/"bytes" field holding n bytes. */
static inline size_t tessera_field_size(size_t n) { return 4 + n; }

static inline uint8_t *tessera_put_u32(uint8_t *p, uint32_t v)
{
    p[0] = (uint8_t)(v & 0xff);
    p[1] = (uint8_t)((v >> 8) & 0xff);
    p[2] = (uint8_t)((v >> 16) & 0xff);
    p[3] = (uint8_t)((v >> 24) & 0xff);
    return p + 4;
}

static inline uint8_t *tessera_put_bytes(uint8_t *p, const void *data, size_t n)
{
    p = tessera_put_u32(p, (uint32_t)n);
    if (n > 0) {
        memcpy(p, data, n);
    }
    return p + n;
}

/* Copies a header name lower-cased (the wire format requires that). */
static inline uint8_t *tessera_put_lower(uint8_t *p, const char *data, size_t n)
{
    size_t i;
    p = tessera_put_u32(p, (uint32_t)n);
    for (i = 0; i < n; i++) {
        char c = data[i];
        p[i] = (uint8_t)((c >= 'A' && c <= 'Z') ? c + ('a' - 'A') : c);
    }
    return p + n;
}

/* ---- reader: bounds-checked; any overrun sets ok = 0 ---- */

typedef struct {
    const uint8_t *data;
    size_t len;
    size_t pos;
    int ok;
} tessera_reader;

static inline void tessera_reader_init(tessera_reader *r, const uint8_t *data, size_t len)
{
    r->data = data;
    r->len = len;
    r->pos = 0;
    r->ok = 1;
}

static inline uint32_t tessera_read_u32(tessera_reader *r)
{
    const uint8_t *p;
    if (!r->ok || r->len - r->pos < 4) {
        r->ok = 0;
        return 0;
    }
    p = r->data + r->pos;
    r->pos += 4;
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

/* Returns a pointer into the buffer (not NUL-terminated) and its length. */
static inline const uint8_t *tessera_read_bytes(tessera_reader *r, size_t *n)
{
    uint32_t len = tessera_read_u32(r);
    const uint8_t *p;
    *n = 0;
    if (!r->ok || r->len - r->pos < len) {
        r->ok = 0;
        return NULL;
    }
    p = r->data + r->pos;
    r->pos += len;
    *n = len;
    return p;
}

#ifdef __cplusplus
}
#endif

#endif /* TESSERA_H */
