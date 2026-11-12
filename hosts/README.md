# Hosting Tessera web applications

A Tessera web application is a program with

```tessera
fn handle(request: http.Request) -> http.Response { ... }
```

The same source can be hosted in four ways:

| Way | Build | Process model | Good for |
|---|---|---|---|
| CGI | executable | one process per request | simplest setups, low traffic, shared hosting |
| FastCGI | executable, `app fastcgi --port N` | long-running process, server talks FastCGI to it | nginx / Apache without a native module |
| HTTP server | executable, `app serve --port N` (automatic under IIS HttpPlatformHandler) | long-running process behind a reverse proxy | any reverse proxy, IIS (recommended), containers |
| Native module | shared library, `tessera1 build app.tes --library` | inside the web server's worker processes | lowest overhead on Apache / nginx / IIS |

The executable modes (CGI, FastCGI, HTTP, `app request GET /path` for testing)
are implemented in `runtime/web.rs`. The shared library and the server
modules in this directory form the native-module route.

```
hosts/
  include/tessera.h                C ABI + wire format (+ small inline helpers)
  apache/mod_tessera.c             Apache httpd 2.4 module
  nginx/ngx_http_tessera_module.c  nginx module
  nginx/config                     nginx --add-module configuration
  iis/web.config                   IIS + HttpPlatformHandler (recommended on IIS)
  iis/web.cgi.config               IIS + CGI module
  iis/TesseraModule.cpp/.def       native IIS module (compiles; not run under IIS)
  iis/build.cmd                    MSVC build of the IIS module
  test/                            stub application, ABI test, server tests
runtime/adapter.rs                 C ABI of the library (the only unsafe Rust code)
```

## 1. CGI

The executable runs as a CGI program when `GATEWAY_INTERFACE` (`CGI/1.1`) is
set. One process per request: simple, isolated, but every request pays the
process start.

**Apache** (mod_cgi or mod_cgid):

```apache
ScriptAlias /app /srv/app/app.cgi
<Directory /srv/app>
    Require all granted
</Directory>
SetEnv TESSERA_TIME_LIMIT_MS 5000
SetEnv TESSERA_FS_ROOT /srv/app/data
```

**nginx** has no CGI support; use fcgiwrap (which turns FastCGI into CGI):

```nginx
location /app/ {
    include fastcgi_params;            # sets GATEWAY_INTERFACE, REQUEST_URI, ...
    fastcgi_param SCRIPT_FILENAME /srv/app/app.cgi;
    fastcgi_pass unix:/run/fcgiwrap.socket;
}
```

If you control the server, prefer FastCGI (below): it is the same nginx config
without fcgiwrap and without a process per request.

**IIS**: see `iis/web.cgi.config` (CGI module, plus the `isapiCgiRestriction`
entry it documents).

## 2. FastCGI

Start the application as a FastCGI responder (it listens on TCP):

```sh
TESSERA_TIME_LIMIT_MS=5000 ./app fastcgi --host 127.0.0.1 --port 9000
```

Keep it running with your service manager (systemd, supervisord, ...).

**nginx**:

```nginx
location /app/ {
    include fastcgi_params;
    fastcgi_pass 127.0.0.1:9000;
}
```

**Apache** (mod_proxy + mod_proxy_fcgi):

```apache
ProxyPass "/app/" "fcgi://127.0.0.1:9000/"
```

## 3. HTTP server behind a reverse proxy

```sh
./app serve --host 127.0.0.1 --port 8080
```

The port defaults to `HTTP_PLATFORM_PORT`, `ASPNETCORE_PORT` or `PORT` when
set, else 8080. Bind to 127.0.0.1 and let the proxy face the internet.

**nginx**:

