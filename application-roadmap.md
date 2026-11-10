# From command-line programs to applications and web applications

**Status:** analysis of 2026-11-08 against draft 0.12; the P1 to P3 items were implemented the same day in draft 0.13 (see the next section and `language-design.md`). The rest of this document is the original analysis, kept for its reasoning.

## 0. Implementation status (draft 0.13)

| Item | Status |
|---|---|
| Runtime errors unwind to the host; stack depth (R013), time (R014) and memory (R015) limits; `exit` in a request (R017) | Done |
| File functions returning `Result` (`read_text`, `write_text`, `list_directory`, `create_directory`, `remove_file`) | Done, as new names: `read_file` and `write_file` stay, because the compiler's own source uses them and the bootstrap seed would not accept a changed type |
| `Map`, `Bytes`, `Decimal`, function values with `map`/`filter`/`sort_by` | Done. `Decimal` has a fixed 9 decimal places |
| Text functions, standard input, `time`, `random`, `log` | Done. Time values are `Int` milliseconds, not separate `Instant`/`Duration` types |
| Standard modules `std/http`, `std/json`, `std/url`, `std/html` | Done, written in Tessera |
| Handler contract, test host, CGI, HTTP server, FastCGI | Done |
| `db` | Done for SQLite, through the `sqlite3` program as a child process (no crates are available to generated programs); PostgreSQL not started |
| `net` | Plain HTTP client only; TLS needs a dependency decision |
| Crypto | SHA-256, HMAC, PBKDF2 (instead of Argon2, which is impractical to write from scratch), base64, hex, constant-time comparison, secure random |
| Cookies, signed values, CSRF tokens, security headers | Done in `std/http` |
| Tests in the language, `tessera1 test`; `tessera1 format` | Done; the formatter applies the layout rules, not yet spacing within lines |
| `--library`, C adapter, nginx and Apache modules | Done and tested here against real nginx 1.30 and Apache 2.4 (`hosts/README.md`) |
| IIS | HttpPlatformHandler and CGI configurations (the server and CGI modes are tested on Linux); the native IIS module is written but could not be compiled or run here |
| Narrower capabilities | `TESSERA_FS_ROOT` and `TESSERA_NET_ALLOW`, enforced by the runtime; not yet visible in signatures |
| Request limits | Body size, header count and size, time and memory per request |
| User-defined generics, closures, nested patterns, `Set`, constants, streaming bodies, packages | Not done (P4, or deferred). Generics were listed as P2 in table 3.2 but in phase 5 of section 4; built-in `Map`, `Bytes` and `std/json` covered the cases that needed them |

## 1. The target: one handler, several hosts

A web application should be written once and run unchanged under different hosts:

```tessera
fn handle(request: http.Request) -> http.Response uses db, log {
match request.path {
"/" => http.html(200, "<h1>Hello</h1>")
"/api/items" => http.json(200, items_json(try db.query("SELECT name FROM items", [])))
_ => http.text(404, "not found")
}
}
```

| Host | How it runs the handler | Effort |
|---|---|---|
| Test host | Calls `handle` with constructed requests; used by tests and the harness | Small |
| CGI | One process per request: request from environment variables and standard input, response on standard output. Works with Apache (`mod_cgi`) and nginx (via `fcgiwrap`) without any module | Small, but needs standard input |
| Built-in server | `tessera1 serve app.tes`: HTTP/1.1 server for development and simple deployments | Medium |
| FastCGI / reverse proxy | Long-running process behind nginx or Apache, as with PHP-FPM | Medium |
| nginx and Apache modules | The application compiled as a shared library and loaded into the server's worker processes | Large |

The language-level requirements are the same for every host: a handler contract (section 3.5), errors that do not end the process (section 3.1), and no state shared between requests except through effects. Tessera's lack of global variables already gives the last property for free. That is a real advantage for in-process hosting.

**Recommendation:** build the hosts in the order of the table. CGI and the built-in server prove the contract cheaply; the modules come last and reuse it.

## 2. What exists today

