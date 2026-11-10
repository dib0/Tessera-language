# Toolchain status

Step S0.1 of the DOOM roadmap. Baseline taken on 2026-11-10 on the working tree at commit `8938862` (language draft 0.13, with the uncommitted draft 0.13 changes to `language-design.md`), rustc 1.99.1, Linux x86-64.

## Build

`./build.sh` succeeds: stage 0 (Rust), runtime embedding, the bootstrap compiler from the seed `bootstrap/tessera1.rs`, stage 1, the self-hosting check (stage 1 rebuilds itself to a byte-identical fixpoint) and `tools/xsend`.

## Test suite

`cd compiler && cargo test --release`: **18 passed, 0 failed** (about 75 seconds; `stage1_is_self_hosting` takes most of it).

| Test | Covers | Result |
|---|---|---|
| `suite::invalid_programs_report_the_expected_error` | 42 programs in `tests/compile_fail` (stage 0) | pass |
| `suite::notes_are_exported` | `notes` JSON export | pass |
| `suite::generated_rust_is_deterministic_and_forbids_unsafe` | byte-identical output, `#![forbid(unsafe_code)]` | pass |
| `suite::compiler_cli_exit_codes` | exit codes 0/1/2 | pass |
| `suite::fuzz_front_end_never_panics` | mutated sources | pass |
| `suite::multi_file_program_shares_one_namespace` | `tests/multi_file` | pass |
| `suite::programs_run_with_expected_output` | `tests/run`, `examples/valid` | pass |
| `suite::runtime_errors_stop_the_program_predictably` | 6 programs in `tests/runtime_fail` | pass |
| `stage1::stage0_and_stage1_programs_behave_identically` | shared programs, same output, messages and exit code | pass |
| `stage1::stage1_reports_the_same_error_codes` | `tests/compile_fail` through stage 1 | pass |
| `stage1::stage1_is_self_hosting` | stage 1 compiles itself | pass |
| `stage1::modules_compile_and_run` | `tests/modules/run` | pass |
| `stage1::module_errors_are_reported` | 13 cases in `tests/modules/fail` | pass |
| `stage1::embedded_runtime_is_up_to_date` | `stage1/runtime_text.tes` matches `runtime/` and `std/` | pass |
| `stage1::stage1_builds_the_graphical_tic_tac_toe` | `ui` effect | pass |
| `stage1::stage1_only_programs_run_with_expected_output` | 8 programs in `tests/stage1_run` | pass |
| `stage1::stage1_only_errors_are_reported` | 48 programs in `tests/stage1_fail` | pass |
| `stage1::web_application_runs_in_every_host` | `tests/web` (CGI, server, nginx/Apache hosts, `test` declarations) | pass |

Failing tests: none.

## Examples

Built with `bin/tessera1 build`:

| File | Builds | Runs | Notes |
|---|---|---|---|
| `examples/valid/calculator.tes` | yes | yes, exit 0 | output matches `calculator.out` |
| `examples/valid/receipt.tes` | yes | yes, exit 0 | output matches `receipt.out` |
| `examples/runtime/R001-out-of-bounds.tes` | yes | exit 70, R001 | as intended |
| `examples/invalid/E0204-shadowing.tes` | no, E0204 | — | as intended |
| `examples/invalid/E0301-undeclared-effect.tes` | no, E0301 | — | as intended |
| `examples/invalid/E0401-detached-note.tes` | no, E0401 | — | as intended |
| `examples/invalid/E0503-implicit-conversion.tes` | no, E0503 | — | as intended |
| `examples/tictactoe/tictactoe.tes` | yes | not run here (opens a window); built by the test suite | — |

## Formatting

`bin/tessera1 format --check` over `examples/`, `std/`, `stage1/` and `tools/`: no file would change.

## Notes for the roadmap

The roadmap's "Starting point" was written from draft 0.2. The repository is at draft 0.13, which already has most of Phase 1: `return`/`break`/`continue` (E0210), records with `with`, enums with exhaustive `match`, `Bytes`, `Result` with `try`, and modules. Phase 1 therefore only adds what is missing; see `language-design.md` section 3, draft 0.14.

## After Phase 1 (draft 0.14)

`./build.sh` (fixpoint reached) and `cargo test --release`: **19 passed, 0 failed**. The new test `stage1::stage1_only_runtime_errors_stop_the_program_predictably` runs the programs in `tests/stage1_runtime_fail`.