```nginx
location /app/ {
    proxy_pass http://127.0.0.1:8080;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

**Apache** (mod_proxy + mod_proxy_http):

```apache
ProxyPass        "/app/" "http://127.0.0.1:8080/app/"
ProxyPassReverse "/app/" "http://127.0.0.1:8080/app/"
```

**IIS (recommended IIS route)**: HttpPlatformHandler, see `iis/web.config`.
IIS starts `app.exe` without arguments and sets `HTTP_PLATFORM_PORT`; the
runtime then automatically serves HTTP on `127.0.0.1:%HTTP_PLATFORM_PORT%`
and IIS proxies to it, restarts it when it exits, and stops it on app pool
recycle. `processesPerApplication` starts several processes. Install
"HttpPlatformHandler v1.2" first. Environment variables (limits) go in
`<environmentVariables>`.

## 4. Native modules (shared library)

### 4.1 Building the library

```sh
tessera1 build app.tes --library -o libapp.so
```

The compiler writes a build directory containing the generated `app.rs` and a
copy of `runtime/adapter.rs`, and runs

```sh
rustc --edition 2021 --crate-type cdylib -C opt-level=2 -C panic=unwind -o libapp.so adapter.rs
```

`-C panic=unwind` is required: runtime errors unwind to the request boundary
and become a 500; with `panic=abort` any runtime error would kill the server
worker.

**Windows (for the IIS module)**: the compiler runs on Windows too, so build
the `.dll` there with the same command, giving it the name the module expects
(`tessera_app.dll` by default; a name with an extension is used exactly as
given):

```sh
tessera1 build app.tes --library -o tessera_app.dll
```

rustc also writes `tessera_app.dll.lib` and `tessera_app.dll.exp` next to it;
IIS needs neither. To build the library on Linux for Windows instead, copy the
build directory (`app.rs` + `adapter.rs`) over and run rustc there, or
cross-compile (`rustup target add x86_64-pc-windows-msvc` plus an
MSVC-compatible linker and the Windows SDK libraries, e.g. lld-link with xwin);
the cross-compiled route is not tested here.

### 4.2 Apache: `mod_tessera`

Build (with apxs from the `apache2-dev` / `httpd-devel` package):

```sh
apxs -c -I hosts/include hosts/apache/mod_tessera.c
# or without apxs:
gcc -shared -fPIC -O2 -D_GNU_SOURCE -I hosts/include -I/usr/include/apache2 -I/usr/include/apr-1.0 \
    -o mod_tessera.so hosts/apache/mod_tessera.c -ldl
```

Configuration:

```apache
LoadModule tessera_module /usr/lib/apache2/modules/mod_tessera.so

# Environment for the applications, set in each child at start (server-wide).
TesseraEnv TESSERA_TIME_LIMIT_MS 5000
TesseraEnv TESSERA_FS_ROOT /srv/app/data

<Location /app>
    SetHandler tessera
    TesseraApplication /srv/app/libapp.so   # server or directory scope
    TesseraMaxBody 10485760                 # bytes; default 10 MiB; larger -> 413
</Location>
```

- Libraries are `dlopen`ed in every child process at `child_init` (after
  `TesseraEnv` is applied), so a graceful restart loads a replaced library.
  The ABI version is checked; load errors go to the error log and requests
  for that application get 500. A missing file is also warned about at
  startup.
- The application sees the raw (percent-encoded) path from the request line,
  `r->args` as query, `r->useragent_ip` as remote address (so mod_remoteip is
  honoured) and all request headers (lower-cased).
- Body read with `ap_setup_client_block`/`ap_get_client_block` (chunked
  bodies are de-chunked); `Content-Length` above the limit is refused before
  reading.
- Any MPM works. With event/worker, requests run concurrently in the same
  process; each call runs the application on its own thread in the library.

### 4.3 nginx: `ngx_http_tessera_module`

Build nginx with the module (static or dynamic):

```sh
./configure --with-threads --add-module=/path/to/hosts/nginx
# or: --add-dynamic-module=/path/to/hosts/nginx  and  load_module modules/ngx_http_tessera_module.so;
make && make install
```

Configuration:

```nginx
thread_pool tessera threads=16;              # main context, optional