| Area | Available |
|---|---|
| Types | `Int` (64-bit), `Bool`, `Text`, `Unit`, `List[T]`, `Option[T]`, `Result[T, E]`, records, enums (recursive) |
| Text | `length`, `split`, `join`, `index_of`, `contains`, `to_lower`, `to_upper`, `code_points`, `from_code_point(s)`, interpolation, `+` |
| Lists | `+`, `slice`, `replace_at`, `sort` (Int/Text), `index_of`, `contains`, `range` |
| Effects | `io` (print), `env` (args, exit, environment variables), `fs` (whole files), `process` (run a program), `ui` (one window) |
| Structure | Modules (`import`, `pub`), notes, no globals, no shadowing |
| Safety | Bounds and overflow checks, no `unsafe` in generated code, effect checking |

## 3. What is missing

Priority: **P1** blocks real applications, **P2** needed for web applications, **P3** needed for the server modules, **P4** improves programs but has a workaround.

### 3.1 Robustness: errors must not end the process (P1)

| Missing | Why | Proposed shape |
|---|---|---|
| Recoverable runtime errors at a boundary | A runtime error (R001 to R012) calls `process::exit(70)`. In a server module that kills the worker and every request it is handling | Runtime errors unwind (a Rust panic, caught with `catch_unwind`, which is safe Rust) to the host boundary. The CLI host still exits with 70. A web host answers `500` and logs the message and location. The language itself stays without exceptions |
| `exit` and `exit_with` in a handler | Also end the process | Compile error (or a runtime error turned into 500) when used from a handler; only `main` may end the program |
| Resource limits per request | An endless loop or huge allocation in one request blocks or kills a worker | Time limit and allocation limit enforced by the runtime (counting loop iterations and list growth), reported as new runtime errors |
| Deep recursion | Overflowing the 512 MB stack aborts the process (known limitation, section 12). A module cannot reserve such a stack | A depth counter in generated functions, giving a runtime error instead of an abort |
| File errors as values | `read_file` and `write_file` stop the program on failure (R006/R007). An application must handle a missing file | `read_file(path) -> Result[Text, FileError]`, same for `write_file`. Breaking change, so introduce it before applications exist |

### 3.2 Data types (P1/P2)

| Missing | Why | Proposed shape |
|---|---|---|
| `Map[K, V]` (P1) | Sessions, headers, JSON objects, caches, counting. Planned (section 18.2); the harness programs search lists by hand instead | Built-in immutable map with in-place update under the same rule as lists. `get(map, key) -> Option[V]`, `put`, `remove`, `keys` (sorted for deterministic output). Keys `Int` or `Text` at first |
| `Set[T]` (P4) | Deduplication, permissions | Can wait; `Map[T, Unit]` works meanwhile |
| `Bytes` (P2) | HTTP bodies, uploads, images, hashing and base64 all work on bytes; `Text` must stay valid UTF-8 | Immutable byte list with `to_bytes(text)`, `from_bytes(bytes) -> Option[Text]`, indexing and `slice` |
| Decimal numbers (P2) | Prices, percentages, coordinates, JSON numbers | Decide between `Float` (fast, inexact, a known source of bugs) and a fixed-point `Decimal` (exact, matches "money in cents" from the spec). Recommendation: `Decimal` first, as it matches Tessera's predictability goal |
| Constants (P4) | Configuration values are written as functions (`fn max_size() -> Int = 100`) | `const MAX_SIZE = 100` (immutable, compile-time). Not a global variable, so it keeps the no-shared-state property |
| User-defined generics (P2) | A JSON value, a `Page[T]` or a typed repository cannot be written once for several types | Planned (section 18.3). Measure what models write first; built-in `Map` and `Bytes` remove most of the need |
| First-class functions (P2) | Sorting records by a field, routing tables, middleware. `sort` only handles `Int` and `Text` lists today | Named functions as values (`sort_by(items, item_price)`), without closures at first. That keeps the code reviewable and covers routing and sorting |
| Nested patterns and guards (P4) | `.Error(.NotFound(id))` must be two matches today | Allow one level of nesting first |

### 3.3 Standard library (P1/P2)