| Roadmap step | State | Tests |
|---|---|---|
| S1.1 Early exit | Existed (draft 0.3, E0210) | `tests/run/semantics.tes`, `tests/stage1_run/doom_core.tes` (early return from nested loops, `break` in nested loops, `continue` in `while`), `E0210-*`, `E0501-return-wrong-type` |
| S1.2 Records | Existed | `doom_core.tes` (nested records), `E0511-missing-field`, `E0511-duplicate-field`, `E0209-unknown-field-in-literal` |
| S1.3 Mutable record fields | **Added**: `set r.field = v` on a `var` | `set_places.tes`, `E0205-set-field-of-let`, `E0209-set-unknown-field`, `E0501-set-field-wrong-type`, `E0512-set-field-of-enum` |
| S1.4 Enums and `match` | Existed | `doom_core.tes`, `E0509-*` |
| S1.5 Mutable arrays | **Added** as `set xs[i] = v` on lists and `Bytes`, plus `repeat(list, n)`; no separate `Array[T]` (spec section 3) | `set_places.tes` (320x200 frame buffer filled and read back, checksum checked against Python), `R001-set-*`, `R018-set-byte-value`, `E0205-set-element-of-*`, `E0507-set-*` |
| S1.6 Bytes and bit operations | `Bytes` existed (`length`, `b[i]`, `slice`); **added** `bit_and`, `bit_or`, `bit_xor`, `shift_left`, `shift_right` | `doom_core.tes` (values checked against Python), `R020-shift-*`, `R002-shift-left-overflow` |
| S1.7 Recoverable errors | Existed (`Result`, `try`, draft 0.7) | `doom_core.tes`, `result_and_try.tes`, `E0508-unhandled-result`, `E0515`-`E0517` |
| S1.8 Modules | Existed (draft 0.5) | `tests/modules/` (two-file program, `E0603-cycle`) |
| S1.9 Generics | Not done (optional); still deferred in spec section 18 | — |
| S1.10 Overflow review | **Documented** in spec section 7.6 | `doom_core.tes` (products and quotients at ±2^31), `R002-fixed-mul-out-of-range` |

Performance: writing all 64,000 elements of a frame buffer 100 times takes about 11 ms (`--opt 3`).

## After Phase 2 (draft 0.15)

`cargo test --release`: **23 passed, 0 failed**. Programs and fixture in `tests/stage1_host/`.

| Roadmap step | State | Test |
|---|---|---|
| S2.1 `fs` read | **Added** `read_bytes(path) -> Result[Bytes, FileError]` (`read_file` already existed for text and stops with R006; `FileError` instead of `Text` matches `read_text`) | `binary_files_are_read_and_written_as_values`: a 268-byte fixture (`PWAD`, two little-endian `i32`, bytes 0 to 255) decodes to the values written by Python; a missing file gives `.Error(.NotFound)` |
| S2.2 `fs` write | **Added** `write_bytes(path, data) -> Result[Unit, FileError]` | same test: the written file is byte-identical to the changed buffer; writing into a missing directory gives an error value. `E0301-read-bytes-needs-fs`, `E0501-write-bytes-text`, `E0508-write-bytes-result-ignored` |
| S2.3 `env` | Existed (`args()`, `exit`, `exit_with`); `main` keeps no parameters (spec 9.3) | `arguments_come_in_and_exit_codes_go_out`: exit codes 3, 64 and 0 reach the shell |
| S2.4 `time` | Existed (`monotonic`, `sleep`, milliseconds) | `frame_pacing_keeps_35_frames_per_second`: 35 frames paced from the start time take 995 to 1,100 ms (measured: 1,001); 10 naive `sleep(35)` take 350 to 500 ms |
| S2.5 Console input | Existed (`read_line() -> Option[Text] uses io`) | `console_input_is_echoed_until_the_end`: empty lines, `\r\n`, a last line without a line break, and empty input |

The pacing test measures wall-clock time, so a heavily loaded machine could push it over its upper bounds.

## After Phase 3 (draft 0.16)

`cargo test --release`: **24 passed, 0 failed**. Module tests in `tests/std/` use `test` declarations and run through `tessera1 test`.

| Roadmap step | State | Check |
|---|---|---|
| S3.1 Clean up `std/` | Nothing to fix (see `std-audit.md`). `build.sh` and the runtime test now embed every file in `std/`, so a new module needs only a line in `stage1/main.tes` | all nine modules compile and are tested |
| S3.2 `std/bytes` | **Added**, plus `read_i16_le` (sign extension cannot use `shift_left`) | `tests/std/bytes_test.tes`: expected values from Python's `struct` for edge patterns (0, -1, ±2^15, ±2^31, unaligned) |
| S3.3 `std/fixed` | **Added**. `to_int` is called `floor_int` because `to_int` is the built-in for `Decimal` | All 168,678 entries of tables with 1, 3, 7, 360, 8192, 10240 and 65536 entries are within 0.5003 units of Python's `math.sin`/`math.cos`; `tests/std/fixed_test.tes` checks sample angles within one unit, exact quadrant values and the ±2^31 products |
| S3.4 `std/math` | **Added** | `tests/std/math_test.tes`: zero, negatives, `Int` limits, `isqrt` of 2^63 - 1 and of every value below 2000; R002/R010 cases |
| S3.5 `std/wad` | **Not a standard module** (game-specific; belongs to the DOOM port) | — |
| S3.6 `std/palette` | **Added** as a generic 768-byte RGB palette reader; `PLAYPAL` is that format | entries 0, 1 and 255 of a synthetic palette match Python |
| S3.7 `std/ppm` | **Added** as a pure `encode`; write with `write_bytes` | a 3 x 2 image is byte-identical to one built in Python; a 320 x 200 frame opened with ImageMagick (`identify`: PPM 320x200 8-bit sRGB) and showed the expected gradient and sine wave |
| S3.8 `std/text` | **No module**: `pad_left`, `pad_right`, `parse_int`, `split` are built-ins | `tests/std/text_builtins_test.tes`: empty input, negative and invalid numbers, `Int` limits |