http {
    tessera_env TESSERA_TIME_LIMIT_MS 5000;  # http level, applied in each worker at start
    server {
        location /app/ {
            tessera /srv/app/libapp.so;
            tessera_max_body 10m;            # default 10m; larger -> 413
            client_max_body_size 10m;        # nginx's own limit (default 1m!) applies first
            tessera_thread_pool tessera;     # default: "default" (implicit, 32 threads)
        }
    }
}
```

- `tessera_handle` blocks until the application returns. With `--with-threads`
  the call runs as a thread pool task and the response is sent from the
  completion handler in the event loop; the worker keeps serving other
  connections. **Without thread support the call is made directly and blocks
  the whole worker** (all its connections) for the duration of the request;
  `tessera_thread_pool` is then ignored with a warning. Use enough
  `worker_processes`, or better, build with `--with-threads`.
- Bodies that nginx buffered to a temporary file (bigger than
  `client_body_buffer_size`) are read back from the file before the task is
  posted.
- nginx clears the environment of its workers; the master's `TESSERA_*`
  variables only reach the application through `tessera_env` or nginx's
  `env NAME;` directive.
- Remote address is `$remote_addr` (connection address text; realip module
  is honoured). Path is the raw request-line path (`$request_uri` up to `?`).
- The response is sent without copying: the library's buffer is returned with
  `tessera_free` from a request pool cleanup.

### 4.4 IIS: native module (compiles; not run under IIS)

`iis/TesseraModule.cpp` is a small IIS 7+ native module (`CHttpModule`,
`OnExecuteRequestHandler`) written against the documented `httpserv.h` API.
`iis/build.cmd` compiles it cleanly with MSVC and the real `httpserv.h`
(Visual Studio 2022, Windows SDK 10.0.26100, `/W4`, no warnings), and the
resulting DLL exports `RegisterModule`. **It has still not been loaded into
IIS and served a request.** Prefer HttpPlatformHandler (section 3) unless you
need in-process hosting, and test it before use.

Behaviour: loads the application DLL once per worker process in
`RegisterModule` (path from the app pool environment variable
`TESSERA_APPLICATION`, default `tessera_app.dll` next to `TesseraModule.dll`),
checks the ABI version (errors go to the Windows Application event log,
source `TesseraModule`; requests then get 500), reads the body with
`IHttpRequest::ReadEntityBody` (limit `TESSERA_MAX_BODY_MB`, default 10 ->
413), builds the wire request from the raw URL, known + unknown headers and
the remote address, calls `tessera_handle`, and writes the response with
`SetStatus`/`SetHeader`/`WriteEntityChunks`.

Build (x64 Native Tools Command Prompt, Windows SDK installed):

```bat
hosts\iis\build.cmd
```

Registration (elevated prompt; adjust paths and names):

```bat
rem 1. register the module with IIS (adds it to <globalModules> and <modules>)
%windir%\system32\inetsrv\appcmd install module /name:TesseraModule /image:C:\inetpub\tessera\TesseraModule.dll

rem 2. tell the worker process where the application DLL is (IIS 10+)
%windir%\system32\inetsrv\appcmd set config -section:system.applicationHost/applicationPools /+"[name='DefaultAppPool'].environmentVariables.[name='TESSERA_APPLICATION',value='C:\inetpub\tessera\tessera_app.dll']" /commit:apphost

rem 3. map requests of the application to the module
%windir%\system32\inetsrv\appcmd set config "Default Web Site/app" -section:system.webServer/handlers /+"[name='Tessera',path='*',verb='*',modules='TesseraModule',resourceType='Unspecified',requireAccess='Script']"
```

Equivalent `applicationHost.config` fragments:

```xml
<system.webServer>
  <globalModules>
    <add name="TesseraModule" image="C:\inetpub\tessera\TesseraModule.dll" />
  </globalModules>
  <modules>
    <add name="TesseraModule" />
  </modules>
</system.webServer>
<system.applicationHost>
  <applicationPools>
    <add name="DefaultAppPool">
      <environmentVariables>
        <add name="TESSERA_APPLICATION" value="C:\inetpub\tessera\tessera_app.dll" />
        <add name="TESSERA_TIME_LIMIT_MS" value="5000" />
      </environmentVariables>
    </add>
  </applicationPools>
</system.applicationHost>
<location path="Default Web Site/app">
  <system.webServer>
    <handlers>
      <add name="Tessera" path="*" verb="*" modules="TesseraModule" resourceType="Unspecified" requireAccess="Script" />
    </handlers>
  </system.webServer>