| Missing | Why | Proposed shape |
|---|---|---|
| Text functions (P1) | Every parser and every web handler needs them | `trim`, `starts_with`, `ends_with`, `replace`, `slice_text(text, start, end)` (characters), `repeat`, `pad_left`, `lines` |
| Number formatting (P2) | Prices, padded IDs, file sizes | `format_int(n, width)`, plus `Decimal` formatting |
| Standard library modules (P2) | One flat built-in list does not scale to HTTP, JSON and crypto | `import std/http`, `std/json`, `std/url`, `std/html`, `std/crypto`, written in Tessera where possible, so that their code is reviewable and checked like any other |
| JSON (P2) | The format of nearly every API | `std/json`: `parse(text) -> Result[Json, JsonError]`, a `Json` enum, and an encoder that escapes correctly. Typed decoding into records needs generics or generated code; start with the `Json` enum |
| URL, query and form handling (P2) | Routing and form input | `std/url`: percent-decoding and encoding, query parsing into `Map[Text, List[Text]]`, path segments |
| HTML escaping (P2) | Interpolating user input into HTML causes cross-site scripting | `html.escape(text)`; later possibly an `Html` type whose interpolation escapes automatically. Since interpolation is how Tessera builds text, this deserves a design decision before web use |
| Date and time values (P2) | Logging, expiry, cookies | `Instant` (UTC) and `Duration` types, formatting as RFC 3339 and as the HTTP date format |

### 3.4 Effects: talking to the world (P1/P2)

Effects are Tessera's capability system, so every new kind of access is a new effect. A reviewer can then see from a signature what a function can do.

| Missing | Why | Proposed shape |
|---|---|---|
| Standard input (P1) | Pipes, interactive programs, CGI request bodies. Absent today | `read_line() -> Option[Text]` and `read_input() -> Text` under `io` |
| `time` (P1) | Timestamps, timeouts, measuring | `now() -> Instant`, `monotonic()`, `sleep(duration)` under a new effect `time`. Keeping it an effect keeps pure functions deterministic and testable |
| `random` (P1) | IDs, games, sampling | `random_int(low, high)` under `random`; `secure_random_bytes(n)` for tokens |
| Directories (P2) | Static files, uploads, configuration directories | `list_directory`, `create_directory`, `remove_file` under `fs` |
| `log` (P2) | Servers need structured logs that go to the host's log, not standard output | `log(level, message)` under `log`; the host decides where it goes |
| `net` (P2) | HTTP clients, calling other services | A blocking HTTP client first (`http.get`, `http.request`), raw TCP later. Planned (section 18.5) |
| `db` (P2/P3) | Almost every web application stores data | SQLite first (embedded, no server), then PostgreSQL. Only parameterized queries (`query(sql, [params])`), so SQL injection cannot be written. Rows as `Map[Text, Value]` until generics allow typed rows |
| Narrower capabilities (P3) | `fs` allows the whole file system; a hosted application should only touch its own directory | Effect parameters configured by the host, for example `fs` limited to a root directory and `net` to a list of hosts. Enforced in the runtime, visible in the signature |

### 3.5 The web layer (P2)

| Missing | Proposed shape |
|---|---|
| Handler contract | An entry point `fn handle(request: http.Request) -> http.Response uses ...` in place of `main`. The compiler checks its signature as it checks `main` today. The effects it declares are what the host must provide |
| Request and response types | Records in `std/http`: method (enum), path, query, headers (`Map`, case-insensitive names), cookies, body as `Bytes` with a `text_body` helper, remote address. Response: status, headers, body |
| Routing | A `match` on the path covers small applications; `std/http` helpers for path parameters (`/items/:id`) |
| Cookies and sessions | Cookie parsing and `Set-Cookie` building with secure defaults (`HttpOnly`, `Secure`, `SameSite=Lax`); signed session cookies need `std/crypto` |
| Request limits | Maximum body size, header count and time, set by the host, rejected before the handler runs |
| Streaming and large files | Later: a response body that is produced in parts, and serving files without loading them into memory |
| WebSockets, HTTP/2 | Out of scope at first; nginx and Apache handle HTTP/2 in front of the application |

### 3.6 Security building blocks (P2)

| Missing | Why |
|---|---|
| SHA-256, HMAC-SHA-256, constant-time comparison | Signed cookies, CSRF tokens, webhook signatures |
| Password hashing (Argon2id or bcrypt) | User accounts. Never to be written by hand |
| Base64 and hex | Tokens, data URLs, basic authentication |
| Secure random | Session IDs and tokens |
| CSRF protection and security headers | Defaults in `std/http`, so a model-written application is safe without extra work |

