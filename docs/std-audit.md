# Standard module audit

Step S0.2 of the DOOM roadmap, 2026-11-10, language draft 0.13.

The files in `std/` are modules, not programs: `tessera1 check std/json.tes` reports E0212 (no `main`), which is expected. Each was checked by importing it from a small program (`import std/<name>` plus a use) and through the existing tests. They are embedded into the compiler (`stage1/runtime_text.tes`), so `import std/...` works from any directory.

| File | Compiles | Tested by | Effects | Status and reason |
|---|---|---|---|---|
| `std/html.tes` | yes | `tests/stage1_run/std_modules.tes`, `tests/web/app.tes` | none | Working. `escape` only. |
| `std/http.tes` | yes | `tests/web/app.tes` (CGI, servers, host modules, `test` declarations), `tests/stage1_fail/E0213-*` | `net` (`fetch`, `get_text`); rest pure | Working. HTTP client is plain HTTP (no TLS), a documented interim choice of draft 0.13. |
| `std/json.tes` | yes | `tests/stage1_run/std_modules.tes`, harness task `json-total` | none | Working. Numbers are `Decimal`. |
| `std/url.tes` | yes | `tests/stage1_run/std_modules.tes` | none | Working. |

No file needs fixing or moving to `std/experimental/` (S3.1 is therefore only a check that new files keep this state). Stage 0 does not support modules and cannot compile any `std/` file; that is by design (stage 0 is frozen at draft 0.3).