</location>
```

The worker process bitness must match the DLLs (64-bit app pool, x64 DLLs).

## 5. The C ABI

Declared in `include/tessera.h`, implemented by `runtime/adapter.rs`:

```c
int32_t tessera_abi_version(void);   /* 1 */
int32_t tessera_handle(const uint8_t *request, size_t request_len,
                       uint8_t **response, size_t *response_len);
void    tessera_free(uint8_t *response, size_t response_len);
```

- `tessera_handle` returns 0 and a library-allocated response buffer, which
  the caller must release with `tessera_free(ptr, len)` (exactly the returned
  pointer and length, once). Application failures (runtime errors, limits,
  undecodable requests) are HTTP statuses (500, 400, ...) inside a normal
  response. Nonzero return: 1 = invalid arguments (NULL pointers), 2 =
  internal failure; `*response` is then NULL.
- It blocks until the application finishes and is thread-safe.
- Hosts must refuse a library whose `tessera_abi_version()` is not the
  version they were built for.

### What the library build needs from `app.rs`

`adapter.rs` contains `#[path = "app.rs"] mod app;` and uses exactly
`app::tessera_app` and `app::tw_library_handle`. So `app.rs` must:

- be a module file: inner attributes (`#![forbid(unsafe_code)]`,
  `#![allow(...)]`) are fine at its top, but no `fn main` is needed (an unused
  one would only warn), no `#![crate_type]`/crate-level-only attributes such
  as `#![no_std]` or `#![feature]`, and no `#[global_allocator]` conflicts;
- define `pub fn tessera_app(request: RawRequest) -> RawResponse` (the
  compiled `handle`, converted from/to `RawRequest`/`RawResponse`);
- define `pub fn tw_library_handle(app: fn(RawRequest) -> RawResponse, input: &[u8]) -> Vec<u8>`
  (provided by `runtime/web.rs`) and the `RawRequest`/`RawResponse` types it
  uses; items may use `crate::` paths only if they are written as
  `crate::app::...` (`app.rs` is no longer the crate root); plain
  relative paths are not affected.

### Wire format

All integers are u32 little-endian; `str`/`bytes` = u32 length + bytes;
strings UTF-8.

| request | response |
|---|---|
| u32 version (= 1) | u32 version (= 1) |
| str method | u32 status |
| str path (raw, percent-encoded, no query) | u32 header_count |
| str query (without `?`) | header_count x (str name, str value) |
| str remote_addr | bytes body |
| u32 header_count | |
| header_count x (str name, str value), names lower-cased | |
| bytes body | |

## 6. Security notes

- `runtime/adapter.rs` is the only Rust code of the runtime that contains
  `unsafe`: turning the caller's pointer/length into a slice, writing the
  out-parameters, and rebuilding the `Box<[u8]>` in `tessera_free`. It checks
  NULL pointers and wraps the call in `catch_unwind` (unwinding into C is
  undefined behaviour). The generated `app.rs` keeps `#![forbid(unsafe_code)]`.
- The server modules are C/C++ and run in the web server process; they
  only use bounds-checked reader helpers (`tessera_reader`) on the response.
- A native module shares the server worker's process: its user, its
  environment, its address space. Runtime errors are contained (500), but a
  Rust *abort* (stack overflow on the request thread, allocation failure)
  terminates the whole worker process; Apache and nginx start a new one, the
  other requests on that worker are lost. If that matters, prefer the
  FastCGI/HTTP process modes, which isolate the application in its own
  process.
- Limits are honoured by the runtime from the environment:
  `TESSERA_TIME_LIMIT_MS`, `TESSERA_MEMORY_LIMIT_MB`, `TESSERA_STACK_MB`,
  `TESSERA_MAX_BODY_MB`, `TESSERA_FS_ROOT` (file access confined to this
  directory), `TESSERA_NET_ALLOW` (allowed outgoing network destinations).
  Set them with `TesseraEnv` (Apache), `tessera_env` (nginx), app pool
  `environmentVariables` (IIS), `<environmentVariables>` (HttpPlatformHandler)
  or the process environment (CGI/FastCGI/HTTP). In a native module they are
  process-wide: all applications loaded into one server share them.
- Runtime error messages are written to stderr, which Apache and nginx
  redirect to their error log (verified on both).