These must not be written in Tessera by a model. They belong in the runtime, implemented once and reviewed. That raises a project decision: the runtime has no dependencies today. The options are to implement the few primitives in the runtime (SHA-256 and HMAC are small; Argon2 is not), or to allow a small, audited set of Rust crates.

### 3.7 Build, packaging and hosting (P3)

| Missing | Why | Proposed shape |
|---|---|---|
| Library output | A server module loads a shared library, not an executable | `tessera1 build app.tes --library -o app.so`: no `main`, an exported handler, and a version symbol the host checks |
| A stable C interface | nginx and Apache modules are written in C | A small fixed interface: `tessera_handle(request bytes) -> response bytes`, with the request and response in a simple binary format. Generated code stays free of `unsafe`; the C-facing glue (raw pointers need `unsafe`) lives in one separate, reviewed adapter crate that is not generated |
| Thread model | The runtime uses `Rc`, which cannot be shared between threads. nginx workers are single-threaded processes; Apache's worker and event MPMs use threads | Every request runs entirely on one thread and shares nothing, which `Rc` allows. No change is needed as long as no state survives between requests outside effects |
| nginx module | A content handler that reads the body, calls the library and writes the response | In C, using the adapter. Blocking calls (database, `net`) must run in nginx's thread pool, not the event loop |
| Apache module | A `mod_tessera` content handler | In C. Simpler than nginx because Apache allows blocking handlers |
| Hot reload | Deploying a new version without restarting the server | Load the new library next to the old one; the version symbol prevents mixing |
| Configuration | Database location, secrets, allowed paths | From the host (nginx/Apache directives or environment variables), available through `env` |
| Packages | Sharing `std` extensions and application modules | Deferred (section 18). Vendored modules in the project work until then |

### 3.8 Testing and tooling (P2)

| Missing | Why | Proposed shape |
|---|---|---|
| Tests in the language | Model-written code needs tests next to it, and the harness tasks show models test by running programs | `test "name" { ... }` declarations with `assert`, run by `tessera1 test`. A test may only use effects the test runner provides (none, or a fake `db`) |
| Handler tests | Web code must be testable without a server | The test host from section 1: build an `http.Request`, call `handle`, inspect the `http.Response` |
| Formatter | Planned (section 14) | `tessera1 format`, mostly relevant for humans reviewing diffs |
| Harness tasks for applications | The 18 tasks are small command-line programs; every run now passes. They cannot show whether maps, JSON or handlers are easy for a model | Add tasks for maps, JSON, a handler with routing, form parsing and HTML escaping, and measure each feature as it is added |

## 4. Proposed order

Each phase ends in something runnable and measurable.

1. **Complete command-line applications** (P1): standard input, `Map`, the text functions, file errors as `Result`, `time` and `random`, runtime errors that unwind to the host. Harness: map and parsing tasks.
2. **Web core** (P2): standard library modules, `Bytes`, `std/json`, `std/url`, `std/html`, `log`, the handler contract with the test host and CGI, then `tessera1 serve`. Harness: handler tasks run through the test host.
3. **Data and security** (P2): `db` (SQLite), `std/crypto`, cookies and sessions, `Decimal`, date and time values, named functions as values (`sort_by`, routing tables).
4. **Server modules** (P3): library output, the C adapter, `mod_tessera` for Apache, then the nginx module with a thread pool for blocking effects, resource limits and narrowed capabilities.
5. **As measurement shows the need** (P4): user-defined generics, nested patterns, `Set`, constants, streaming bodies.

## 5. Decisions needed before building

1. Errors at the host boundary: unwinding runtime errors (recommended) versus a process per request.
2. Decimal numbers: `Decimal` (recommended) or `Float`, or both.
3. Dependencies in the runtime: implement crypto and SQLite access without crates, or allow an audited set (SQLite is impractical to write from scratch).
4. HTML safety: an escaping function, or an `Html` type whose interpolation escapes automatically.
5. The handler's binary interface between the C module and the Tessera library.
6. Whether `read_file`/`write_file` change to `Result` now (breaking) or get new names.