- Keep the server's own body limits (`TesseraMaxBody`, `tessera_max_body` +
  `client_max_body_size`, IIS `maxAllowedContentLength`) in line with
  `TESSERA_MAX_BODY_MB`.

## 7. Threading

- Every `tessera_handle` call runs the application on its own thread inside
  the library; requests share no state, so concurrent calls are safe.
- Apache: prefork, worker and event MPMs all fine (handlers may block).
- nginx: use `--with-threads`; the thread pool size bounds the number of
  applications running at once per worker, further requests wait in the
  pool's queue (`thread_pool ... max_queue=`).
- IIS: the module calls the application synchronously on the IIS request
  thread.

## 8. Reloading

- Apache: `apachectl graceful`; nginx: `nginx -s reload`; IIS: recycle the
  application pool. New worker processes load the library again.
- Replace a library by writing the new file next to it and renaming it over
  the old one (`mv libapp.so.new libapp.so`). Do not overwrite a loaded `.so`
  in place (`cp new libapp.so`): processes that still map it can crash.
- Libraries are never `dlclose`d (unloading Rust libraries is not reliable);
  reloading relies on new worker processes.
- The ABI version check prevents loading a library built for an incompatible
  ABI; such a library is logged and its locations answer 500.

## 9. What was tested, and how

Environment: Linux 6.14 (Ubuntu 24.04), rustc 1.99.1, gcc 13.3.0,
Apache/2.4.58 (Ubuntu package binary, headers from `apt-get download
apache2-dev libapr1-dev libaprutil1-dev` + `dpkg -x`, no root),
nginx 1.30.6 (stable source from nginx.org), curl 8.5.0. Everything ran as a
normal user.

| Test | Command | Result |
|---|---|---|
| C ABI | `hosts/test/run_abi_test.sh` (builds `adapter.rs` + stub `test/app.rs` as cdylib, compiles `test/abi_test.c` with `-Wall -Wextra -Werror`, dlopens it) | PASS, 38 checks: ABI version; normal request (raw path, query, remote, lower-cased header, body, status 201, custom header); empty body; 5 MiB body echoed byte-exact; 404; runtime panic -> 500; garbage, wrong version, truncated and empty input -> 4xx; NULL out-pointers -> 1 and cleared; `tessera_free(NULL, 0)`; 16 threads x 25 concurrent calls |
| C ABI with the real runtime | `app.rs` = `runtime/core.rs` + `runtime/web.rs` (snapshot of 2026-11-08) + the stub's `tessera_app`, same build, same `abi_test` | PASS, 38 checks (so `adapter.rs` works with the real `tw_library_handle`) |
| Apache + nginx | `hosts/test/run_server_tests.sh` (builds library and modules, runs `test/http_test.sh` against each server) | PASS: Apache event MPM 12/12, Apache prefork MPM 12/12, nginx `--with-threads` 12/12, nginx without threads 12/12; both nginx builds compile under nginx's `-Werror`; Apache module compiles with `-Wall -Wextra -Werror`; no crashed workers |
| HTTP checks (`http_test.sh`) | curl | GET with raw path `%20`, query, custom header and `TesseraEnv`/`tessera_env` variable; POST body; chunked POST; body of limit-1 bytes echoed byte-exact (nginx: via its temp file); Content-Length and chunked body over the limit -> 413; 404 and 500 from the app; HEAD without body; unloadable library -> 500; 200 requests 20 at a time |
| Apache + nginx with the real runtime library | same `http_test.sh` with the core.rs+web.rs library | PASS 12/12 on each |
| Extra (manual) | | wrong ABI version library (C stub returning 2) rejected and logged, 500, on both; graceful restart / `nginx -s reload` picked up a library replaced by rename and a changed `TesseraEnv`; nginx with threads: a fast request during a 2 s request in the same single worker took 0.6 ms (without threads: 1.7 s, i.e. blocked, as documented); client abort during a running request (thread and non-thread builds) left the worker serving; `tessera_thread_pool` warning on the non-thread build |
| IIS | not testable here | `TesseraModule.cpp` only syntax-checked with mingw-w64 g++ 13 (`-fsyntax-only`) against a mock `httpserv.h`; `web.config` / `web.cgi.config` checked to be well-formed XML (`xmllint`). Nothing ran under IIS. |
