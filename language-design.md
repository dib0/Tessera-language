# Tessera Language Design

**Status:** Draft 0.25. Implemented by the stage 1 compiler in `stage1/` (written in Tessera, self-hosting, itself split into modules). The stage 0 compiler in `compiler/` (Rust) implements everything except the `ui` effect (section 9.4), modules (section 9.5), `Option` (section 5.5), `Result` and `try` (section 5.6), `with` (section 5.2), `.Variant` shorthand (section 5.2), text interpolation (section 4.5), `for _` (section 8.1), statement arms in `match` (section 7.5), `set x += e` (section 6.2), `+` on lists (section 7.1) the built-ins `parse_int`, `split`, `sort`, `index_of`, `contains`, `to_lower`, `to_upper` and `exit_with` (section 10), and everything added in drafts 0.13 to 0.20: `Map`, `Decimal`, `Bytes`, function values, `let ... else`, the effects `time`, `random`, `log`, `db` and `net`, web applications, `test` declarations, the standard modules, `set` on fields and elements, the bit operations, binary files, the standard modules added in draft 0.16, `Float`, the `audio` effect and the game modules of draft 0.20 (sections 5.1, 6.1, 6.2, 7.6, 9.6 to 9.8, 10), TLS and the crypto built-ins of draft 0.22, the standard modules `std/logging` and `std/config` of draft 0.23, the Model First Programming tools of draft 0.24 (section 20), and the interpreter and WebAssembly build of draft 0.25 (section 21). Both compilers run on Linux and on Windows, which are tested; macOS is unix like Linux and is expected to work apart from `ui`, but is not tested (draft 0.21, section 16.1).
**Scope:** Everything described here is implemented and tested unless marked *not implemented*. Anything not described here is not part of the language.

This document is the authoritative description of Tessera. When the compiler and this document disagree, the document wins and the compiler has a bug, or the document is updated through an explicit decision (see section 19).

---

## 1. Design principles

These principles decide trade-offs. When two choices are otherwise equal, pick the one higher on this list.

1. **Safe by construction.** Unsafe behavior must be impossible to write, not merely discouraged.
2. **Readable at review time.** A reviewer, normally a model, should understand what a function can do from its signature and what a line does without context from elsewhere.
3. **One way to write it.** Each concept has one syntax and one canonical formatting. Fewer choices means fewer mistakes and smaller, more predictable diffs.
4. **Explicit over implicit.** No implicit conversions, no hidden effects, no hidden mutation.
5. **Familiar over novel.** Where a widely known syntax exists (Rust, Kotlin, Python, Go), reuse it unless it conflicts with 1 to 4.
6. **Compact, but not cryptic.** Remove boilerplate, not words. Full words over symbols where the symbol is not universal.

## 2. A first look

```tessera
note "Keep the age policy in one review point."
fn is_adult(age: Int) -> Bool = age >= 18

fn label(name: Text, age: Int) -> Text {
let category = if is_adult(age) { "adult" } else { "minor" }
category + ": " + name
}

fn main() -> Unit uses io {
let people = ["Ada", "Linus", "Grace"]
let ages = [36, 17, 85]

for index in range(0, length(people)) {
print(label(people[index], ages[index]))
}
}
```

What a reviewer can read off this without further context:

- `is_adult` and `label` have no `uses` clause, so they cannot print, read files, or touch the network.
- `main` declares `uses io`, so it is the only place output can happen.
- Nothing is mutated: there is no `var` and no `set`.
- `people[index]` is bounds-checked; an invalid index stops the program with a clear runtime error instead of reading arbitrary memory.

A larger example with records, enums and `match` is `examples/valid/calculator.tes`.

## 3. Changes from earlier drafts

From the POC sketch in `poc-language-design.md`:

| Sketch | This design | Reason |
|---|---|---|
| `i64`, `bool`, `str`, `unit` | `Int`, `Bool`, `Text`, `Unit` | Full words, consistent capitalized type names (principle 6). `Int` is still 64-bit. |
| `c ? a : b` | `if c { a } else { b }` | One conditional form instead of two (principle 3). |
| `!{io}` | `uses io` | Readable without knowing the notation. |
| `age:i64` | `age: Int` | Canonical spacing improves diff readability. Token cost to be measured, not assumed. |

From draft 0.2 to 0.3, to make it possible to write a compiler in Tessera itself:

| Added | Why |
|---|---|
| Records (`type`) and enums with `match` | Syntax trees and tokens need structured data. |
| `return`, `break`, `continue` | Parsers have many early exits; nesting everything is less readable. |
| Effects `fs` and `env` | A compiler reads source files, writes output, and reads arguments. |
| Text and list built-ins (`code_points`, `slice`, `join`, ...) | Needed for lexing and code generation. |
| `fail(message)` | Pure code needs a way to stop on states that must be impossible. |
| Several files per program | A compiler is too large for one file; modules are still deferred. |

From draft 0.3 to 0.4, to let the stage 1 compiler run `rustc` and to build graphical programs:

| Added | Why |
|---|---|
| Effect `process` with `run_process` | The stage 1 compiler starts `rustc` itself. |
| `environment_variable` (effect `env`) | Lets a program read configuration such as `TESSERA_RUSTC`. |
| Effect `ui`, the `WindowEvent` type and drawing built-ins | Simple graphical programs (section 9.4). |
| `rgb(red, green, blue)` | Colors without hexadecimal literals. |

From draft 0.4 to 0.5:

| Added | Why |
|---|---|
| Modules: `import`, `pub`, qualified names (section 9.5) | Re-use across files with explicit, reviewable dependencies. |
| Import effect limits (`import json uses nothing`) | A dependency cannot quietly gain new capabilities. |
| `file_exists(path)` (effect `fs`) | The compiler reports a missing module as an error instead of crashing. |

From draft 0.5 to 0.6, the two most-requested features from writing the compiler (section 19):

| Added | Why |
|---|---|
| `Option[T]` with `Option.Some(value)` / `Option.None`, and `value_or` (section 5.5) | Replaces empty-or-one-element lists and hand-written enums as "maybe" values. |
| `record with { field: value }` (section 5.2) | Changing one field no longer means repeating every field. |

From draft 0.6 to 0.7:

| Added | Why |
|---|---|
| `Result[T, E]` with `Result.Ok(value)` / `Result.Error(error)` (section 5.6) | Recoverable errors as values, with the reason attached. |
| `try` (section 5.6) | Passes a failure up to the caller in one word, so code after it only sees the success value. |

From draft 0.7 to 0.8, the first two changes chosen with the measurement harness in `harness/` (section 19):

| Added | Why |
|---|---|
| `.Variant` where the type is known from context (section 5.2) | Removes the type name from about 97% of variant uses in the compiler, where it repeated what the context already said. |
| Text interpolation `"found \(count) items"` (section 4.5) | Replaces most `"..." + to_text(x) + "..."` chains. |

From draft 0.8 to 0.9, the two errors a model made in the first harness run (section 19):

| Changed | Why |
|---|---|
| `for _ in ...` for a loop variable that is not used (section 8.1) | The model wrote `for _k in range(0, n)`, as in Rust and Python. `_` already meant "unused" in `let _` and patterns; now it does for loop variables too. |
| A match arm may be one statement: `Kind.A => set total = total + 1` (section 7.5) | The model wrote this in a statement-position `match` and got E0110; the block `{ set ... }` added nothing. |
| Canonical layout has no indentation (section 14) | Indentation has no meaning in Tessera, and leaving it out made programs 12% fewer tokens with no loss in correctness. |

From draft 0.9 to 0.10, the code the model wrote most often in the harness runs (section 19):

| Added | Why |
|---|---|
| `parse_int(text) -> Option[Int]` (section 10) | 78 hand-written integer parsers made up 24% of all lines the model wrote. |
| `split`, `sort`, `index_of`, `contains` (section 10) | Splitting text, sorting and searching made up another 10%. `index_of` and `contains` work on lists and on `Text`, like `length`. |
| `set x += e`, `set x -= e` (section 6.2) | `set x = x + ...` was the most common statement; the short form does not repeat the name. `set` stays, so every change is still found by searching for `set`. |

From draft 0.10 to 0.11, the next most frequent patterns in the draft 0.10 run (section 19):

| Added | Why |
|---|---|
| `+` joins two lists of the same type (section 7.1) | `set xs = append(xs, item)` was the most common statement left; it becomes `set xs += [item]`, with the same in-place update. `append` and `concat` remain. |
| `exit_with(code, message)` (section 10) | Printing an error and then calling `exit(1)` appeared in almost every program, often in a helper function of its own. The message goes to standard error, as command-line convention expects. |
| `to_lower`, `to_upper` (section 10) | Written by hand in a third of the programs that handle words. |

From draft 0.11 to 0.12, two fixes found in the draft 0.11 run (section 19):

| Changed | Why |
|---|---|
| The `then` branch of an `if` can take its type from the `else` branch (section 7.3) | `if bad { .None } else { parse_int(text) }` was E0518 while the swapped form compiled; it was the only failed first attempt in the run. |
| `parse_int` no longer accepts a leading `+` (section 10) | Only `-`, so that `parse_int` accepts exactly what `to_text` produces; the model had added its own check for `+` because a task allowed only `-`. |

From draft 0.12 to 0.13, the application gaps P1 to P3 of [`application-roadmap.md`](application-roadmap.md) and the three findings of the draft 0.12 run:

| Added | Why |
|---|---|
| Runtime errors unwind to the host; stack, time and memory limits (section 12) | A runtime error used to end the process, which in a web server module ends a worker. Now the command line still exits with 70, while a web host answers 500 and continues. Deep recursion is error R013 instead of an abort. |
| `Map[K, V]` with literals `[k: v]` and `[:]` (sections 5.1, 10) | Sessions, headers, JSON objects and counting; the harness programs searched lists by hand. |
| `Decimal` with literals `19.99` (sections 4.5, 5.1) | Exact amounts (9 decimal places) for prices and JSON numbers; chosen over a binary float for predictability. |
| `Bytes` (section 5.1) | Request bodies, hashing and encodings work on bytes; `Text` stays valid UTF-8. |
| Functions as values, `fn(A) -> B` types, `map`, `filter`, `sort_by` (section 9.6) | Sorting records by a field, routing tables; and the draft 0.12 finding that 22 of 54 programs built a list only to `join` it. Named functions only, no closures. |
| `let x = value else stop` (section 6.1) | The draft 0.12 finding: 22 of 54 programs spent four lines on "the value, or stop". |
| Text, standard input, file (`Result`), time, random, log, database, network and crypto built-ins (section 10) | What every application needs; crypto and SQLite access live in the runtime, written once and reviewed, not by models. |
| Web applications: `fn handle(request: http.Request) -> http.Response` (section 9.7) | One program runs as CGI, as an HTTP server (also behind IIS), as a FastCGI server, and inside nginx, Apache and IIS as a shared library (`hosts/README.md`). |
| `test "name" { ... }` and `tessera1 test`; `tessera1 format` (sections 9.8, 14) | Tests next to the code; the canonical layout applied by the compiler. |
| Standard modules `std/http`, `std/json`, `std/url`, `std/html`, written in Tessera (section 10.1) | Reviewable library code, checked like any other module. |
| The `word-count` task no longer contradicts `to_lower` | The draft 0.12 finding: the task said only ASCII letters change case, so the model correctly avoided the built-in. |

From draft 0.13 to 0.14, the language core a DOOM port needs (Phase 1 of the DOOM roadmap; the rest of that phase, early exit, records, enums, `Result` and modules, already existed):

| Added | Why |
|---|---|
| `set` on a field or element of a `var`: `set player.health = 100`, `set pixels[y * 320 + x] = color`, `set world.things[i].x += 1` (section 6.2) | Game state and frame buffers change a few values at a time; rebuilding a 64,000-element list or a nested record for one change is slow and hard to read. Changes stay visible: only through `set`, only on a `var`. |
| `repeat(list, count)` (section 10) | Creates a buffer of a fixed size, such as `repeat([0], 320 * 200)`, with the existing name `repeat` (which already repeats text). |
| `bit_and`, `bit_or`, `bit_xor`, `shift_left`, `shift_right` (section 10) | Binary file formats, flags and fixed-point arithmetic. Functions, not operators: they are rare, and `and`, `or`, `not` stay boolean. |
| Safe range of 16.16 fixed-point arithmetic (section 7.6) | DOOM computes with 16.16 fixed-point numbers; the spec states when `Int` cannot overflow. |

Decided against, with the reasons:

- **A separate `Array[T]` type.** Lists are already values backed by one contiguous buffer, and `set xs[i] = v` changes one element in place when nothing else holds the list (section 6.2). A second sequence type would give two ways to write the same program (principle 3) and conversions between them, without making anything faster.
- **`byte_at` and `bytes_length`.** `Bytes` already has `b[i]` and `length(b)`.

From draft 0.14 to 0.15, host access for a DOOM port (Phase 2 of the DOOM roadmap):

| Added | Why |
|---|---|
| `read_bytes(path)`, `write_bytes(path, data)` (section 10) | WAD files, images and saved games are binary; `read_text` refuses data that is not UTF-8. Both return failures as values (`FileError`), like `read_text` and `write_text`. |

The rest of that phase already existed and is now covered by tests: command-line arguments (`args()`) and exit codes (`exit`, `exit_with`), a monotonic clock and `sleep` for frame pacing, and `read_line` for console input. `main` keeps taking no parameters: arguments come from `args()`, so the `env` effect stays visible in the signature (section 9.3).

From draft 0.15 to 0.16, general-purpose standard modules (Phase 3 of the DOOM roadmap). Formats that belong to one game, such as WAD files, stay in the game's own modules.

| Added | Why |
|---|---|
| `std/bytes`: little-endian numbers and zero-padded ASCII names in `Bytes` | Every binary file format needs them, and sign extension is easy to get wrong (section 7.6: `shift_left` never drops bits). |
| `std/math`: `min`, `max`, `clamp`, `abs`, `isqrt` | Written by hand in many programs; `isqrt` is easy to get subtly wrong at large values. |
| `std/fixed`: 16.16 fixed-point arithmetic and sine/cosine tables computed with integers only | Deterministic arithmetic for games and simulations (section 7.6), without floating point. |
| `std/palette`: 256-color RGB palettes | Indexed-color images (DOOM's `PLAYPAL`, PCX, GIF color tables) store palettes as 768 bytes. |
| `std/ppm`: binary PPM (P6) images | The simplest image format any viewer opens; lets a program show what it renders without a window. |

No `std/text` module: `pad_left`, `pad_right`, `split` and `parse_int` are already built-in functions (section 10). `parse_int` keeps returning `Option[Int]`; a `Result` would only repeat the input in its error.

From draft 0.16 to 0.17:

| Added | Why |
|---|---|
| `Float`, a 64-bit IEEE 754 number that is always finite (sections 4.5, 5.1, 10) | Geometry, physics, statistics and scientific code need fast approximate numbers; `Decimal` is exact but slow and limited to 9 decimals. |
| A number like `0.5` is a `Float` where one is expected, a `Decimal` elsewhere; exponents always mean `Float` | Programs written before draft 0.17 keep their meaning, and models can write `x * 0.5` naturally. |
| Infinity and NaN are runtime errors (R002, R003, R021) | As for `Int` overflow: a bad value stops the program where it appears instead of spreading silently. It also keeps `==` and sorting well defined. |
| `float`, `floor`, `ceil`, `sqrt`, `sin`, `cos`, `tan`, `atan2`, `exp`, `ln`, `pow`, `pi`, `parse_float`; `to_int`, `round`, `to_text`, `sort`, `sort_by` accept `Float` | The usual floating-point library. |

Decided against: a 32-bit float type (one floating-point type is enough, principle 3), `%` on `Float`, and keeping infinities and NaN (they make `==` non-reflexive and silently poison results).

The DOOM port uses 16.16 fixed point (decision D2, [`docs/doom-decisions.md`](docs/doom-decisions.md)). `+ - * / sqrt` are reproducible everywhere, but `sin`, `cos` and the other library functions can differ in the last bit between platforms, which demo playback (milestone M9) cannot tolerate unless angles come from tables.

From draft 0.17 to 0.18, the `ui` extension of DOOM decision D1 ([`docs/doom-decisions.md`](docs/doom-decisions.md)):

| Added | Why |
|---|---|
| `draw_pixels(x, y, width, colors, scale)` | A rendered frame is 64,000 pixels; one X11 request per pixel is far too slow. `scale` enlarges it in the runtime instead of in the program. |
| `poll_event()` | A game loop must not wait for input. |
| `WindowEvent.KeyUp(name)`, and the key names `Shift`, `Control`, `Alt` | Games act on keys that are held down. Existing `match`es on `WindowEvent` need a `KeyUp` arm (or `_`). |

From draft 0.18 to 0.19, sound for games:

| Added | Why |
|---|---|
| Effect `audio` with `play_sound(samples, rate, volume, pan)` (section 10) | Games and alerts need sound effects. The runtime mixes up to 16 sounds and streams them to the system's audio player (`aplay` or `pw-cat`) through a pipe, so it needs no libraries and no `unsafe`. Since draft 0.21 it reaches the sound card directly on Windows, which has no such player. |
| Key names `F1` to `F12`, `Delete`, `Home`, `End`, `PageUp`, `PageDown` (section 9.4) | Menus and games use function keys (save, load, quit). |

Decided against, for now: music. Playing MIDI-like music needs a synthesizer in the runtime; it can be added later as a separate decision.

From draft 0.19 to 0.20, general modules taken out of Tesseradoom (`examples/tesseradoom/`), which now uses them:

| Added | Why |
|---|---|
| `std/raster`: software images, fill, lines, blitting with transparency, remapping, bitmap fonts | Any program that draws its own pixels (games, plots, image tools) needs the same handful of operations. |
| `std/sound`: WAV files, tones, positional volume and pan, `play` | The built-in `play_sound` takes raw samples; these turn files and positions into them. |
| `std/keys`: held keys and recently typed text from window events | Every interactive program tracks held keys the same way; typed text gives shortcuts and codes. |
| `std/plane`: 2D fixed-point geometry (ray/segment, crossing, circle collision, nearest direction) | Collision, picking and sight lines in games and simulations, without overflow (section 7.6). |
| `std/rng`: a seeded, repeatable random number generator | Games, simulations and tests need the same numbers on every run and platform; `random_int` is secure and therefore not repeatable. |

Changed in draft 0.20, from the Tesseradoom findings (end of section 19):

| Changed | Why |
|---|---|
| `range(a, b)` is empty when `a >= b`, instead of R004 for `a > b` | Three crashes in Tesseradoom came from spans clipped to the screen that were empty; every other language with ranges treats them as empty. Stage 0 shares the runtime and changes with it. |
| The last use of a variable in a `return` or a function's final expression is moved, and `if`/`match` branches count as alternatives (section 6.2) | The forms that still copied (`return f(c)`, `set c = if a { f(c) } else { g(c) }`) cost Tesseradoom a factor of 20 until rewritten by hand. Rust checks every move in the generated code. |
| A rejected redefinition (E0204) still declares the new name | Later uses no longer produce follow-up errors that refer to the old variable. |

From draft 0.20 to 0.21, the same language on Windows as on unix (section 16.1). Nothing in the language changed; what changed is where the runtime can carry it out:

| Changed | Why |
|---|---|
| The `ui` effect also opens a native window on Windows (section 9.4) | The X11 client only speaks to an X server over a unix socket. The Windows runtime draws the same things with GDI and reports the same `WindowEvent`s, so a program compiles and behaves the same on both. |
| The `audio` effect plays through the sound card itself on Windows (section 10) | Windows has no `aplay`, and a language that needs an extra program installed before a game makes a sound is not usable there. |
| Generated Rust says `#![cfg_attr(windows, deny(unsafe_code))]` instead of `forbid` when, and only when, a program uses `ui` or `audio` (section 16) | A window and a sound card have no interface other than their C functions. `forbid` cannot be lifted anywhere in a crate, so those two programs say `deny` on Windows, where exactly one module per device is `#[allow(unsafe_code)]`. On every other system, and for every program without `ui` or `audio`, the blanket `forbid` is unchanged. |
| `build -o name` writes `name.exe` on Windows | Windows cannot start a program without the extension; `-o name.exe` is left exactly as given. The generated Rust is `name.rs` either way. |
| `\` counts as a path separator on Windows | A path typed with backslashes must find its modules. The path is otherwise kept exactly as typed, so a diagnostic points at what the person wrote. |

From draft 0.21 to 0.22, TLS and authenticated encryption:

| Added | Why |
|---|---|
| `net_request`, and so `std/http`'s `fetch` and `get_text`, accept `https://` URLs (section 10) | Almost every service a program talks to is only reachable over HTTPS; "use a reverse proxy" is no answer for a client. The runtime speaks TLS 1.3 and checks the server's certificate chain against the system's trusted certificates, its dates and its host name. There is no switch to skip those checks. |
| `app serve --tls-cert FILE --tls-key FILE` (section 9.7) | A web application can serve HTTPS itself, for development or small deployments, without nginx, Apache or IIS in front. |
| Built-ins `sha512`, `hmac_sha512`, `hkdf_sha256`, `aead_seal`, `aead_open` (section 10) | Key derivation and authenticated encryption, which TLS needs anyway, for programs: encrypted cookies, tokens and files. |
| `std/crypto`: `new_key`, `derive_key`, `encrypt`, `decrypt`, `encrypt_text`, `decrypt_text` (section 10.1) | The built-ins are primitives a program can misuse (a repeated nonce breaks ChaCha20-Poly1305). The module fixes the one safe combination: a random nonce stored with the ciphertext, and a context that binds a value to its use. |

All cryptography is in the runtime (`runtime/crypto.rs`, `runtime/tls.rs`), written once, in safe Rust, without dependencies, and tested against the published test vectors and against OpenSSL and curl in both directions. Code that handles secrets runs in constant time: AES is computed without lookup tables, and the field arithmetic selects with masks instead of branches.

Decided against, with the reasons:

- **Running `curl` or `openssl` as a child process**, the way SQLite is reached. Small, but it makes HTTPS depend on a program being installed and on its version, and the server side would still need its own TLS.
- **Allowing crates such as rustls.** The runtime and the generated programs are built with `rustc` alone, without Cargo; a dependency would change how every program is built.
- **TLS 1.2 and older.** SSL 2 and 3 and TLS 1.0 and 1.1 are broken or deprecated (RFC 8996). TLS 1.2 would double the protocol code and its cipher-suite pitfalls for the few servers that lack TLS 1.3; the error message for such a server says so.
- **An option to skip certificate checks.** It is the usual way a test setting reaches production. A test server's certificate is trusted by pointing `TESSERA_CA_FILE` at it instead.
- **Session resumption, 0-RTT and client certificates.** Resumption and 0-RTT save one round trip at the price of state and replay risks; client certificates have no use yet.

From draft 0.22 to 0.23, logging and configuration (section 10.1). Nothing in the language changed; both are standard modules written in Tessera:

| Added | Why |
|---|---|
| `std/logging`: `Level`, `Format`, `Logger`; `new`, `with_field`, `with_level`, `parse_level`, `level_name`, `enabled`, `render`, `write`, `debug`, `info`, `warning`, `error` | Every server and tool ends up writing its own level check and its own `key=value` formatting around `log`, usually without escaping, so a newline in a user value forges a log line. The module fixes one format (text in logfmt style, or JSON lines) with every value escaped. There are no global variables, so a logger is a value that is passed along, which also makes the context of a line (a request id) explicit. |
| `std/config`: `Config`; `empty`, `from_map`, `parse`, `load`, `merge`, `with_value`, `environment_name`, `from_environment`, `has`, `find`, `names`, `section`, `origin`, `get_text`, `get_int`, `get_decimal`, `get_float`, `get_bool`, `get_list`, `encode` | Programs need their database location, port and log level from a file, with defaults and with overrides from the environment (containers, IIS, systemd). Every value remembers where it came from, so an error says ``setting `server.port` (app.conf line 3) is "80x", expected a whole number`` instead of only that something is wrong. |

Decided against, with the reasons:

- **A global or implicit logger.** It would need global state, which the language does not have (section 9.7). Passing a `Logger` costs one parameter and shows in the signature which functions can log, next to `uses log`.
- **Logging functions without a fields argument, next to ones with.** Two ways to say the same thing (principle 3); `[:]` for no fields is short and the type is known from the parameter.
- **Timestamps in the line.** The host's log (nginx, Apache, journald) adds them, and adding one needs the `time` effect in every function that logs. A program writing its own file adds `format_time(now())` as a field.
- **TOML, YAML or JSON as the configuration format.** TOML and YAML need far more parser for nesting and types that settings rarely use; JSON allows no comments. `key = value` with sections covers settings, and the types are given by the getter, so `port = 8080` and `port = "8080"` are the same.
- **Inline comments after a value.** `#` is common in URLs and passwords, and a rule for when it starts a comment is a trap; comments are whole lines.
- **Reading every environment variable with a prefix.** There is no built-in to list the environment, and reading only the keys that have a default means a typo in a variable name is not silently a new setting.
- **Getters with a default argument** (`get_int(c, key, 8080)`). Defaults would be spread over the code; with `from_map` and `merge` they sit in one place, and a missing key is an error that names it.

From draft 0.23 to 0.24, **Model First Programming** (section 20): the tools around the language are built for a model that writes and repairs code, not for a person at an editor. The language itself (syntax, types, semantics) did not change.

| Added | Why |
|---|---|
| The syntax tree is the program; its canonical text is printed from it (sections 14 and 20.1). `tessera1 format` is now a full printer, `tessera1 ast` writes the tree as JSON, `tessera1 render` prints JSON as text, and every command accepts a `.json` tree | Exactly one textual form per program: no formatting variance between models or runs, and diffs show only real changes. Text and tree convert into each other without loss, comments included, which every file in the repository is tested for. |
| Edit operations (`tessera1 edit`, section 20.2): `add_fn`, `add_item`, `replace_fn`, `replace_item`, `remove_fn`, `remove_item`, `rename_symbol`, `set_effect`, `add_match_arm`, `insert_stmt_before`, `insert_stmt_after`, `replace_stmt`, `remove_stmt` | Small typed changes instead of find-and-replace on text. Each operation finds its target in the tree, the changed files are printed canonically, and the whole program is checked before a file is written: a batch applies completely or not at all. `rename_symbol` follows the program's types, so it renames a field or variant also where it is used through a `.Variant` shorthand or a module alias. |
| The grammar is exported (`tessera1 grammar`, section 20.3): GBNF for the canonical text, a JSON Schema for the tree (recursive, or unrolled to a fixed depth with `--depth`), and a JSON Schema for the edit operations | Grammar-constrained decoding: a decoder that follows the grammar cannot produce a syntax error. Claude's structured outputs accept no recursive schema, which is why the tree schema can be unrolled. |
| Diagnostics carry `expected`, `actual` and machine-applicable `fixes` (section 20.4); `tessera1 fix` applies them | The repair is in the error. The text output shows each fix as the line it produces; `fix` applies every unambiguous fix and repeats until none is left. |
| New codes E0701 to E0706 for invalid trees and refused edit operations (section 13) | Every refusal has a stable code and a concrete help line, like any other diagnostic. |

Decided against, with the reasons:

- **Keeping text as the source of truth with a lenient formatter.** The old `format` only removed indentation, so two programs with the same tree could still differ in spacing, parentheses, `x = x + 1` against `x += 1`, or `"a" + to_text(n)` against `"a\(n)"`. With the tree as the truth each of those has one written form.
- **Comments as tree nodes in the language.** Comments stay outside the grammar of expressions; the lexer collects them and the printer puts each one back above the node it belongs to. In the JSON tree they are a `comments` list on that node. A comment that stood at the end of a code line moves to its own line above the next statement.
- **Line-based edits (`replace lines 10 to 14`).** Line numbers change with every edit and are exactly what models get wrong; anchors are the canonical first line of a statement, which a model can quote and which is unique or refused.
- **Applying edits that do not check.** An edit operation that leaves an error refuses the whole batch and reports the error in the would-be text, so a program on disk always checks after an edit.
- **Fixes for every diagnostic.** A fix is only given where one change is clearly right (`let` to `var`, a missing `uses io`, `len` to `length`, `float(n)`). Where a choice is needed (`p.z` when both `x` and `y` exist, a non-exhaustive `match`) the diagnostic keeps its help text and offers no fix, so `tessera1 fix` never guesses.
- **A machine-readable specification generated from the compiler, contracts and property tests** (steps 5 and 6 of the MFP plan) are deferred until the tools above are measured (section 18).

From draft 0.24 to 0.25, **Tessera in the browser** (section 21): a model generates a program, as text or as its syntax tree, and a web page runs it, with no server and no Rust toolchain. The language itself did not change.

| Added | Why |
|---|---|
| `tessera1 interpret <files...> [-- <args>]` runs `main` straight from the checked syntax tree (section 21.1), in the effects `io`, `env`, `time`, `random` and `log` | A browser cannot run `rustc`. The interpreter is written in Tessera, in `stage1/interp.tes`, so it is part of the compiler and travels with it. Built-in functions are called directly, so their results are the compiled ones by construction; every runtime error is checked by the interpreter first, so it carries the program's location. It behaves exactly as the compiled program: the same output, error messages and exit code, which a test checks for every test program. |
| E0707: `interpret` refuses a program whose `main` uses another effect | `main` declares every effect the program uses (section 9.2), so its `uses` clause decides before anything runs. |
| `tessera1 build <files> --wasm -o x.wasm` (section 21.3): a program with `main` as a WebAssembly module for a JavaScript host, with `runtime/wasm.rs` as its interface | The compiler is such a program: `tessera1 build stage1/main.tes --wasm` gives `browser/tessera.wasm`, the whole command line (`check`, `format`, `ast`, `render`, `fix`, `edit`, `grammar`, `interpret`) in one module. |
| `browser/tessera.js` (section 21.4), a JavaScript library without dependencies around that module | `run(program)` takes Tessera text or a syntax tree and returns the output, the exit code and, when the program did not compile, the diagnostics as JSON (section 20.4), so a model can repair and retry inside the page. |

Decided against, with the reasons:

- **A second code generator that writes JavaScript.** It would run faster, but it would be a third implementation of the semantics (overflow, evaluation order, error messages) next to the Rust runtime, in a language that has no 64-bit integers. The interpreter reuses the runtime's built-ins and is checked against the compiled programs.
- **Compiling on a server and running the result in the page.** That is not self-contained: every page would need the server, and a program could not be checked and repaired offline.
- **The interpreter in JavaScript, working on the JSON tree.** The tree carries no types; the checker would have to run first anyway, and the interpreter would be a third implementation again.
- **`fs`, `net`, `db`, `process`, `ui` and `audio` in the browser.** The host already offers files to the compiler, so `fs` could follow; the others need asynchronous browser interfaces that a blocking program cannot wait for. Deferred until measured.

## 4. Source files and lexical structure

### 4.1 Files

- Source files use the extension `.tes` and must be valid UTF-8.
- A program starts at its **entry file**, the first file given to the compiler, which defines `main`. Other files are **modules**, loaded through `import` (section 9.5).
- Further files given on the command line together with the entry file form one entry module and share its namespace. This is how stage 0 builds multi-file programs; with stage 1, prefer modules.
- The compiler rejects files containing a byte order mark, tab characters, or carriage returns (E0101).

### 4.2 Comments and notes

```tessera
// A comment explains code to the reader. It carries no meaning for the compiler.

note "A note records design intent. Notes are part of the program's metadata."
```

The difference is deliberate: comments are for reading, notes are for tooling. See section 11. Comments are kept by every tool that rewrites a file (section 14): each one stays above the statement or declaration it stands above, and a comment at the end of a line moves to its own line.

There are no block comments (`/* */`). Nesting rules and unterminated comments are a known source of parser bugs and confusing diffs.

### 4.3 Identifiers and naming

| Kind | Style | Example |
|---|---|---|
| Functions, parameters, variables, fields | `snake_case` | `total_price`, `is_valid` |
| Types, enum variants | `PascalCase` | `Token`, `TokenKind`, `LeftParen` |
| Effects | lowercase single word | `io` |

Naming style is **enforced by the compiler** (E0207, an error, not a warning). Consistent names make code easier for a model to predict when it writes and to check when it reviews. The case of the first letter is also how the parser tells types (`Token { ... }`, `TokenKind.Plus`) from values.

Identifiers are ASCII letters, digits, and `_`, starting with a letter. `_` alone is not a name but marks a binding that is not used (`let _`, `for _`, `Type.Variant(_)`); it cannot be read (E0110), and `_x` is rejected (E0103) so there is one way to say "unused". Non-ASCII identifiers are rejected (E0103) to avoid homoglyph attacks (`а` Cyrillic vs `a` Latin). Text literals may contain any Unicode.

### 4.4 Keywords

```
fn  type  enum  let  var  set  if  else  match  for  in  while
return  break  continue  note  uses  and  or  not  true  false
import  pub  with  try
```

Reserved for future use (E0108 when used as a name): `module`, `unsafe`, `loop`, `const`, `trait`, `impl`. (Stage 0 also reserves `import`, `pub` and `try`.)

### 4.5 Literals

| Literal | Examples | Notes |
|---|---|---|
| Int | `0`, `42`, `1_000_000` | Decimal only. `_` only between digits. Leading zeros rejected (`007` is an error). |
| Bool | `true`, `false` | |
| Text | `"hello"`, `"line\n"`, `"\(count) items"` | Escapes: `\n`, `\t`, `\\`, `\"`, `\u{1F600}`. `\(expression)` inserts the value of an `Int`, `Bool` or `Text` expression (stage 1); the expression may contain calls and its own text literals; braces have no special meaning. Must close on the same line. No raw control characters. |
| Unit | `()` | Rarely written; it is the value of statements. |
| List | `[1, 2, 3]`, `[]` | Elements must share one type. An empty list needs a known type from context (section 5.3). |
| Map | `["ada": 36, "linus": 54]`, `[:]` | Keys `Int` or `Text`, values of one type. A key given twice keeps the last value. An empty map needs a known type (E0505). |
| Decimal | `19.99`, `0.5` | Digits, a point and 1 to 9 digits (E0106 otherwise). Negative values use unary `-`. |
| Float | `0.5` where a `Float` is expected, `6.02e23`, `1e-9`, `2.5E+3` | A number with a point is a `Float` when the context expects one (section 5.1), with any number of decimals; a number with an exponent is always a `Float`. Its magnitude must be below 10^308 (E0106). |

Negative numbers are the unary `-` applied to a literal. `-9223372036854775808` is accepted so the full `Int` range is expressible.

### 4.6 Statement separation

Statements end at a newline. There are no semicolons. A line continues onto the next line only when:

- it ends inside an open `(` or `[`, or
- it ends with a binary operator (`+`, `and`, `==`, ...), `=`, `+=`, `-=`, `->`, `=>`, `.`, a comma, or an opening bracket.

```tessera
let total = base_price
+ shipping        // ERROR: the line above is already a complete statement

let total = base_price +
shipping          // OK
```

`else` must be on the same line as the closing `}` of the `if` block (E0111).

## 5. Types

### 5.1 Built-in types

| Type | Meaning |
|---|---|
| `Int` | Signed 64-bit integer. Overflow is a runtime error, never wraps silently. |
| `Bool` | `true` or `false`. |
| `Text` | Immutable UTF-8 text. Length and positions count Unicode scalar values, not bytes. |
| `Unit` | The type with one value `()`. Returned by functions that only perform effects. |
| `List[T]` | Ordered, bounds-checked sequence of `T`. A value: elements of a list in a `var` can be changed with `set xs[i] = v` (section 6.2), but never through another name. |
| `Option[T]` | Either a value of type `T` or no value (section 5.5). |
| `Result[T, E]` | Either a value of type `T` or an error of type `E` (section 5.6). |
| `Map[K, V]` | Immutable map from `Int` or `Text` keys (E0208 otherwise) to values, ordered by key, so iteration and output are deterministic. |
| `Float` | IEEE 754 64-bit floating-point number, always finite: an operation whose result would be infinite or not a number stops with a runtime error instead (R002, R003, R021), so `==` and `<` behave as for other numbers. See below. |
| `Decimal` | Exact fixed-point number with 9 decimal places, range about ±1.7 × 10^29. `+ - * /` and comparisons; multiplication and division round the last place half away from zero; overflow is R002 and division by zero R003. Never mixed with `Int` (`decimal(n)`, `to_int(d)`). |
| `Bytes` | Sequence of bytes 0 to 255: `length`, indexing (an `Int`), `slice`, `+`, `==`, and `set b[i] = v` on a `var` (R018 outside 0 to 255). |
| `fn(A, B) -> C uses e` | A function value (section 9.6). |
| `FileError` | Built-in enum `NotFound`, `PermissionDenied`, `Other(message: Text)`, the error of the file functions that return a `Result`. |
| `Database` | An open SQLite database (effect `db`, section 10). |

**`Float` and `Decimal`.** Both are written with a decimal point. A literal such as `0.5` is a `Float` where a `Float` is expected: an annotation (`let x: Float = 0.5`), a parameter, a field, a return value, the other operand of an arithmetic operator or comparison with a `Float` (`speed * 0.5`, `0.5 * speed`, `x < 1.0`), or an element of a list literal that also holds a `Float` (`[0.5, 1e3]`). Everywhere else it is a `Decimal`, as before draft 0.17: `var total = 0.0` is a `Decimal`, so write `var total: Float = 0.0` (the E0503 message says so). A literal with an exponent (`1e-9`) is always a `Float`, so `let epsilon = 1e-9` needs no annotation. Use `Decimal` for amounts that must be exact (money, JSON numbers); use `Float` for measurements, geometry and science. The two never mix, and neither mixes with `Int`: convert with `float(n)` and `to_int(x)`.

`Float` rules:

- `+`, `-`, `*`, `/`, unary `-` and the comparisons work as in IEEE 754 (results rounded to the nearest representable number). `%` is not defined (E0503).
- A result too large to represent is R002 (overflow); division by zero is R003; an operation with no real result, such as `sqrt(-1.0)` or `ln(0.0)`, is R021. Infinity and NaN therefore never appear in a program, `==` is reflexive, and `sort` is well defined.
- `-0.0` equals `0.0`; `to_text` shows it as `-0.0`.
- `+ - * / sqrt` give the same result on every platform. `sin`, `cos`, `tan`, `atan2`, `exp`, `ln` and `pow` come from the platform's math library and can differ in the last bit between platforms. Programs that must compute identically everywhere (game demos, simulations replayed elsewhere) use `Int` or 16.16 fixed point (`std/fixed`).
- `Float` cannot be a `Map` key (E0208).

Internally there is also a `Never` type for expressions that do not produce a value (`return`, `break`, `continue`, `fail(...)`, `exit(...)`). It cannot be written in source but fits wherever any type is expected, so `if bad { fail("...") } else { 42 }` is an `Int`.

### 5.2 Records and enums

A **record** groups named fields:

```tessera
type Token {
kind: TokenKind
text: Text
line: Int
}

let token = Token { kind: TokenKind.Name, text: "main", line: 1 }
print(token.text)
```

- Fields are written one per line, without commas.
- A record literal must give every field exactly once (E0511). Fields may be given in any order and are evaluated in the written order.
- Records are values. A field of a record in a `var` can be changed with `set token.line = 2` (section 6.2); otherwise make a changed copy with `with`:

  ```tessera
let moved = token with { line: token.line + 1 }
let checker = state with { locals: [], loop_depth: 0 }
```

  `value with { ... }` names one or more fields of the record type of `value`, each at most once (E0209 for unknown fields, E0511 for duplicates or an empty list). The value is evaluated first, then the new field values in written order. Fields not mentioned keep their value. `with` binds like a call or field access, so `make_token() with { line: 1 }` and `checker.state with { failed: true }` work without parentheses. Using `with` on anything other than a record is E0512.

An **enum** is a value that is one of several variants, each with its own fields:

```tessera
enum Expr {
Number(value: Int)
Add(left: Expr, right: Expr)
Missing
}

let sum = Expr.Add(Expr.Number(1), Expr.Number(2))
```

- Variants are written with their type, `Expr.Number(1)`, unless the type is already known from the context. Then `.Number(1)` is enough (stage 1): in patterns, function arguments, return values, annotated bindings, record fields, elements of a list of known type, and on the right side of `==` and `!=`. The same shorthand works for `Option` and `Result` (`.Some(x)`, `.None`, `.Ok(v)`, `.Error(e)`). Where no type is known, such as `let c = .Red` or `.Red == c`, it is error E0518. A reader always finds the type in the surrounding signature, annotation or matched value; there are no bare variant names.
- Variant fields are given by position when constructing and read with `match` (section 7.5). Enum values have no `.field` access.
- Types may refer to themselves and to each other in any order (recursive syntax trees are fine).
- `==` and `!=` compare records and enums structurally.

User-defined generic types are deferred. There is no built-in optional or result type yet; declare a small enum where one is needed.

### 5.3 No implicit conversions

Nothing converts automatically. `"Age: " + 20` is a type error (E0503); write `"Age: " + to_text(20)`. `if 1 { ... }` is a type error (E0504); conditions must be `Bool`.

### 5.4 Type inference

- Function parameters and return types are **always written**. A signature is the review contract; it must be readable without inference.
- `let` and `var` types are inferred from the initializer and may be written explicitly: `let count: Int = 0`.
- An empty list takes its type from context: an annotation, a parameter type, a return type, or the other argument of a built-in. Without context it is error E0505.

### 5.5 Optional values

`Option[T]` is a built-in type for "a value, or nothing". It behaves like this enum, except that it works for every `T`:

```tessera
enum Option[T] {
Some(value: T)
None
}
```

```tessera
fn find_index(items: List[Text], wanted: Text) -> Option[Int] {
for index in range(0, length(items)) {
if items[index] == wanted {
return Option.Some(index)
}
}
Option.None
}

fn describe(position: Option[Int]) -> Text {
match position {
Option.Some(index) => "found at " + to_text(index)
Option.None => "missing"
}
}
```

`value_or(find_index(names, "Ada"), -1)` gives the index, or `-1`.

- Values are written `Option.Some(value)` and `Option.None`, like other enum variants.
- `Option.None` needs its type from context, like `[]` (E0505 otherwise): a return type, a parameter, an annotation, or the other side of `==`.
- `match` on an `Option` must cover both variants (E0509).
- `==` compares options by value.
- `value_or(option, default)` returns the value or the default.
- `Option` cannot be redefined (E0201).

For operations that can fail with a reason, use `Result` (section 5.6).

### 5.6 Recoverable errors: `Result` and `try`

`Result[T, E]` is a built-in type for "a value, or an error explaining why not". It behaves like this enum for every `T` and `E`:

```tessera
enum Result[T, E] {
Ok(value: T)
Error(error: E)
}
```

The error type is chosen by the program: a `Text` message, or better an enum that callers can `match` on.

```tessera
enum ParseError {
Empty
NotADigit(position: Int)
}

fn parse_number(text: Text) -> Result[Int, ParseError] {
let codes = code_points(text)
if length(codes) == 0 {
return Result.Error(ParseError.Empty)
}
var value = 0
for index in range(0, length(codes)) {
let digit = codes[index] - 48
if digit < 0 or digit > 9 {
return Result.Error(ParseError.NotADigit(index))
}
set value = value * 10 + digit
}
Result.Ok(value)
}

note "`try` returns the first error; the rest of the function only sees numbers."
fn add_all(texts: List[Text]) -> Result[Int, ParseError] {
var total = 0
for text in texts {
set total = total + try parse_number(text)
}
Result.Ok(total)
}
```

- Values are written `Result.Ok(value)` and `Result.Error(error)`. Each fixes only half of the type, so the other half must come from context: a return type, a parameter or an annotation (E0505 otherwise).
- `match` on a `Result` must cover `Ok` and `Error` (E0509). `==` compares by value.
- **`try value`** unwraps a `Result.Ok`, or immediately returns the `Result.Error` from the current function. The function must return a `Result` (E0516) whose error type the error fits (E0517); there are no implicit error conversions. `try` binds like `not` and `-`: `try parse(x) + 1` adds 1 to the parsed value.
- `try` also works on an `Option` inside a function that returns an `Option`: `Option.None` is returned immediately.
- `try` on anything else is E0515. `Result` cannot be redefined (E0201).
- `Result` is for expected failures. `fail(...)` and runtime errors remain for situations that indicate a bug (section 12).

## 6. Bindings and mutation

### 6.1 Immutable bindings with `let`

```tessera
let greeting = "hello"
let scores: List[Int] = []
let _ = compute_and_discard()
```

A `let` binding can never change. `let _ = ...` evaluates a value and explicitly discards it.

```tessera
let count = parse_int(text) else exit_with(1, "error: not a number")
let row = first_row(rows) else return http.not_found()
```

`let x = value else stop` takes the value out of an `Option` (`.Some`) or `Result` (`.Ok`, the error is discarded); otherwise it evaluates `stop`, which must have type `Never`: `return`, `break`, `continue`, `fail(...)`, `exit(...)` or `exit_with(...)` (E0501). It replaces a four-line `match` that names the value only to pass it on.

### 6.2 Mutable bindings with `var` and `set`

```tessera
var total = 0
for price in prices {
set total = total + price
}
```

Mutation is visible twice: at the declaration (`var`) and at every change (`set`). A reviewer can search for `set` to find every mutation in a file.

Rules:

- `set` only works on a `var` (E0205). Parameters and loop variables are never mutable.
- A `var` that is never `set` is a compiler error (E0206; use `let`).
- Writing `name = value` without `set` is a syntax error with a hint.
- `set x += e` is short for `set x = x + e`, and `set x -= e` for `set x = x - e`; they are checked as the long form, so `+=` works on `Int` and `Text` and `-=` on `Int`.
- Lists, text, maps, bytes and records are values: assigning one to another name, passing it to a function or putting it in a list gives a copy that never changes when the original does. To build a list, add to it: `set names += ["Ada"]`.

**Changing part of a variable** (draft 0.14). `set` also takes a *place*: a `var` followed by fields and indexes.

```tessera
var player = Player { name: "marine", health: 100, position: Point { x: 0, y: 0 } }
set player.health -= 10
set player.position.x = 64

var pixels = repeat([0], 320 * 200)
set pixels[y * 320 + x] = color

var world = load_world()
set world.things[3].angle = 90
```

- The root must be a `var` of the same function (E0205 otherwise, so parameters, `let` bindings and loop variables can never be changed, also not in part). Setting a place counts as changing the `var` for E0206.
- `.field` steps into a record (E0209 for an unknown field, E0512 for an enum or a type without fields); `[index]` steps into a `List[T]` or `Bytes` with an `Int` index (E0507 for anything else, including `Map`). The new value must have the type of the place (E0501); in `Bytes` it is an `Int`, and R018 when outside 0 to 255.
- `set place += e` and `set place -= e` are short for `set place = place + e` and `set place = place - e`, so the place's index expressions are evaluated twice, as in the long form.
- Order: the index expressions are evaluated from left to right, then the new value; then the indexes are checked from left to right (R001 when out of bounds) and the place is written.
- Only the named `var` changes. A copy made earlier keeps its value:

  ```tessera
  var a = [1, 2, 3]
  let b = a
  set a[0] = 9      // a is [9, 2, 3], b is still [1, 2, 3]
  ```

**Performance guarantee.** `set x = f(x, ...)` where `x` is `Text`, a list, map, `Bytes`, record or enum (records and enums since draft 0.18), and `x` appears exactly once on the right side, updates `x` in place. Building a list or text with `append`, `concat`, `replace_at` or `+` in a loop is therefore linear, not quadratic. Setting a place (`set xs[i] = v`, `set r.field = v`) takes constant time, plus one copy of the list or record on the first change after a copy of it was made elsewhere (copy on write). Since draft 0.20 the last use of a variable is moved in three more places: a variable used once in a `return` value, or in the expression a function ends with (`return draw(c, x)`, `fn f(c: Canvas) -> Canvas = draw(c, 1)`); and a variable used once in each branch of an `if` or `match` counts as used once, so `set c = if a { f(c) } else { g(c) }` updates in place like `set c = f(c)`. A function that changes a list it receives, `fn plot(cells: List[Int], i: Int) -> List[Int] { var out = cells ... out }`, works in place too when `var out = cells` is a statement of the function body itself (not inside a loop or branch) and the parameter is used nowhere else; called as `set grid = plot(grid, i)`, no copy is made. These are implementation strategies that do not change the meaning of the program: no other code can observe the old value.

### 6.3 No shadowing

A name cannot be redeclared while it is in scope, including in nested blocks, loop variables and `match` bindings (E0204):

```tessera
let count = 1
if ready {
let count = 2    // ERROR E0204: `count` is already defined on line 1
}
```

Shadowing is a common cause of review mistakes where a reader assumes the outer value. Two sibling blocks may each use the same name.

## 7. Expressions

Tessera is expression-oriented: blocks, `if` and `match` produce values.

### 7.1 Operators

From highest to lowest precedence:

| Precedence | Operators | Operand types | Result |
|---|---|---|---|
| 1 | `f(x)`, `xs[i]`, `record.field` | call, index, field | |
| 2 | `-x`, `not x`, `try x` | `Int`, `Decimal`, `Float`, `Bool`, `Result`/`Option` (section 5.6) | same, or the unwrapped value |
| 3 | `*`, `/`, `%` | `Int`; `*` and `/` also on `Decimal` and `Float` | same |
| 4 | `+`, `-` | `Int`, `Decimal`, `Float`; `+` also on `Text` and on two lists of the same type | same |
| 5 | `==`, `!=`, `<`, `<=`, `>`, `>=` | see below | `Bool` |
| 6 | `and` | `Bool` | `Bool` |
| 7 | `or` | `Bool` | `Bool` |

- Binary operators are left-associative, except comparisons, which **do not chain**: `a < b < c` is error E0104.
- `and` and `or` short-circuit. Mixing them without parentheses is error E0105: write `(a and b) or c`.
- `not a == b` is error E0105: write `not (a == b)` or `(not a) == b`.
- `==` and `!=` work on any two values of the same type, comparing by value (lists element by element, records field by field).
- `<`, `<=`, `>`, `>=` work on `Int`, `Decimal` and `Float` (E0514 otherwise).
- Integer `/` truncates toward zero; `%` has the sign of the left operand. Division or remainder by zero is runtime error R003.
- Overflow of `+`, `-`, `*`, `/`, `%` and unary `-` is runtime error R002.
- `+` on lists gives a new list with the items of both: `[1, 2] + [3]` is `[1, 2, 3]`. The element types must match (E0503).

There are no method calls: `length(xs)`, not `xs.length()`.

### 7.2 Blocks

A block `{ ... }` is a sequence of statements. Its value is the value of the last line if that line is an expression; otherwise `()`.

```tessera
let area = {
let width = 4
let height = 5
width * height
}
```

An expression on any other line must produce `Unit` (or never return). A value that is silently dropped is error E0508; use `let _ = ...` to discard it on purpose.

### 7.3 `if`

```tessera
let kind = if count == 0 { "empty" } else { "has items" }

if verbose {
print("done")
}
```

- The condition must be `Bool`. No parentheses around it; braces are always required.
- An `if` without `else` has type `Unit`, and its block must not produce a value (E0506).
- With `else`, both branches must have the same type (E0506). Where no type is expected, the `else` branch gets the type of the `then` branch, so `.None`, `.Ok(x)` or `[]` there need no annotation. When the `then` branch itself ends in such a value, it gets the type of the `else` branch instead: `if bad { .None } else { parse_int(text) }` is an `Option[Int]`.
- `else if` chains are allowed: `if a { ... } else if b { ... } else { ... }`.

### 7.4 Evaluation order

Evaluation is strictly left to right: function arguments, operands, list elements and record fields are evaluated in written order. This is part of the language, not a backend detail.

### 7.5 `match`

```tessera
fn evaluate(expr: Expr) -> Int {
match expr {
Expr.Number(value) => value
Expr.Add(left, right) => evaluate(left) + evaluate(right)
Expr.Missing => fail("missing expression")
}
}

fn describe(count: Int) -> Text {
match count {
0 => "none"
1 => "one"
_ => "many"
}
}
```

- `match` works on enums, `Option`, `Result`, `Int`, `Text` and `Bool`. Arms are tried top to bottom; one arm per line.
- An arm body is an expression, or one `set`, `let`, `for` or `while` statement, which makes the arm a `Unit` block: `Op.Push(value) => set stack = append(stack, value)`. Several statements need a block `{ ... }`.
- Patterns: `_` (anything), a literal (`42`, `-1`, `"text"`, `true`), or a variant `Type.Variant(binding, _, ...)` (or `.Variant(...)`, the type being that of the matched value). Each binding names one variant field by position; `_` ignores a field. Patterns do not nest.
- **Exhaustive.** A `match` on an enum must cover every variant or end with `_`; on `Bool` it must cover both values; on `Int` and `Text` it needs a `_` arm (E0509). Listing every variant is preferred over `_`, so adding a variant later produces errors at every place that must handle it.
- An arm that can never be reached is an error (E0510).
- All arms must have the same type (E0506).
- In `if`, `while`, `for` and `match` headers, `{` starts the body. A record literal there must be wrapped in parentheses.

### 7.6 Bit operations and fixed-point arithmetic

Bit operations are built-in functions on `Int`, which is a 64-bit two's complement integer (section 10):

| Function | Result |
|---|---|
| `bit_and(a, b)`, `bit_or(a, b)`, `bit_xor(a, b)` | Bitwise on all 64 bits; negative numbers in two's complement, so `bit_and(-1, 255)` is `255`. |
| `shift_left(a, n)` | `a * 2^n`. R002 when that does not fit in `Int`, as for `*`: shifting never silently loses bits. |
| `shift_right(a, n)` | Arithmetic: `a / 2^n` rounded toward negative infinity, so `shift_right(-1, 1)` is `-1` and `shift_right(-5, 1)` is `-3`. |

The shift amount `n` must be 0 to 63; anything else, including a negative amount, is runtime error R020.

**16.16 fixed point.** A number `v` is stored as the `Int` `v * 65536`. The range of DOOM's 32-bit fixed-point type, `-2^31` to `2^31 - 1` (about ±32768.0), is safe for both operations:

```tessera
fn fixed_mul(a: Int, b: Int) -> Int = shift_right(a * b, 16)
fn fixed_div(a: Int, b: Int) -> Int = shift_left(a, 16) / b
```

- `a * b` of two such values has a magnitude of at most `2^62`, below `Int`'s limit of `2^63 - 1`, so the product never overflows. Products are exact before the shift; `shift_right` then rounds toward negative infinity, as DOOM's `FixedMul` does.
- `shift_left(a, 16)` of such a value has a magnitude of at most `2^47`, so the division never overflows either (`/` truncates toward zero; R003 when `b` is 0).
- Outside that range the operations stay safe but can stop: `a * b` is R002 once `|a * b| >= 2^63`, for example when both values are about `3.04 * 10^9` (about ±46341.0 as fixed point) or one is `2^32` and the other `2^31`.

## 8. Control flow

### 8.1 `for`

```tessera
for name in names {
print(name)
}

for index in range(0, 10) {
print(to_text(index))
}
```

- `for` iterates over a `List[T]` (E0507 otherwise). The loop variable is an immutable binding of type `T`, or `_` when it is not used: `for _ in range(0, n)`.
- The list is evaluated once before the loop starts.
- `for` is a statement; its body must not produce a value.

### 8.2 `while`

```tessera
var remaining = 10
while remaining > 0 {
set remaining = remaining - 1
}
```

### 8.3 `return`, `break`, `continue`

- `return value` leaves the function with `value`; `return` alone is allowed in functions returning `Unit`.
- `break` leaves the innermost loop; `continue` starts its next iteration. Outside a loop they are error E0210.
- All three have type `Never`, so they can appear in any branch: `let x = if ok { 1 } else { return 0 }`.

## 9. Functions and effects

### 9.1 Declaration

Two body forms, chosen by size:

```tessera
// Single expression
fn square(x: Int) -> Int = x * x

// Block
fn describe(x: Int) -> Text {
let squared = square(x)
to_text(x) + " squared is " + to_text(squared)
}
```

- The return type is always written, including `-> Unit`.
- A function's value is the value of its body.
- Functions may call themselves and each other in any order; declaration order does not matter.
- No default arguments, no overloading, no variadic parameters. One name, one signature (E0201).
- Built-in function names cannot be reused (E0211).

### 9.2 Effects with `uses`

An **effect** is a capability to act on the world outside the program:

| Effect | Allows |
|---|---|
| `io` | Writing to standard output and standard error. |
| `fs` | Reading and writing files. |
| `env` | Reading command-line arguments and environment variables, and choosing the exit code. |
| `process` | Starting other programs. |
| `ui` | Opening a window, drawing in it, and receiving mouse and keyboard events (section 9.4). |
| `audio` | Playing sounds (section 10, draft 0.19). |

A function declares the effects it needs after the return type:

```tessera
fn save_report(path: Text, report: Text) -> Unit uses fs {
write_file(path, report)
}

fn main() -> Unit uses io, fs, env { ... }
```

Rules:

1. A function may only call functions whose effects are all in its own `uses` list (E0301). Effects are checked against signatures, so they are visible and checked at every level, not inferred.
2. A function other than `main` that declares an effect it never uses is error E0302. Signatures must not overstate what code can do, or reviewers learn to ignore them.
3. Unknown effects (E0303) and duplicates (E0304) are errors.
4. Functions without `uses` are **pure**: same inputs, same output, no observable action. They may still stop with a runtime error (section 12).

Since draft 0.13 there are also `time` (`now`, `monotonic`, `sleep`), `random` (`random_int`, `random_bytes`, `password_hash`), `log` (`log`), `db` (the SQLite functions) and `net` (`net_request` and the `std/http` client). Keeping time and randomness effects keeps pure functions deterministic and testable. The stage 0 compiler knows none of these nor `ui`, and reports E0303 for them. Splitting capabilities finely is deliberate: "this function can read files" and "this function can open network connections" are very different review questions.

### 9.3 `main`

```tessera
fn main() -> Unit uses io { ... }
```

Every program has exactly one `main`, which takes no parameters and returns `Unit` (E0212), except a web application, which has `handle` instead (section 9.7). Command-line arguments come from `args()`, and a non-zero exit code is set with `exit(code)`; both need the `env` effect. When `main` returns normally, the exit code is 0.

### 9.4 Graphics with `ui`

A program gets one fixed-size window. Drawing goes to an off-screen buffer and becomes visible on `present()`, so the picture never flickers. Coordinates are pixels from the top-left corner; colors are `Int` values made with `rgb`.

```tessera
fn main() -> Unit uses ui {
open_window("Hello", 320, 200)
var running = true
while running {
fill_rectangle(0, 0, 320, 200, rgb(255, 255, 255))
draw_text(20, 20, "Click to quit", rgb(0, 0, 0))
present()
match next_event() {
WindowEvent.Click(x, y, button) => { set running = false }
WindowEvent.Key(name) => {}
WindowEvent.KeyUp(name) => {}
WindowEvent.Close => { set running = false }
}
}
}
```

`WindowEvent` is a built-in enum:

```tessera
enum WindowEvent {
Click(x: Int, y: Int, button: Int)   // button 1 left, 2 middle, 3 right, 4/5 scroll
Key(name: Text)                      // "a", "A", "7", " ", "Escape", "Enter", "Backspace", "Tab", "Left", "Up", "Right", "Down", "Shift", "Control", "Alt", "F1" to "F12", "Delete", "Home", "End", "PageUp", "PageDown"
KeyUp(name: Text)                    // a key was released (draft 0.18); same names as `Key`
Close                                // the user asked to close the window
}
```

A key held down reports one `Key` and, when let go, one `KeyUp`; the keyboard's automatic repeat adds further `Key` events but no `KeyUp` in between. `KeyUp` names a letter key by the letter without Shift (`"a"`), so a program tracking held keys should compare lowercase names.

**Games and animation** (draft 0.18). `next_event()` waits; `poll_event()` returns `.None` at once when nothing happened, so a loop can draw a new frame every few milliseconds and still react to input. `draw_pixels(x, y, width, colors, scale)` draws a whole image in one call: `colors` holds rows of `width` colors made with `rgb` (or read with `std/palette`), and each is drawn as a `scale` x `scale` square, so a 320 x 200 frame fills a 640 x 400 window with `scale` 2. It is far faster than one `fill_rectangle` per pixel.

```tessera
var frame = repeat([rgb(0, 0, 0)], 320 * 200)
while true {
// ... change frame ...
draw_pixels(0, 0, 320, frame, 2)
present()
match poll_event() {
.Some(event) => if event == .Close { return }
.None => sleep(10)
}
}
```

The window is redrawn automatically when it is uncovered. The program decides what to do on `Close`; the window disappears when the program ends. A complete example is `examples/tictactoe/tictactoe.tes`.

The runtime talks to the window system of the machine it was built for, without libraries:

- **unix:** a local X11 display, over its socket, in safe Rust (`runtime/ui.rs`). `DISPLAY` must be set.
- **Windows:** a window of its own, drawn with GDI (`runtime/win32.rs`). This is the one place in a `ui` program where `unsafe` is allowed; see section 16.

Both draw into an off-screen buffer that `present()` shows, both report the same events with the same key names, and a program cannot tell which one it got. macOS has no backend: a program that uses `ui` does not build there.

### 9.5 Modules

Each file other than the entry file is a **module**. Its name is its path relative to the directory of the entry file, without `.tes`: the file `geometry/shapes.tes` is the module `geometry/shapes`.

```tessera
// geometry/shapes.tes
pub type Point {
x: Int
y: Int
}

pub enum Shape {
Circle(radius: Int)
Square(side: Int)
}

fn square(value: Int) -> Int = value * value

pub fn area(shape: Shape) -> Int {
match shape {
Shape.Circle(radius) => 3 * square(radius)
Shape.Square(side) => square(side)
}
}
```

```tessera
// main.tes (the entry file)
import geometry/shapes uses nothing

fn main() -> Unit uses io {
let shape = shapes.Shape.Circle(2)
print(to_text(shapes.area(shape)))
let label = match shape {
shapes.Shape.Circle(_) => "circle"
shapes.Shape.Square(_) => "square"
}
print(label)
}
```

Rules:

1. **Private by default.** Only declarations marked `pub` can be used by other modules (E0607). A public record exposes all its fields, a public enum all its variants.
2. **Explicit, qualified use.** `import geometry/shapes` makes the module available under its last path segment, `shapes`. Every use from another module is qualified: `shapes.area(...)`, `shapes.Shape`, `shapes.Shape.Circle(2)`, `shapes.Point { x: 1, y: 2 }`, and the same in types and patterns. A reader can always see where a name comes from. There are no wildcard imports and no renaming. Inside its own module a declaration is used unqualified.
3. **Imports come first** in a file, before all declarations (E0605). A module is imported at most once per file, and two imports cannot share a last segment (E0604).
4. **Module paths** are `snake_case` segments separated by `/` (E0602). They can only name files inside the project: `..` and absolute paths cannot be written. A missing file is E0601.
5. **No cycles.** Modules must form layers (E0603). The entry file cannot be imported.
6. **Every import is used** (E0610), and a local name cannot be the same as an import's name (E0204).
7. **Only the entry file defines `main`** (E0611).
8. **Public means complete.** A public function or type cannot mention a private type of its module in its signature or fields (E0609), because other modules could not name that type.
9. **Effect limits on imports.** `import geometry/shapes uses nothing` promises that no public function of that module uses any effect; `import storage uses fs` allows only `fs`. The compiler checks the promise against the module's signatures (E0608). Because every function declares its effects, this limits everything the module can do, including through its own imports. Imports without `uses` have no limit.

There is no package manager: imports resolve only to files in the project. Fetching code from elsewhere is a separate supply-chain question (section 18).

In generated code, names from a module get its path as a prefix (`f_geometry__shapes__area`, `T_geometry__shapes__Shape`); names in the entry file keep their plain form.

### 9.6 Functions as values

```tessera
fn price_of(item: Item) -> Int = item.price

fn report(items: List[Item]) -> Text = join(map(sort_by(items, price_of), name_of), ", ")

fn apply_twice(f: fn(Int) -> Int, n: Int) -> Int = f(f(n))
```

- A function name without a call is a value of type `fn(parameter types) -> result uses effects`. Built-ins with one signature are values too, and `to_text` where the parameter type is known: `map(numbers, to_text)`.
- Only named functions, no closures: a function value captures nothing, so it is as easy to review as a call.
- Calling a function value uses its effects, which the caller must declare (E0301). A function with effects fits a parameter type only if that type lists those effects.
- Records containing function values compare with `==` by identity of the function.

### 9.7 Web applications

```tessera
import std/http

fn handle(request: http.Request) -> http.Response uses db, log {
let parameters = http.match_path("/items/:id", request.path) else return http.not_found()
http.text(200, "item \(value_or(get(parameters, "id"), ""))")
}
```

- An entry file with `handle` and no `main` is a web application. `handle` must have exactly this signature (E0213) and may not use `io` or `ui` (E0213): standard output belongs to the server; diagnostics go through `log`.
- `tessera1 build app.tes -o app` gives an executable that is a CGI program (when the server sets `GATEWAY_INTERFACE`), an HTTP/1.1 server (`app serve --port N`, also started without arguments when IIS HttpPlatformHandler sets `HTTP_PLATFORM_PORT`), a FastCGI server (`app fastcgi --port N`) or a test host (`app request GET /path`).
- `app serve --tls-cert cert.pem --tls-key key.pem` serves HTTPS (TLS 1.3) instead of HTTP (draft 0.22). The certificate file holds the server's certificate followed by any intermediate certificates, in PEM; the key file the private key, in PEM (PKCS #8, SEC 1 or PKCS #1; ECDSA P-256 or P-384, or RSA of 2048 bits or more), without a passphrase. A key that does not belong to the certificate stops the server at start-up. Behind nginx, Apache or IIS, those servers handle TLS instead.
- `tessera1 build app.tes --library -o libapp.so` gives a shared library with a small C interface, loaded by the modules for nginx, Apache and IIS in `hosts/`. The generated code stays free of `unsafe`; the C interface lives in one hand-written adapter, `runtime/adapter.rs`.
- Every request runs on its own thread with its own stack, a time limit and a memory limit, and shares nothing with other requests: Tessera has no global variables. A runtime error in one request gives that request a 500 response and a log entry.

### 9.8 Tests

```tessera
test "prices are summed" {
assert(total([1.5, 2.25]) == 3.75)
}
```

`test "name" { ... }` declarations sit at the top level of a file. `tessera1 test file.tes` runs those of the entry module, each on its own thread, reports `ok` or `FAILED` with the runtime error (an `assert` that fails is R019), and exits with 1 if any failed. A test may use every effect except `ui`. `build` ignores tests.

## 10. Standard library

The standard library is a fixed set of built-in functions. `T` stands for any element type.

| Function | Signature | Notes |
|---|---|---|
| `print` | `(value: Text) -> Unit uses io` | Writes `value` and a newline to standard output. |
| `print_error` | `(value: Text) -> Unit uses io` | Same, to standard error. |
| `to_text` | `(value: Int or Bool) -> Text` | Decimal number, or `"true"` / `"false"`. |
| `length` | `(value: List[T] or Text) -> Int` | Number of elements or of Unicode scalar values. |
| `append` | `(list: List[T], item: T) -> List[T]` | New list with `item` at the end. |
| `concat` | `(first: List[T], second: List[T]) -> List[T]` | New list with all items of both. |
| `slice` | `(list: List[T], start: Int, end: Int) -> List[T]` | Items `start` up to, not including, `end`. R004 if out of range. |
| `replace_at` | `(list: List[T], index: Int, item: T) -> List[T]` | New list with one item replaced. R001 if out of bounds. |
| `range` | `(start: Int, end: Int) -> List[Int]` | `start` up to, not including, `end`; empty when `start >= end` (draft 0.20; before, `start > end` was R004). |
| `code_points` | `(text: Text) -> List[Int]` | The Unicode scalar values of the text. |
| `from_code_point` | `(code: Int) -> Text` | One-character text. R005 if not a valid scalar value. |
| `from_code_points` | `(codes: List[Int]) -> Text` | Text from scalar values. R005 if any is invalid. |
| `join` | `(parts: List[Text], separator: Text) -> Text` | Concatenates with a separator between parts. |
| `split` | `(text: Text, separator: Text) -> List[Text]` | The parts between separators, including empty ones: `split("a,,b", ",")` is `["a", "", "b"]`, `split("", ",")` is `[""]`. An empty separator gives the characters: `split("abc", "")` is `["a", "b", "c"]`. |
| `parse_int` | `(text: Text) -> Option[Int]` | An optional `-` followed by one or more ASCII digits. Anything else (a leading `+`, empty text, spaces, other characters, a value that does not fit in `Int`) gives `.None`. |
| `sort` | `(list: List[Int] or List[Text]) -> List[Int] or List[Text]` | Ascending. Text is ordered by code points, so `"Z"` comes before `"a"`. Other element types are E0501. |
| `index_of` | `(list: List[T], item: T) -> Option[Int]` or `(text: Text, part: Text) -> Option[Int]` | Position of the first equal item, or of the first occurrence of `part`, counted in characters. `index_of(text, "")` is `.Some(0)`. |
| `contains` | `(list: List[T], item: T) -> Bool` or `(text: Text, part: Text) -> Bool` | Whether `index_of` would find it. |
| `to_lower`, `to_upper` | `(text: Text) -> Text` | Unicode case mapping; the length can change (`to_upper("ß")` is `"SS"`). |
| `read_file` | `(path: Text) -> Text uses fs` | Whole file as UTF-8 text. R006 on failure. |
| `write_file` | `(path: Text, content: Text) -> Unit uses fs` | Creates or replaces the file. R007 on failure. |
| `args` | `() -> List[Text] uses env` | Command-line arguments, without the program name. |
| `exit` | `(code: Int) -> Never uses env` | Stops the program with exit code 0 to 255. |
| `exit_with` | `(code: Int, message: Text) -> Never uses io, env` | Writes `message` and a newline to standard error, then stops with exit code 0 to 255 (R009 otherwise). The only built-in with two effects. |
| `fail` | `(message: Text) -> Never` | Stops the program with runtime error R010. |
| `value_or` | `(option: Option[T], default: T) -> T` | The value inside `option`, or `default`. |
| `file_exists` | `(path: Text) -> Bool uses fs` | True if `path` is an existing regular file. |
| `environment_variable` | `(name: Text) -> Text uses env` | The value, or `""` when it is not set. |
| `run_process` | `(program: Text, arguments: List[Text]) -> Int uses process` | Runs a program (found via `PATH`) with inherited input and output, waits, and returns its exit code; `-1` if it could not be started or was killed. |
| `rgb` | `(red: Int, green: Int, blue: Int) -> Int` | A color. Each component 0 to 255, else R011. |

Added in draft 0.13:

| Function | Signature | Notes |
|---|---|---|
| `trim`, `starts_with`, `ends_with`, `replace` | `(Text) -> Text`; `(Text, Text) -> Bool`; `(text, old, new) -> Text` | `trim` removes outer whitespace; `replace` every occurrence (an empty `old` changes nothing). |
| `slice_text`, `repeat`, `pad_left`, `pad_right`, `lines` | `(Text, Int, Int) -> Text`; `(Text, Int) -> Text`; `(Text, width: Int, fill: Text) -> Text`; `(Text) -> List[Text]` | Positions in characters (R004 when out of range); `fill` is one character (R004); `lines` splits at `\n` and drops a final `\r` and a final empty line. |
| `get`, `put`, `remove`, `keys`, `values` | maps | `get(map, key) -> Option[V]`; `put`/`remove` give a changed map; `keys`/`values` in key order. `length` and `contains` also take a map. |
| `map`, `filter`, `sort_by` | `(List[T], fn(T) -> U) -> List[U]`, `(List[T], fn(T) -> Bool) -> List[T]`, `(List[T], fn(T) -> K) -> List[T]` | `sort_by` is stable; `K` is `Int`, `Text` or `Decimal`. |
| `read_line`, `read_input` | `() -> Option[Text] uses io`, `() -> Text uses io` | One line without its line break (`.None` at the end of input), or all of standard input. R008 if not UTF-8. |
| `read_text`, `write_text`, `list_directory`, `create_directory`, `remove_file` | `uses fs`, each `-> Result[..., FileError]` | Failures as values, unlike `read_file`/`write_file`. `list_directory` is sorted. |
| `to_bytes`, `from_bytes`, `bytes`, `byte_values` | `Text -> Bytes`, `Bytes -> Option[Text]`, `List[Int] -> Bytes` (R018 outside 0..255), `Bytes -> List[Int]` | |
| `decimal`, `to_int`, `round`, `parse_decimal` | `Int -> Decimal`, `Decimal -> Int` (toward zero), `(Decimal, places: Int) -> Decimal` (half away from zero, places 0..9), `Text -> Option[Decimal]` | `to_text` also takes a `Decimal` (shortest exact form: `"1.5"`). |
| `now`, `monotonic`, `sleep` | `() -> Int uses time`, `() -> Int uses time`, `(ms: Int) -> Unit uses time` | Milliseconds: since 1970 (UTC), and since an arbitrary point. |
| `format_time`, `parse_time`, `http_date` | `Int -> Text` (`2026-11-08T14:52:38.123Z`), `Text -> Option[Int]` (RFC 3339), `Int -> Text` (`Sun, 08 Nov 2026 14:52:38 GMT`) | Pure. |
| `random_int`, `random_bytes` | `(low: Int, high: Int) -> Int uses random` (high exclusive), `(n: Int) -> Bytes uses random` | Cryptographically secure source. |
| `sha256`, `hmac_sha256`, `equal_constant_time` | `Bytes -> Bytes`, `(key, message) -> Bytes`, `(Bytes, Bytes) -> Bool` | |
| `base64_encode`, `base64_decode`, `hex_encode`, `hex_decode` | `Bytes -> Text`, `Text -> Option[Bytes]` | |
| `password_hash`, `password_verify` | `Text -> Text uses random`, `(password, hash) -> Bool` | PBKDF2-HMAC-SHA-256 with 600,000 iterations and a random salt. |
| `log` | `(level: Text, message: Text) -> Unit uses log` | Standard error, which servers route to their error log. |
| `db_open`, `db_query`, `db_execute` | `Text -> Result[Database, Text]`, `(Database, sql: Text, params: List[Text]) -> Result[List[Map[Text, Text]], Text]`, `... -> Result[Int, Text]` | SQLite. One statement per call; parameters `?1`, `?2`, ... are bound, never pasted into the SQL. A row maps column names to text; NULL columns are absent. `db_execute` returns the number of changed rows. |
| `net_request` | `(method, url, header_lines: List[Text], body: Bytes) -> Result[Bytes, Text] uses net` | `http://` or `https://` (since draft 0.22); the raw response, for `std/http`. |
| `assert` | `(condition: Bool) -> Unit` | R019 when false. |

Added in draft 0.14:

| Function | Signature | Notes |
|---|---|---|
| `repeat` | `(list: List[T], count: Int) -> List[T]` | The items of `list`, `count` times: `repeat([0], 3)` is `[0, 0, 0]`. R004 if `count` is negative. The `Text` form above is unchanged. |
| `bit_and`, `bit_or`, `bit_xor` | `(a: Int, b: Int) -> Int` | Section 7.6. |
| `shift_left`, `shift_right` | `(value: Int, amount: Int) -> Int` | Section 7.6. `amount` 0 to 63, else R020; `shift_left` R002 on overflow. |

Added in draft 0.15:

| Function | Signature | Notes |
|---|---|---|
| `read_bytes` | `(path: Text) -> Result[Bytes, FileError] uses fs` | The whole file. A missing file is `.Error(.NotFound)`, not a runtime error. |
| `write_bytes` | `(path: Text, data: Bytes) -> Result[Unit, FileError] uses fs` | Creates or replaces the file. |

Added in draft 0.17 (`Float`, section 5.1):

| Function | Signature | Notes |
|---|---|---|
| `float` | `(value: Int) -> Float` | Exact up to ±2^53, otherwise the nearest `Float`. |
| `to_int` | `(value: Float) -> Int` | Toward zero. R002 outside the `Int` range. (The `Decimal` form is unchanged.) |
| `floor`, `ceil` | `(value: Float) -> Float` | Round down or up to a whole number; combine with `to_int` for an `Int`. |
| `round` | `(value: Float, places: Int) -> Float` | To `places` decimals (0 to 15, else R004), half away from zero, as for `Decimal`; the result is the nearest `Float` to that decimal. |
| `sqrt`, `exp`, `ln` | `(value: Float) -> Float` | R021 for a negative `sqrt` or a non-positive `ln`; R002 when `exp` overflows. |
| `sin`, `cos`, `tan` | `(radians: Float) -> Float` | |
| `atan2` | `(y: Float, x: Float) -> Float` | The angle of the point `(x, y)`, from -π to π. |
| `pow` | `(base: Float, exponent: Float) -> Float` | R021 when there is no real result (`pow(-8.0, 0.5)`), R002 on overflow. |
| `pi` | `() -> Float` | π. |
| `parse_float` | `(text: Text) -> Option[Float]` | An optional `-`, digits, optionally `.` and digits, optionally `e` or `E`, a sign and digits: what `to_text` produces and what literals look like. `"inf"`, `"nan"`, `"+1"`, `".5"` and out-of-range values give `.None`. |

`to_text`, interpolation, `sort` and `sort_by` keys also accept `Float`. `to_text` gives the shortest text that reads back as the same number, always with a `.` or an exponent: `1.0`, `0.1`, `-2.5`, `1e21`, `1e-7`.

Added in draft 0.22:

| Function | Signature | Notes |
|---|---|---|
| `sha512`, `hmac_sha512` | `Bytes -> Bytes`, `(key, message) -> Bytes` | 64-byte results. |
| `hkdf_sha256` | `(secret: Bytes, salt: Bytes, info: Bytes, length: Int) -> Bytes` | HKDF (RFC 5869) with SHA-256: `length` bytes (1 to 8,160, else R004) of key material for the purpose named in `info`. |
| `aead_seal` | `(key: Bytes, nonce: Bytes, plaintext: Bytes, associated: Bytes) -> Bytes` | ChaCha20-Poly1305 (RFC 8439): the ciphertext followed by a 16-byte tag. `key` is 32 bytes and `nonce` 12 (R004 otherwise). A nonce may never be used twice with the same key; `std/crypto.encrypt` takes care of that. |
| `aead_open` | `(key: Bytes, nonce: Bytes, sealed: Bytes, associated: Bytes) -> Option[Bytes]` | The plaintext, or `.None` when `sealed` or `associated` was changed or the key or nonce differ. |

**HTTPS.** `net_request` speaks TLS 1.3 to `https://` URLs (default port 443) with the cipher suites TLS_AES_128_GCM_SHA256, TLS_AES_256_GCM_SHA384 and TLS_CHACHA20_POLY1305_SHA256 and the key exchange X25519 or secp256r1. The server's certificate chain must lead to a trusted certificate: those of the system (on unix the CA bundle in `/etc/ssl` or `/etc/pki`, or `SSL_CERT_FILE`; on Windows the certificate store, read through PowerShell), or, when `TESSERA_CA_FILE` names a PEM file, only the certificates in that file. Certificates are checked for their dates, their key usage, name constraints, RSA keys of at least 2048 bits, and the host name against the subject alternative names (the common name is not used). A server without TLS 1.3, a certificate that fails a check, or any protocol error gives `.Error` with the reason; nothing can switch the checks off. `TESSERA_NET_ALLOW` applies as for `http://`.

The runtime has no external dependencies, so SQLite is reached through the `sqlite3` program (a persistent child process per database), an interim choice of draft 0.13; see the roadmap. TLS and the cryptography under it are part of the runtime itself (draft 0.22).

### 10.1 Standard modules

`std/...` modules are part of the compiler and written in Tessera, so they are checked like any other module and can be reviewed in `std/`:

| Module | Contents |
|---|---|
| `std/http` | `Request`, `Response`, `Header`; `text`, `html` (with a Content-Security-Policy), `json`, `redirect`, `not_found`, `with_header`, `with_cookie` (HttpOnly, Secure, SameSite=Lax); `header`, `text_body`, `form`, `cookies`, `cookie`, `match_path`; `sign`/`verify` (HMAC-signed values), `csrf_token`/`check_csrf`; the client `fetch` and `get_text` (effect `net`; `http://` and `https://`); `request` and `response_text` for tests. |
| `std/crypto` | `new_key() -> Bytes uses random` (32 random bytes); `derive_key(secret: Bytes, purpose: Text) -> Bytes` (HKDF-SHA-256, a 32-byte key per purpose); `encrypt(key, plaintext: Bytes, context: Text) -> Bytes uses random` (ChaCha20-Poly1305 under a random nonce: nonce, ciphertext and tag); `decrypt(key, sealed, context) -> Option[Bytes]`; `encrypt_text(key, text, context) -> Text uses random` and `decrypt_text(key, token, context) -> Option[Text]` (URL-safe base64 without padding, for cookies and URLs). `context` is authenticated but not stored, so a value encrypted for one use is refused in another. Draft 0.22. |
| `std/logging` | `enum Level { Debug, Info, Warning, Error }`, `enum Format { Text, Json }`, `type Logger { level, format, fields: List[Field] }`, `type Field { key, value }`; `new(level)` (text format, no fields), `with_field(logger, key, value)` (context added to every line, in order), `with_level`, `parse_level(text) -> Option[Level]` (`debug`, `info`, `warning` or `warn`, `error`, any case), `level_name`, `enabled(logger, level)`; `render(logger, level, message, fields: Map[Text, Text]) -> Text` (the line: the message, then the context fields, then `fields` in key order, as `key=value`, a value quoted as a JSON string when it is empty or holds a space, `=`, `"` or a control character, and control characters in the message escaped; or with `.Json` one object with `level`, `message` and the fields); `write(logger, level, message, fields)` and `debug`, `info`, `warning`, `error(logger, message, fields)` (effect `log`; nothing is written below the logger's level). Draft 0.23. |
| `std/config` | `type Config { values: Map[Text, Text], origins: Map[Text, Text] }`; `empty`, `from_map(values, origin)`, `parse(text, name) -> Result[Config, Text]`, `load(path) -> Result[Config, Text] uses fs`, `merge(base, top)` (`top` wins), `with_value(config, key, value, origin)`, `environment_name(prefix, key)` (`SHOP` and `server.max-size` give `SHOP_SERVER_MAX_SIZE`), `from_environment(config, prefix) uses env` (replaces each key the configuration already has by its environment variable when that is set and not empty), `has`, `find -> Option[Text]`, `names`, `section(config, prefix)`, `origin(config, key)`; `get_text`, `get_int`, `get_decimal`, `get_float`, `get_bool` (`true`/`yes`/`on`/`1`, `false`/`no`/`off`/`0`, any case), `get_list` (split at commas, trimmed), each `-> Result[..., Text]` with an error that names the key and its origin; `encode(config) -> Text`, which `parse` reads back to the same values. The file format: one `key = value` per line, `[section]` headers that prefix the keys (`section.key`), comment lines starting with `#` or `;`, keys of letters, digits, `_`, `-` and `.`; a value is the rest of the line without outer whitespace, or in double quotes with the escapes `\"`, `\\`, `\n`, `\r`, `\t`. A malformed line, a key set twice or an unclosed quote is an error with the file name and line. Draft 0.23. |
| `std/json` | `enum Json`, `parse`, `encode` (keys sorted), `quote`, `field`, `as_text`, `as_int`, `as_decimal`, `as_bool`, `as_list`, `text_field`, `int_field`, `from_int`. |
| `std/url` | `percent_encode`, `percent_decode`, `parse_query`, `parse_query_all`, `encode_query`, `path_segments`. |
| `std/html` | `escape`. |
| `std/bytes` | `read_u16_le`, `read_i16_le`, `read_u32_le`, `read_i32_le` (`(data: Bytes, at: Int) -> Int`), `read_ascii(data, at, count) -> Text` (stops at the first zero byte; bytes above 127 become `?`). Reading past the end is R001. |
| `std/math` | `min`, `max`, `abs` (R002 for the smallest `Int`), `clamp(value, low, high)` (R010 if `low > high`), `isqrt(value)` (largest `r` with `r * r <= value`; R010 for a negative value). |
| `std/fixed` | 16.16 fixed point (section 7.6): `one`, `from_int`, `floor_int` (the integer part, rounded toward negative infinity; `to_int` is the built-in for `Decimal`), `mul`, `div` (rounds toward zero; R003 for 0), `abs`; `sine(index, count)` and `cosine(index, count)`, the value at angle `2π · index / count` rounded to the nearest unit (computed with 28 fraction bits, so within 0.0003 of a unit from exact; only values that are almost exactly halfway can round the other way), and `sine_table(count)`, `cosine_table(count)` for indexes `0` to `count - 1`. Integer arithmetic only, so every platform computes the same tables. |
| `std/palette` | `read(data: Bytes, at: Int) -> List[Int]`: the 256 colors of a 768-byte RGB palette, packed like `rgb(red, green, blue)`; `red`, `green`, `blue` take them apart. |
| `std/raster` | `type Image { width, height, pixels }` (row by row; colors or palette indexes), `blank`, `from_pixels`, `pixel`, `fill`, `line` (Bresenham), `blit(image, source, x, y, transparent)`, `stretch(image, source, x, y, width, height, transparent)` (nearest pixel), `remap(image, table)` (palette indexes to colors, or a light table); `type Font { glyphs: Map[Int, Image], space, transparent }`, `measure`, `text`. Each function returns the changed image and works in place as `set image = raster.f(image, ...)`. |
| `std/sound` | `type Sound { samples: Bytes, rate }`, `from_wav(Bytes) -> Result[Sound, Text]` (PCM, 8 or 16 bits, mono or stereo), `tone(frequency, milliseconds, loudness)`, `position(dx, dy, right_x, right_y, range) -> [volume, pan]`, `play(sound, volume, pan)` (effect `audio`). |
| `std/keys` | `type Keys { held, typed }`, `none`, `update(keys, WindowEvent)`, `held`, `any_held`, `typed(keys, word)`, `forget_typed`, `release_all`. Names compare in lower case. |
| `std/plane` | 16.16 fixed point: `type Hit { hit, distance, along }`, `intersect` (ray against a segment in whole units), `ratio`, `orient`, `crosses`, `circle_touches`, `direction(dx, dy, cosines, sines, count)`. |
| `std/rng` | `next(seed)`, `byte(seed) -> [value, seed]`, `roll(seed, low, high) -> [value, seed]`: a repeatable linear congruential generator (not for secrets). |
| `std/ppm` | `encode(width, height, pixels: List[Int], palette: List[Int]) -> Bytes`: a binary PPM (P6) image of palette indexes, row by row from the top left. R010 if the number of pixels is not `width * height` or a side is not positive; R001 for an index outside the palette. Write it with `write_bytes`. |

The modules added in drafts 0.16 and 0.20 use no effects except `std/sound.play` (`audio`); their tests are in `tests/std/`.

Graphics (effect `ui`, section 9.4) and sound (effect `audio`):

| Function | Signature | Notes |
|---|---|---|
| `open_window` | `(title: Text, width: Int, height: Int) -> Unit` | Opens the program's only window, 1 to 4096 pixels per side. |
| `fill_rectangle` | `(x: Int, y: Int, width: Int, height: Int, color: Int) -> Unit` | |
| `draw_line` | `(x1: Int, y1: Int, x2: Int, y2: Int, thickness: Int, color: Int) -> Unit` | Rounded ends. |
| `draw_circle` | `(center_x: Int, center_y: Int, radius: Int, thickness: Int, color: Int) -> Unit` | Outline. |
| `fill_circle` | `(center_x: Int, center_y: Int, radius: Int, color: Int) -> Unit` | |
| `draw_text` | `(x: Int, y: Int, text: Text, color: Int) -> Unit` | `(x, y)` is the top-left corner. One built-in monospaced font; characters outside Latin-1 show as `?`. |
| `text_width` | `(text: Text) -> Int` | Width in pixels of `text` in that font. |
| `present` | `() -> Unit` | Shows everything drawn since the last `present`. |
| `next_event` | `() -> WindowEvent` | Waits for the next click, key press, key release or close request. |
| `poll_event` | `() -> Option[WindowEvent]` | The next event if one is waiting, otherwise `.None` at once (draft 0.18). |
| `play_sound` | `(samples: Bytes, rate: Int, volume: Int, pan: Int) -> Unit uses audio` | Starts playing a sound and returns at once (draft 0.19): 8-bit unsigned mono samples (128 is silence) at `rate` samples per second (1,000 to 96,000), `volume` 0 to 100, `pan` from -100 (left) to 100 (right). Up to 16 sounds play at the same time; a 17th replaces the oldest. R004 for arguments out of range. The runtime sends the mixed sound to the sound card itself on Windows, and on unix to an audio player that reads 16-bit stereo samples at 22,050 per second from standard input: `aplay` or `pw-cat` on `PATH`, or the program named in `TESSERA_AUDIO_PLAYER`, which is tried first on every system. Without any of them nothing is heard and the program continues. A sound starts within about 60 ms plus the audio system's own delay. |
| `draw_pixels` | `(x: Int, y: Int, width: Int, colors: List[Int], scale: Int) -> Unit` | Draws rows of `width` colors with their top-left corner at `(x, y)`, each pixel as a `scale` x `scale` square (draft 0.18). R004 if `width` or `scale` (1 to 16) is out of range or `length(colors)` is not a multiple of `width`; R011 for a value that is not a color (0 to 16777215). |

Drawing calls without an open window, and losing the connection to the display, are runtime error R012.

`length`, `to_text`, `sort`, `index_of`, `contains`, `slice`, `repeat`, the list functions and the map functions are the only overloaded or generic functions, and only because user-defined generics are deferred.

## 11. Notes

A **note** records design intent: why something exists, what constraint it encodes, what must not change without review.

```tessera
note "Age limit is set by legal requirement NL-2024-17; do not change without legal review."
note "Kept pure so it can be unit tested without stubs."
fn is_adult(age: Int) -> Bool = age >= 18
```

Rules:

- A note attaches to the **next declaration** (`fn`, `type` or `enum`). Several notes may precede one declaration; they keep their order.
- A note must sit directly above its declaration. A note followed by a blank line or by nothing is error E0401, so a note never silently drifts away from what it describes.
- Notes have no effect on type checking, effects, or runtime behavior.
- The parser and code generator preserve notes exactly. Generated Rust carries them as `// note:` comments.
- Notes on statements are expected later.

### 11.1 Metadata export

`tessera notes program.tes` writes the notes as JSON:

```json
{
  "notes": [
    {"file": "program.tes", "item": "is_adult", "kind": "function", "line": 3, "text": [
      "Age limit is set by legal requirement NL-2024-17; do not change without legal review.",
      "Kept pure so it can be unit tested without stubs."
    ]}
  ]
}
```

`kind` is `function`, `type` or `enum`. Embedding notes in a typed intermediate representation export is *not implemented* yet.

## 12. Runtime errors

Some errors can only be detected while running. A program cannot catch them; they stop the program, or in a web application the request, in a defined way.

| Code | Situation |
|---|---|
| R001 | List or `Bytes` index out of bounds (indexing, `set xs[i] = v`, `replace_at`) |
| R002 | Overflow: `Int` out of range, or a `Float` result too large to represent |
| R003 | Division or remainder by zero |
| R004 | Invalid `slice` bounds, or another argument outside its allowed range |
| R005 | Invalid Unicode code point |
| R006 | File cannot be read |
| R007 | File cannot be written |
| R008 | Command-line argument is not valid UTF-8 |
| R009 | `exit` code outside 0 to 255 |
| R010 | `fail(message)` was called |
| R011 | `rgb` component outside 0 to 255 |
| R012 | Window error: no display, no open window, or the display connection failed |
| R013 | Recursion too deep: the stack is full |
| R014 | Time limit exceeded (`TESSERA_TIME_LIMIT_MS`) |
| R015 | Memory limit exceeded (`TESSERA_MEMORY_LIMIT_MB`) |
| R016 | File path outside the directory allowed by `TESSERA_FS_ROOT` |
| R017 | `exit` called while handling a web request |
| R018 | Byte value outside 0 to 255 |
| R019 | `assert` failed |
| R020 | Shift amount outside 0 to 63 (`shift_left`, `shift_right`) |
| R021 | `Float` operation without a real result (`sqrt` of a negative number, `ln` of zero, ...) |

When a runtime error occurs:

1. Output already printed is flushed; nothing more is written to standard output.
2. A message is written to standard error, pointing at the failing operation:
   ```
   runtime error R001: list index 5 is out of bounds for a list of length 3
     at program.tes:12:15 in function `label`
   ```
3. The process exits with code **70** (`EX_SOFTWARE`). In a web application only the request ends: it gets a 500 response, and the message goes to the server's error log.

Internally a runtime error unwinds (as a Rust panic with a Tessera payload) to the boundary of the program or request; `exit` unwinds the same way. Nothing in Tessera can observe or stop the unwinding.

**Limits.** Every function entry checks the stack, so deep recursion is R013 instead of a crash. Loops and function entries check the time limit. The memory limit counts the bytes that list, text and map operations allocate during a run or request. The command line uses a 512 MB stack and no time or memory limit unless the environment variables are set; a web request gets a 64 MB stack (`TESSERA_STACK_MB`), 30 seconds and 512 MB by default. `TESSERA_FS_ROOT` makes file paths relative to one directory and refuses absolute paths, `..` and symbolic links leading out; `TESSERA_NET_ALLOW` lists the hosts `net` may reach.

Exit code and message format are defined by Tessera, not inherited from the Rust backend.

**Known limitation:** running out of memory before the memory limit is reached (the limit counts only list, text and map growth) still aborts the process.

Runtime errors are for bugs and broken environments. Expected failures, such as invalid input, are values of type `Result` (section 5.6) and are handled by the program.

## 13. Compiler diagnostics

Diagnostics are part of the language design: an LLM fixing code relies on them as much as a person does.

```
error[E0301]: `greet` uses effect `io`, but `format_report` does not declare it
  --> report.tes:14:5
   |
14 |     greet(name)
   |     ^^^^^^^^^^^ this call needs `io`
   |
help: add `uses io` to the signature of `format_report`, or move the call to a function that already uses `io`
```

Rules:

- Every diagnostic has a stable code, a file, a line, a column, and a one-line message.
- Every error suggests a concrete fix where one exists. Where exactly one change is clearly right, the diagnostic also carries it as a machine-applicable fix (section 20.4).
- Error codes are never reused for a different meaning.
- `tessera check --json` emits the same diagnostics as JSON for tools and agents.
- The compiler reports all errors it can find in one pass. Type checking runs only when the whole program parsed cleanly, and a value whose type is already wrong does not cause follow-up errors.

| Code | Meaning |
|---|---|
| E0101 | Tab, carriage return or byte order mark in the file |
| E0102 | Unmatched or unclosed bracket |
| E0103 | Unexpected character, or non-ASCII letter in a name |
| E0104 | Chained comparison |
| E0105 | `and`/`or` mixed without parentheses, or `not` before a comparison |
| E0106 | Invalid or too large number |
| E0107 | Expression nested too deeply |
| E0108 | Reserved word used as a name |
| E0109 | Invalid text literal or escape |
| E0110 | Syntax error (unexpected token) |
| E0111 | `else` not on the same line as `}` |
| E0201 | Duplicate definition |
| E0202 | Unknown name |
| E0203 | Unknown function |
| E0204 | Shadowing |
| E0205 | `set` on something that is not a `var` |
| E0206 | `var` never changed |
| E0207 | Naming style violated |
| E0208 | Unknown type |
| E0209 | Unknown field or variant |
| E0210 | `break` or `continue` outside a loop |
| E0211 | Built-in function redefined |
| E0212 | Missing or invalid `main` |
| E0213 | Invalid `handle` in a web application: wrong signature, or `io`/`ui` |
| E0301 | Effect used but not declared |
| E0302 | Effect declared but not used |
| E0303 | Unknown effect |
| E0304 | Effect listed twice |
| E0401 | Note not directly above a declaration |
| E0501 | Type mismatch |
| E0502 | Wrong number of arguments |
| E0503 | Operator applied to wrong types |
| E0504 | Condition is not `Bool` |
| E0505 | Empty list without a known type |
| E0506 | Branches with different types, or a value from an `if` without `else` |
| E0507 | Indexing or looping over something that is not a list |
| E0508 | Unused value |
| E0509 | Non-exhaustive `match` |
| E0510 | Unreachable `match` arm |
| E0511 | Wrong record literal (missing or duplicate field, not a record) |
| E0512 | Field access on a value without fields |
| E0513 | Pattern does not fit the matched value |
| E0514 | Comparison of unsupported types |
| E0515 | `try` on a value that is not a `Result` or `Option` |
| E0516 | `try` in a function whose return type cannot carry the failure |
| E0517 | `try` on a `Result` whose error type does not fit the function's |
| E0518 | `.Variant` where no type is known from context |
| E0601 | Imported module not found |
| E0602 | Invalid module path |
| E0603 | Import cycle, or the entry file imported |
| E0604 | Module imported twice, or two imports with the same name |
| E0605 | Import after a declaration |
| E0606 | Unknown module name in a qualified name |
| E0607 | Private declaration used from another module |
| E0608 | Module exceeds the effects its import allows |
| E0609 | Public declaration exposes a private type |
| E0610 | Unused import |
| E0611 | `main` outside the entry file |
| E0701 | Invalid JSON syntax tree (section 20.1) |
| E0702 | Unknown edit operation, a field missing, or operations that are not valid JSON (section 20.2) |
| E0703 | An edit operation names a file that is not part of the program |
| E0704 | The target of an edit operation is not found (declaration, function, anchor or `match`) |
| E0705 | The target of an edit operation is ambiguous, or a declaration of that name already exists |
| E0706 | The source text of an edit operation does not parse as what the operation needs |
| E0707 | `interpret` cannot run a program whose `main` uses an effect other than `io`, `env`, `time`, `random` and `log` (section 21.1) |

## 14. Canonical formatting

Every program has exactly one textual form, its **canonical text**, which the compiler prints from the syntax tree (section 20.1). `tessera1 format <files...>` replaces each file by its canonical text; `--check` only reports the files that would change. Formatting never changes the program: the tree of the canonical text is the tree of the original (tested for every file in the repository), and formatting twice gives the same result as formatting once. The canonical text uses the short forms below, which the frozen stage 0 compiler does not read, so the programs shared with stage 0 (`examples/valid/` and the shared tests) are kept in its syntax.

- No indentation: every line starts in the first column. Indentation has no meaning in Tessera, which is written and read mainly by models; braces carry the structure. Leaving it out saves about 12% of the tokens of a program (section 19). Tabs are never allowed.
- One space around binary operators, after commas and colons, and inside `{ ... }` of record literals and `with`: `fn f(a: Int, b: Int) -> Int`, `Point { x: 1, y: 2 }`.
- Opening brace on the same line; `} else {` on one line.
- One statement per line, and each expression on the line of its statement: there is no line length limit. Blocks, `match` arms and declarations with fields or variants span lines; nothing else does.
- `if c { a } else { b }` stays on one line when every branch is a single value without statements, `return`, `break` or `continue`; otherwise each branch is a block over several lines.
- A `match` arm whose body is one `set`, `let`, `for` or `while` is written without braces (section 7.5).
- Parentheses only where the grammar needs them: around an operand that binds less tightly than its operator, around a comparison inside a comparison, around `and` inside `or` (and the reverse), and around a block or record literal in an `if`, `while`, `for` or `match` header.
- The short forms: `set x += e` and `set x -= e` for `set x = x + e` and `set x = x - e` (also on places), and `"a\(x)b"` for `"a" + to_text(x) + "b"`. A lone `to_text(x)` is written as the call, not as `"\(x)"`, which is the same tree.
- One blank line between declarations, none between imports and none inside bodies. Notes sit directly above their declaration. A comment group keeps its blank lines at the top level.
- Text literals use the escapes `\n`, `\t`, `\\`, `\"` and `\u{...}` for other control characters.

## 15. Grammar (EBNF)

Newline handling follows section 4.6. `NL` means a statement-ending newline. `NAME` starts with a lowercase letter, `TYPE` with an uppercase letter.

```ebnf
program        = { import NL } { declaration } ;
import         = "import" NAME { "/" NAME } [ "uses" ( "nothing" | NAME { "," NAME } ) ] ;
declaration    = { "note" TEXT NL } [ "pub" ] ( function | record | enum ) | test ;
test           = "test" TEXT block ;                 (* "test" is not a keyword *)
function       = "fn" NAME "(" [ params ] ")" "->" type [ effects ] body ;
params         = param { "," param } [ "," ] ;
param          = NAME ":" type ;
effects        = "uses" NAME { "," NAME } ;
body           = "=" expression | block ;
record         = "type" TYPE "{" { field NL } "}" ;
enum           = "enum" TYPE "{" { variant NL } "}" ;
variant        = TYPE [ "(" [ field { "," field } ] ")" ] ;
field          = NAME ":" type ;
type           = type_name [ "[" type { "," type } "]" ]   (* List[T], Map[K, V] *)
               | "fn" "(" [ type { "," type } ] ")" "->" type [ effects ] ;
type_name      = TYPE | NAME "." TYPE ;             (* qualified: module.Type *)

block          = "{" { statement NL } [ expression ] "}" ;
statement      = let_stmt | set_stmt | for_stmt | while_stmt | expression ;
let_stmt       = ( "let" | "var" ) ( NAME | "_" ) [ ":" type ] "=" expression [ "else" expression ] ;
set_stmt       = "set" place ( "=" | "+=" | "-=" ) expression ;
place          = NAME { "." NAME | "[" expression "]" } ;
for_stmt       = "for" ( NAME | "_" ) "in" header_expr block ;
while_stmt     = "while" header_expr block ;

expression     = or_expr | and_expr ;                (* never both, see 7.1 *)
or_expr        = comparison { "or" comparison } ;
and_expr       = comparison { "and" comparison } ;
comparison     = additive [ compare_op additive ] ;  (* no chaining *)
additive       = multiplicative { ( "+" | "-" ) multiplicative } ;
multiplicative = unary { ( "*" | "/" | "%" ) unary } ;
unary          = ( "-" | "not" | "try" ) unary | postfix ;
postfix        = primary { "[" expression "]" | "." NAME
                         | "with" "{" field_init { "," field_init } [ "," ] "}" } ;
primary        = INT | DECIMAL | TEXT | "true" | "false" | "(" ")" | "(" expression ")"
               | NAME | NAME "(" [ args ] ")" | NAME "." NAME "(" [ args ] ")"
               | "[" [ args ] "]" | "[" ":" "]"
               | "[" expression ":" expression { "," expression ":" expression } [ "," ] "]"
               | type_name "{" [ field_init { "," field_init } [ "," ] ] "}"
               | type_name "." TYPE [ "(" [ args ] ")" ]
               | "." TYPE [ "(" [ args ] ")" ]            (* type from context *)
               | block | if_expr | match_expr
               | "return" [ expression ] | "break" | "continue" ;
args           = expression { "," expression } [ "," ] ;
field_init     = NAME ":" expression ;
if_expr        = "if" header_expr block [ "else" ( if_expr | block ) ] ;
match_expr     = "match" header_expr "{" { pattern "=>" arm_body NL } "}" ;
arm_body       = expression | let_stmt | set_stmt | for_stmt | while_stmt ;
pattern        = "_" | [ "-" ] INT | TEXT | "true" | "false"
               | [ type_name ] "." TYPE [ "(" [ binding { "," binding } ] ")" ] ;
binding        = NAME | "_" ;
compare_op     = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
(* header_expr is an expression in which a bare block or record literal
   is not allowed, because `{` starts the body. *)
```

## 16. Compilation model and tools

```
source files (.tes)
  -> lexer / parser       -> syntax tree with notes      E01xx, E0401
  -> name and type check                                  E02xx, E05xx
  -> effect check                                         E03xx
  -> Rust code generation (safe Rust only)
  -> rustc                -> native executable
```

Guarantees the compiler keeps, each covered by a test:

- No code is generated for a program with any error.
- Generated Rust starts with `#![forbid(unsafe_code)]`, so rustc itself rejects any `unsafe`. A program that uses `ui` or `audio` starts with `#![cfg_attr(windows, deny(unsafe_code))]` and `#![cfg_attr(not(windows), forbid(unsafe_code))]` instead: on Windows the window and the sound card have no interface other than their C functions, and one module per device carries `#[allow(unsafe_code)]`. Everywhere else, and for every program without those two effects, `forbid` holds as before. Which modules those are is checked by a test.
- The same source and compiler version produce byte-identical generated Rust.
- Tessera semantics (overflow, evaluation order, runtime error format, exit codes) are implemented explicitly in the runtime, never by relying on Rust defaults.
- Malformed input never crashes the compiler (fuzz test over mutated sources).

The command-line tools. Stage 0 (`compiler/target/release/tessera`):

```
tessera check <files...> [--json]
tessera build <files...> -o <output> [--opt 0|1|2|3] [--keep-rust <file.rs>]
tessera run <files...> [-- <args...>]
tessera emit-rust <files...> [-o <file.rs>]
tessera notes <files...>
```

Stage 1 (`bin/tessera1`, built by `build.sh`) has the same commands except `run`; `build` keeps the generated Rust next to the executable as `<output>.rs`, so it can be reviewed.

```
tessera1 check <entry.tes> [--json]
tessera1 build <entry.tes> -o <output> [--opt 0|1|2|3] [--library]
tessera1 emit-rust <entry.tes> [-o <file.rs>]
tessera1 notes <entry.tes>
tessera1 test <entry.tes> [-o <test binary>]
tessera1 format <files...> [--check]
tessera1 ast <file.tes>
tessera1 render <file.json> [-o <file.tes>]
tessera1 edit <entry.tes> (--ops <ops.json> | --op <json>) [--dry-run] [--json]
tessera1 fix <entry.tes> [--dry-run] [--json]
tessera1 grammar [--format gbnf|ast-schema|edit-schema] [--depth N]
tessera1 interpret <entry.tes> [-- <args...>]
tessera1 build <entry.tes> --wasm -o <file.wasm>
```

Every command that reads a source file also accepts a JSON syntax tree (`.json`, section 20.1) in its place.

Imported modules are found automatically; only the entry file is given.

Compiler exit codes: 0 ok, 1 compile errors, 2 usage error, 3 Rust backend failure (always a compiler bug). Stage 1 reports an unreadable source file as runtime error R006 (exit code 70).

Both compilers embed the same runtime `runtime/core.rs`. Stage 1 adds `runtime/extra.rs` to every program and, per program, `runtime/web.rs` for decimals, time, randomness, crypto, the database, the network or `handle`, always together with `runtime/crypto.rs` (the cryptographic primitives) and `runtime/tls.rs` (TLS 1.3 for the HTTP client and server); `runtime/ui.rs` and `runtime/win32.rs` for `ui`; `runtime/audio.rs` and `runtime/winmm.rs` for `audio`. `runtime/adapter.rs` is the C interface of `--library` builds, `runtime/wasm.rs` the host interface of `--wasm` builds (section 21.3). Stage 1 carries all of them, and the standard modules in `std/`, as `stage1/runtime_text.tes`, generated by `tools/embed_runtime.tes`.

Four runtime files contain `unsafe` code, and they are the only ones: `adapter.rs` (the C interface a web host calls), `wasm.rs` (the functions a JavaScript host offers a WebAssembly module, section 21.3), `win32.rs` (the window) and `winmm.rs` (the sound card). The last two are reached only on Windows and only by a program that asked for a window or sound; everything that can be done without the operating system's C functions is done in safe Rust.

Since draft 0.13 stage 1 inserts stack and time checks into generated functions and loops, so its Rust differs from that of the frozen stage 0. The parity test therefore compares behaviour: every program shared by both compilers must give the same output, error messages and exit code.

### 16.1 Systems

The compiler and the programs it builds run on Linux and on Windows; the only thing a system must provide is `rustc`, which the compiler starts to turn its generated Rust into an executable. Both are tested on every change. macOS is not: it takes the unix side of every `#[cfg]` in the runtime, exactly as Linux does, so everything except `ui` is expected to work there, but that is an expectation and not a measurement.

| | Linux | Windows | macOS |
|---|---|---|---|
| Compiler, console programs, files, modules, `std/` | yes | yes | expected, untested |
| Web applications (`handle`, `serve`, CGI, `--library`) | yes | yes | expected, untested |
| `db` (needs the `sqlite3` command) | yes | yes, once `sqlite3` is on `PATH` | expected, untested |
| `https://` in `net_request`: the trusted certificates | the system CA bundle | the certificate store, read through PowerShell (untested) | `/etc/ssl/cert.pem`, expected, untested |
| `ui` | X11 | GDI | no backend |
| `audio` | `aplay` or `pw-cat` | the sound card itself | `TESSERA_AUDIO_PLAYER` only |

On Windows:

- `build -o name` writes `name.exe`; a name that already has an extension is left alone. The generated Rust is `name.rs` in both cases.
- `/` and `\` both separate path segments. A path is otherwise passed through unchanged, so a diagnostic names the file the way the person typed it and both compilers point at the same place.
- Source files must still use Unix line endings; a carriage return is E0101. The repository normalises every text file to LF (`.gitattributes`), so a checkout with `core.autocrlf=true` builds.

## 17. Bootstrapping plan

1. **Stage 0** (done): the compiler in `compiler/`, written in Rust.
2. **Stage 1** (done): the compiler in `stage1/`, about 5,100 lines of Tessera, compiled by stage 0. Until draft 0.12 it generated byte-identical Rust to stage 0 for every program without `ui`; since draft 0.13 (stack and time checks) the shared programs are tested to behave identically instead, and to report the same error codes.
3. **Stage 2** (done): stage 1 compiling its own source produces exactly the Rust that stage 0 produces for it, so a stage 2 binary is identical in behavior to stage 1. `build.sh` checks this fixpoint on every build.
4. **From now on** language changes are made in stage 1. Stage 0 stays frozen at draft 0.3 plus `process` and `file_exists`. Draft 0.4's `ui` effect was the first feature that exists only in stage 1.
5. **Bootstrap chain.** When stage 1 itself started using modules (draft 0.5), stage 0 could no longer build it, so a module-free copy of stage 1 bridged the gap; in draft 0.6 that copy was generated from `stage1/`. Once stage 1's own source used `Option` and `with`, no copy that stage 0 can build could build stage 1 any more without a growing chain of snapshots. Since draft 0.7 the bootstrap is therefore a **seed**: `bootstrap/tessera1.rs`, the Rust code stage 1 generates for itself. `rustc` compiles the seed into a compiler that builds `stage1/`, and `build.sh` checks that stage 1 then rebuilds itself to a fixpoint. The first seed was produced through the full chain from stage 0 (stage 0, then the flat copy, then stage 1) and was byte-identical to stage 1's own output. To let stage 1 use a new feature: implement it without using it, regenerate the seed, then use it (see `bootstrap/README.md`). Stage 0 remains the independent reference implementation for the parity tests.

## 18. Deferred, in expected order

1. Typed IR export with notes. (The formatter of section 14 is complete since draft 0.24.)
2. Model First Programming steps 5 to 7 (section 20.5): a machine-readable specification generated from the compiler, contracts or property tests per function, and a source map from tree nodes to runtime traces.
3. User-defined generics (typed JSON decoding, generic containers).
4. Closures, nested patterns, `Set`, constants.
5. A native SQLite or PostgreSQL client, which needs a decision on runtime dependencies (TLS was added without any in draft 0.22).
6. Notes on statements.

What complete applications, web applications and nginx/Apache hosting need beyond this list is analysed in [`application-roadmap.md`](application-roadmap.md), with a proposed order and the decisions that have to be made first.

Not planned for the POC: concurrency, FFI, `unsafe`, package management (fetching modules from outside the project), web frameworks.

## 19. Open questions

These should be answered by writing real programs, starting with the stage 1 compiler:

1. **Token cost.** Does readable spacing and `uses io` instead of `!{io}` cost meaningfully more tokens with the target tokenizer? Measure on the sample programs and the stage 1 compiler.
2. **Error on unused effects (E0302).** Helpful or annoying during refactoring?
3. **Immutable lists.** Is "rebuild with `append`/`replace_at`" acceptable in a real compiler, or is in-place mutation needed? *Draft 0.14:* acceptable for the compiler, not for frame buffers and game state; `set xs[i] = v` and `set r.field = v` change one value of a `var` in place, keeping value semantics.
4. **Note placement.** Above the declaration (current) or inside it?
5. **Naming enforcement as error.** Keep unless real programs show friction.
6. **Missing result type.** How painful is `fail` plus small hand-written enums for error handling in the stage 1 compiler? This decides the design of item 1 in section 18.

Findings from writing the stage 1 compiler (5,100 lines):

- **Optional values were missing** and **record update was verbose**. Both are addressed in draft 0.6; see the measurement at the end of this section.
- **Threading state through every call works but is noisy.** Most parser and checker functions return a record of (new state, result). This is clear for review but doubles signature sizes; worth revisiting together with the result type.
- **The performance model held up.** With the in-place `set x = f(x)` rule, stage 1 compiles its own source in about 0.05 seconds.

Findings from splitting stage 1 into eight modules (draft 0.5):

- **Qualified names cost little in total but cluster.** The conversion added 1,425 qualified references (`ast.Type.Int`, `source.error`, ...), 3.3% more characters overall. Half of them are in the checker, which pattern-matches heavily on types from `ast`. If this proves tiresome in review, the first relaxation to consider is importing specific types by name, not wildcards.
- **The new checks found real problems right away.** Converting the compiler surfaced four public functions returning types other modules could not name (E0609).
- **`uses nothing` works as documentation.** Every compiler phase is imported with `uses nothing`, so the compiler itself guarantees that only `main.tes` touches files or starts processes.

Measurement of draft 0.6 (`Option` and `with`), on stage 1 excluding the generated runtime text. The rewrite was checked to be behavior-preserving: the old and new compiler produce byte-identical output for all 82 test programs and diagnostics.

| | Before | After |
|---|---|---|
| Lines | 5,255 | 5,217 (-0.7%) |
| Characters | 217,362 | 212,273 (-2.3%) |
| Functions | 256 | 245 (11 one-field setter helpers removed) |
| Hand-written optional enums | 2 | 0 |
| Records rebuilt field by field to change one field | 27 | 5 (the rest construct genuinely new values) |
| `[0]` indexing | 69 | 33 (the rest index real lists, such as `args[0]`) |
| `length(...) > 0` / `== 0` tests | 60 | 32 |
| Lines over 150 characters | 128 | 117 |

- **Size barely changed; clarity did.** A `match` on an `Option` takes more lines than `if length(x) > 0 { x[0] }`, which cancels most of the savings from `with`. What changed is that every "maybe" is now visible in a type (`Option[Local]` instead of `List[Local]`), the compiler forces both cases to be handled, and there is no `[0]` left that could fail at run time.
- **`with` paid off most in state-passing code.** The checker's `report` went from a 399-character record rebuild to `c with { diagnostics: append(c.diagnostics, d) }`, and adding a field to `Checker` no longer touches eight helpers.
- **The next pressure point was records like `NameLookup` with `found`/`reported` flags.** They encode "success, failure already reported, or failure still to report". Draft 0.7 addresses this; see below.

Measurement of draft 0.7 (`Result` and `try`). Two places in stage 1 were rewritten, and again old and new compiler produce byte-identical output for all 92 test programs and diagnostics, and identical messages and exit codes for invalid command lines.

- **Option parsing** (`stage1/main.tes`): `parse_options` returns `Result[Options, Text]` instead of an `Options` record with an `error` field that every caller had to check. A missing value after `-o` or `--opt` is passed up with `try value_after(arguments, index)`. The function grew from 47 to 51 lines, but the loop no longer threads a `problem` variable (8 uses before, 2 after) and `Options` has no field that is only meaningful when parsing failed.
- **Name lookup** (`stage1/checker.tes`): the qualified-name logic is now a pure function, `qualified_name`, that returns `Result[Text, LookupFailure]` and uses `try` for each step (find the import, find the declaration, check it is public). A single function turns failures into diagnostics. The `found`/`reported` flags (16 uses) became one `Result[Text, Unresolved]`, and `resolve` calls `lookup_type` once instead of three times. The checker grew from 2,037 to 2,079 lines, mostly because each failure case is now a named enum variant with its own `match` arm.
- **Clearer, not shorter.** As with `Option`, explicit cases cost lines and buy clarity: every way a lookup can fail is listed in one enum, and the compiler checks that each is handled.
- **Found: branches that each fix half of a `Result` do not combine.** `if found { Result.Ok(name) } else { Result.Error(reason) }` needs an annotation, because each branch only knows one of the two types. Fixing this needs a type for "not known yet" that is distinct from the error-recovery type; it is the next candidate for the checker.
- **Found: `Result` fits pure code better than state-passing code.** The checker threads its state (diagnostics, used imports) through every call, and that state must survive failures too. The clean split was to make the lookup itself pure and keep the state changes in one place. Code that is already pure (option parsing, the lookup) benefits most.

Draft 0.8 was the first change driven by the measurement harness (`harness/`, see its README). The harness counts real Claude tokens of Tessera code per language variant, and lets a model solve 18 programming tasks per variant, measuring compile success, test success, fix rounds and tokens. Its static measurement of the compiler rewrite, by characters (token counts need API access and are produced by `harness.py tokens`):

| | Characters | Lines over 150 characters | `" +` / `+ "` text joins |
|---|---|---|---|
| Draft 0.7 compiler | 231,982 | 127 | 847 |
| With `.Variant` | 220,971 (-4.7%) | 118 | 847 |
| With `.Variant` and interpolation | 219,100 (-5.6%) | 108 | 106 |

- **The rewrite was mechanical and checked by the compiler.** Every `Type.Variant` was shortened, and the 31 of 1,166 the checker rejected (E0518, no type known) were restored. All 234 lines with text joins were converted to interpolation. The old and new compiler produce identical output for all 102 test programs.
- **`.Variant` removed the most text; interpolation removed the most noise.** Interpolation saves few characters, but 87% of the text joins disappear, which is what makes long diagnostic messages readable.
- **`.Variant` costs some context in tables.** `builtin("range", [.Int, .Int], list_of(.Int), "")` is shorter but no longer says that these are `ast.Type` values; the reader has to know the parameter types.
- **Whether this helps a model write correct code is measured by `harness.py run`**, which compares the variants `baseline`, `indent-2`, `indent-4`, `shorthand`, `interpolation` and `compact`.

First model runs (2026-11-08, Claude Opus 5.5, effort `medium`, 18 tasks). Averages per task run; program tokens are the final passing program.

| Run | Variant | Runs | Passed | First try | Output tokens | Input tokens | Program tokens |
|---|---|---|---|---|---|---|---|
| Draft 0.8 | 4 spaces | 36 | 100% | 94% | 1,082 | 4,294 | 545 |
| Draft 0.8 | 2 spaces | 36 | 100% | 100% | 1,098 | 4,029 | 527 |
| Draft 0.8 | no indentation | 36 | 100% | 100% | 1,052 | 3,988 | 480 |
| Draft 0.9 | `baseline` (no indentation) | 54 | 100% | 100% | 1,024 | 4,181 | 481 |
| Draft 0.9 | `shorthand` | 54 | 100% | 100% | 976 | 4,529 | 468 |
| Draft 0.9 | `interpolation` | 54 | 100% | 98% | 984 | 4,458 | 478 |
| Draft 0.9 | `compact` (both) | 54 | 100% | 98% | 942 | 4,806 | 465 |
| Draft 0.10 | `baseline` (shorthand, interpolation and new built-ins in the reference) | 54 | 100% | 100% | 816 | 4,690 | 345 |
| Draft 0.11 | `baseline` (`+` on lists, `exit_with`, `to_lower`/`to_upper`) | 54 | 100% | 98% | 842 | 4,940 | 341 |
| Draft 0.12 | `baseline` (`if` typing fix, `parse_int` without `+`) | 54 | 100% | 100% | 769 | 4,844 | 325 |
| Draft 0.13 | `baseline`, the same 18 tasks | 54 | 100% | 100% | 678 | 6,946 | 312 |
| Draft 0.13 | `baseline`, all 21 tasks (plus `word-frequency`, `json-total`, `web-router`) | 63 | 100% | 100% | 732 | 6,948 | 320 |

- **The model writes Tessera correctly from the reference alone.** Every task run passed. The draft 0.8 run had only two first-attempt failures, `for _k in ...` and `Kind.A => set x = ...`; draft 0.9 accepts both forms, and none of the 216 draft 0.9 runs failed on them. The two remaining failures were a stray line `out = out` in the same task, not a language problem.
- **No indentation saves 12% of program tokens** (545 to 480) at no cost in correctness, which led to the canonical layout in section 14.
- **`.Variant` and interpolation in the reference save about 8% of output tokens** (`compact` against `baseline`) and 3% of program tokens; the longer reference costs about 600 more input tokens per request, most of them cached.
- **Found: a quarter of what the model writes is integer parsing.** In the draft 0.8 run, 78 hand-written `parse_int`-style functions make up 24% of all program lines; sorting (4%), splitting text (3%), searching lists (3%) and case conversion (1%) follow. These are candidates for the standard library; draft 0.10 adds `parse_int`, `split`, `sort`, `index_of` and `contains` for them.
- **Draft 0.10 made programs 29% smaller and output 21% cheaper** than the draft 0.9 baseline, on the 16 tasks whose starter code did not change, with every run passing on the first attempt. The model used the new built-ins wherever they fit: no hand-written parser or sort function is left, and no `set x = x + ...`. The largest drops are in tasks that parse or sort (`sort-numbers` -68%, `anagrams` -49%, `ledger` -40%). Tasks without that work (`brackets`, `fix-compile`, `rpn`) did not change beyond noise.
- **Next candidates** (draft 0.10 run, 54 programs, 2,070 lines): `set xs = append(xs, item)` (53 times), a printed error followed by `exit(1)` (53 times, plus helper functions that do only this), and `to_lower` written by hand in a third of the programs that handle words.
- **Draft 0.11 replaced those patterns completely but saved little.** No program uses `append` or prints an error before `exit(1)` any more (54 `+= [...]`, 78 `exit_with`), and no `to_lower` is written by hand. Program tokens fell 1% and output tokens rose 3%, both within the noise between runs: the replaced statements were short already. Its value is one way to build lists and errors on standard error, not size.
- **Found: `.None` in the `then` branch of an `if` has no type.** `if bad { .None } else { parse_int(text) }` is E0518, while the same with the branches swapped compiles, because only the `else` branch is checked against the type of the other. The model's one failed first attempt in the draft 0.11 run was this; the help text (`write the type in front, e.g. MyEnum.None`) led it to write `Option[Int].None`. Draft 0.12 checks the `then` branch against the `else` branch when it has no type of its own, and the help text now suggests a type annotation.
- **The 18 tasks are saturated.** Since draft 0.10 every run passes, and since draft 0.12 every run passes on the first attempt; the remaining differences between runs are a few percent and partly noise. Further changes should be measured with harder tasks, in particular the application features of [`application-roadmap.md`](application-roadmap.md) (maps, JSON, request handlers).
- **Found (draft 0.12 run): unwrap or stop.** 22 of 54 programs contain `let n = match parse_int(x) { .Some(v) => v .None => exit_with(1, "...") }` or the same with `return .Error(...)`; four lines to say "the value, or stop". A form such as `let n = parse_int(x) else exit_with(1, "...")`, where the `else` part must have type `Never`, would make it one line.
- **Found: building a `List[Text]` only to `join` it** (22 programs), from a loop such as `for n in numbers { set parts += [to_text(n)] }`. Named functions as values (`map(numbers, to_text)`) would remove these loops; see the roadmap.
- **Found: tasks can contradict built-ins.** `word-count` says only ASCII letters change case, so the model correctly wrote its own `ascii_lower` instead of the Unicode `to_lower`. This is a property of the task, not a gap in the language.
- **Draft 0.13 (2026-11-08): output 12% cheaper on the same 18 tasks, and the application tasks pass first time.** All 63 runs passed on the first attempt, including the new map, JSON and web-handler tasks. The 54 programs of the old tasks use `let ... else` 39 times and `map(...)` 8 times; `word-count`, reworded so it no longer contradicts `to_lower`, dropped from 282 to 166 program tokens. The cost is the reference: about 2,100 more input tokens per request (most of them cached), because it now describes maps, decimals, functions as values, the standard modules and web applications.
- **Found: lists are still built only to be joined** in 16 programs, now usually where each element is formatted with more than one value (`"\(a) \(b)"`), which a named function cannot express without a helper. Closures would remove these loops; they are deferred (section 18) until measurement shows they are worth the extra review burden.

Findings from writing Tesseradoom (`examples/tesseradoom/`, about 3,000 lines in 13 modules, drafts 0.14 to 0.18):

- **A missed in-place update cost a factor of 20.** The renderer threads its frame buffer through drawing functions as a record: `set c = draw_wall(view, c, ...)`. Draft 0.17 moved only text, lists, maps and bytes in that form, and shared records, so every drawing call copied the 64,000-pixel buffer: 44 frames per second. Moving records and enums too (draft 0.18) made the same program run at about 1,000. Two related forms still copy and had to be rewritten by hand: `return f(c)` and `set c = if a { f(c) } else { g(c) }`. Extending the rule to any last use of a variable would remove this class of trap; it needs a liveness analysis in the code generator.
- **`var copy = parameter` now takes the parameter** when that is its only use (draft 0.18), so helper functions can change a list they receive without copying it.
- **`range(a, b)` with `a > b` stops the program (R004).** That caught real bugs, but also made empty loops crash in three places (spans clipped to the screen). A `range` that is empty for `a >= b` would be the more common expectation; this is a candidate change, to be decided by measurement.
- **No nested patterns** showed up twice (`.Error(.NotFound)`, `.Function(_, .Float, _)`); both were easy to rewrite with `==` or an `if`.
- **Fixed point worked.** All game geometry is 16.16 `Int` with `shift_left`/`shift_right`; the bounds stated in section 7.6 were enough to avoid overflow everywhere except one quotient (a ray almost parallel to a wall), which now checks its magnitude first.

Review of the language after Tesseradoom's sound, weapons, menus and saving (draft 0.20). Done: the empty `range` and the moves listed in section 3. Open candidates, to be decided by measurement or by the maintainer:

- **Import aliases** (`import std/keys as input`). Module names took local names away twice (`screen`, `keys`); renaming the local each time worked but read worse.
- **Built-in names in modules.** `fixed.to_int`, `automap.draw_line` and `overlay.text_width` had to be renamed although modules are always used qualified. Allowing it needs a rule for unqualified use inside the module itself.
- **Nested patterns** (`.Error(.NotFound)`). Rewritten with `==` each time; cheap to work around, but the most common pattern request.
- **Tuples or small anonymous records.** `std/rng.roll` returns `[value, seed]` as a `List[Int]`, as do several Tesseradoom helpers; a pair type would say what each element is.
- **Last use everywhere.** Draft 0.20 moves values at returns and in branches; a full liveness analysis would also cover `let y = f(x)` when `x` is not used again.

Findings from building the Model First Programming tools (draft 0.24, section 20):

- **The parser was lossy in four places**: comments, parentheses, text interpolation (expanded by the lexer) and `+=` (expanded by the parser). The printer recovers the last three by rule, which is what defines the canonical form; comments are now collected by the lexer. The compiler's own source, formatted canonically, generates the same Rust as before apart from source positions.
- **Claude's structured outputs accept no recursive JSON Schema.** A syntax tree is recursive, so grammar-constrained generation of whole trees with Claude needs the unrolled schema (`--depth`), whose size grows linearly with the depth (about 95 KB at depth 12). Edit operations are not recursive and fit strict tool use directly. Which of the two helps more is to be measured.
- **The 13 typical mistakes of `tests/mfp/fix/broken.tes` need one `fix` run.** Twelve are fixed automatically in two rounds (a removed import and an added one would overlap in the first); the remaining one (`p.z` on a record with `x` and `y`) has no single right answer and is left to the model.

Findings from the interpreter and the WebAssembly build (draft 0.25, section 21):

- **Value semantics through a value type cost copies.** `set xs[i] = v` on a large list is in place in compiled code, but the interpreter keeps a list inside a `Value` enum, and taking it out of the enum leaves a second reference, so the list is copied. `tests/stage1_run/record_moves.tes` (100,000 updates of a million elements) does not finish. Moving the fields of a value out on its last use in `match` (a code generation change, no language change) would remove the copy; until then the interpreter suits scripts, not frame buffers.
- **The browser's stack, not the WebAssembly memory, limits recursion.** About 1 KB of the JavaScript engine's own stack per interpreted call, at every optimization level, so V8 allows about 940 nested calls. `tessera.js` sets the limit to 700, which gives R013 with the program's location; a native `interpret` allows 100,000.
- **Overflow checks without overflowing.** A `Float` operation that overflows is itself a runtime error in Tessera, so the interpreter decides on exactly scaled operands (`float_add`, `float_mul`, `float_div` in `stage1/interp.tes`) and the boundary tests in `tests/interpret/fail/` cross each limit by the smallest step. `Decimal` `*` and `/` are checked by division against the largest value and may differ from the compiled program in the last unit at the boundary; `round` is decided exactly from the digits.

## 20. Model First Programming

Tessera is written, reviewed and repaired mainly by models. **Model First Programming (MFP)** is the paradigm that follows from that: the program is a structured object that models change through typed operations, a model can be kept from writing invalid syntax at all, and every error the compiler finds says how to repair it. Text remains, as the one canonical view of the program for reading and review. Draft 0.24 implements the first four steps; the order follows from measurement (`harness/`), which comes first:

1. The syntax tree is the source of truth (20.1).
2. Semantic edit operations instead of text diffs (20.2).
3. Validity during generation: an exported grammar for constrained decoding (20.3).
4. Machine-actionable diagnostics (20.4).
5. A machine-first specification, generated from the compiler (deferred, 20.5).
6. Correctness checkable by machines: contracts or property tests per function, effects and capabilities as hard gates (deferred).
7. A debugging path: stable node ids and a source map from tree nodes to runtime traces (deferred).

### 20.1 The syntax tree as the program

`tessera1 ast <file.tes>` writes the syntax tree of one file as JSON; `tessera1 render <file.json>` prints the canonical text of a tree (section 14); every command that reads source files (`check`, `build`, `test`, `emit-rust`, `notes`, `edit`, `fix`) accepts a `.json` tree in place of a `.tes` file and reads it as its canonical text, so diagnostics point into that text. Converting is lossless: text to tree to text gives the canonical text, and the canonical text gives the same tree, comments included. Every file in the repository is tested for both.

The JSON follows the canonical text, not the compiler's internal form. A file is `{"tessera_ast": 1, "items": [...]}`, with one declaration per line. Every node is an object with a `kind`:

| Node | Kinds and fields |
|---|---|
| Declarations | `fn` (`name`, `params` [`{name, type}`], `returns`, optional `pub`, `notes`, `uses`, and `body`: a `block` or, for `= value`, an expression); `type` (`fields` [`{name, type}`]); `enum` (`variants` [`{name, fields?}`]); `import` (`path`, `uses` when limited, `[]` for `uses nothing`); `test` (`name`, `body`) |
| Types | a name (`"Int"`, `"shapes.Shape"`), `{"name": "List", "args": [...]}`, or `{"name": "fn", "params": [...], "returns": ..., "uses": [...]}` |
| Statements | `let` and `var` (`name`, optional `type`, `value`, optional `else` for `let ... else`); `set` (`target`: a name with `field` and `index` steps, optional `op` `+=` or `-=`, `value`); `for` (`variable`, `iterable`, `body`); `while` (`condition`, `body`); or any expression |
| Expressions | `int` (`value`), `number` (`text`: a `Decimal` or `Float` literal), `bool`, `text` (`value`), `interpolation` (`parts`: texts and expressions), `unit`, `name`, `list` (`items`), `map` (`entries` [`{key, value}`]), `call` (`function`, `args`), `index`, `field` (`target`, `name`), `with` (`target`, `fields`), `unary` (`op` `-` or `not`, `operand`), `try`, `binary` (`op`, `left`, `right`), `if` (`condition`, `then`, optional `else`: a `block` or an `if`), `block` (`statements`, the last one being the block's value when it is an expression), `match` (`value`, `arms` [`{pattern, body}`]), `record` (`type`, `fields`), `variant` (optional `type`, `variant`, optional `args`), `return` (optional `value`), `break`, `continue` |
| Patterns | `wildcard`, `int`, `text`, `bool`, `variant` (optional `type`, `variant`, optional `bindings`) |

A node that stands on its own line (a declaration, statement, `match` arm, field or variant) may carry `comments`, the comment lines above it; at the top level an empty string stands for a blank line. Blocks, `match` and the file may carry `end_comments`, the comments before their closing brace or at the end. There are no positions in the tree: positions belong to the canonical text.

Choices that keep one form: `set x = x + e` is written `{"kind": "set", "op": "+=", ...}`, and a chain `"a" + to_text(x)` is an `interpolation`, because those are their canonical texts. Reading a tree joins neighbouring texts in an interpolation and drops empty ones, as the lexer does.

An invalid tree is E0701, with the JSON path of the first problem (`items[3].body.statements[0]: unknown expression kind ...`).

### 20.2 Edit operations

`tessera1 edit <entry.tes> --ops <ops.json>` (or `--op '<json>'`) applies one operation or a list of them to the program whose entry file is given. Operations apply in order to the trees of the program's files; after each one the changed files are printed and read again. When all have applied, the whole program is checked with the changed texts. Only if it checks are the files written, each in its canonical form (a `.json` file as a tree again). Otherwise nothing is written and the diagnostics, pointing into the would-be text, are reported. `--dry-run` checks without writing; `--json` reports in JSON.

Every operation has an `op` and an optional `file` (default: the entry file; a module as `path/to/module.tes`, the way the compiler finds it):

| Operation | Fields | Effect |
|---|---|---|
| `add_fn` | `source`, optional `after` or `before` | Adds one function (with its notes) after or before the named declaration, or at the end |
| `add_item` | `source`, optional `after` or `before` | The same for any declarations, also several; an `import` goes after the last import |
| `replace_fn`, `replace_item` | `name`, `source` | Replaces the named declaration. Without notes in `source` the old notes are kept; comments above it stay |
| `remove_fn`, `remove_item` | `name` | Removes the declaration (an import by its path) and the comments directly above it |
| `rename_symbol` | `from`, `to`, optional `fn` | With `fn`: a parameter or local variable of that function. Otherwise the function or type `from` of `file`, also in the modules that use it as `alias.from`; or `Type.variant` / `Type.field`, renamed wherever the type checker finds that variant or field, so also through a `.Variant` shorthand, a pattern or a module alias (this needs a program that checks) |
| `set_effect` | `fn`, `uses` | Sets the `uses` list of a function |
| `add_match_arm` | `fn`, `match`, `arm`, optional `after` | Adds `pattern => body` to the `match` whose value is written `match` in canonical form: after the arm with pattern `after`, else before a final `_` arm, else at the end |
| `insert_stmt_before`, `insert_stmt_after` | `fn`, `anchor`, `source` | Inserts statements before or after the statement whose canonical first line is `anchor` |
| `replace_stmt`, `remove_stmt` | `fn`, `anchor`, `source` / — | Replaces or removes that statement (a compound statement with its body) |

`source` is canonical Tessera text, the way it would stand in the file; comments in it are kept. An anchor is the first line of a statement as `tessera1 format` writes it, for example `set total += shapes.area(s)` or `while index < count {`; it must match exactly one statement of the function, searched in every nested block. A function with an `= value` body becomes a block when a statement is inserted.

Refusals: E0702 (unknown operation or missing field, or the operations are not valid JSON), E0703 (the file is not part of the program), E0704 (the target is not found), E0705 (the target is ambiguous, a declaration of that name exists already, or a field or variant rename on a program that does not check), E0706 (`source` does not parse as what the operation needs). They point into the operations and name the operation by its position.

### 20.3 Grammar export

`tessera1 grammar --format <format>` writes:

- `gbnf` (the default): the canonical text in GBNF, the grammar format of llama.cpp, which XGrammar and similar decoders also read. It is stricter than the parser: it accepts only the canonical layout (no indentation, single spaces) and encodes the naming rules (`snake_case` names, `PascalCase` types), so text decoded with it is canonical. Type and effect errors are still the compiler's to find. The file is `grammar/tessera.gbnf`.
- `ast-schema`: the JSON Schema (draft 2020-12) of the tree of section 20.1. A tree nests without limit, so the schema is recursive. With `--depth N` (1 to 64) it is unrolled into N levels (`expr_0` to `expr_N`, the last with only leaf expressions), because some constrained decoders, among them Claude's structured outputs, accept no recursive schema. Every object has `additionalProperties: false`.
- `edit-schema`: the JSON Schema of the operations of section 20.2, one definition per operation; each also serves as the input schema of a separate tool (strict tool use).

`harness/tools/check_grammar.py` checks the export independently and without dependencies: every canonical file in the repository is accepted by the GBNF grammar (with an Earley recognizer) and its tree validates against both forms of the schema; the unrolled schema has no `$ref` cycle.

### 20.4 Machine-actionable diagnostics

Besides its code, position, message, label and help (section 13), a diagnostic can carry:

- `expected` and `actual`: what the compiler wanted and what it found, such as two types (`Option[Int]`, `Int`) or two argument counts.
- `fixes`: repairs, each a `message` and a list of `edits` (file, line, column, end line, end column, `replacement`; an empty range inserts). All edits of one fix belong together. A fix is offered only where one change is clearly right, and the compiler checks before reporting it that it applies to the source.

`check --json` includes them, together with `source_line`, the text of the line the diagnostic points at. The text output shows each fix with the line it produces:

```
error[E0205]: `label` cannot be changed
  --> app.tes:25:5
   |
25 | set label = label + "!"
   |     ^^^^^
   |
help: declare it with `var` instead of `let` (line 24); parameters can never be changed
fix: declare `label` with `var`
  24 | var label = "point"
```

Diagnostics with fixes in draft 0.24: tabs and carriage returns (E0101), `_name` (E0103), a comma where a line break belongs and an assignment without `set` (E0110), a method call `xs.length()` (E0110, E0606: `length(xs)`), `else` on its own line (E0111), a blank line after a note (E0401), unknown names, functions, types, fields, variants and effects close to a known one or written as in another language (`len`, `str`, `push`, `String`, ...; E0202, E0203, E0208, E0209, E0303), `let` changed by `set` and `var` never changed (E0205, E0206), a missing or unused effect (E0301, E0302), a value of the wrong type with one obvious conversion (`float(...)`, `decimal(...)`, `.0` after a literal, `to_text(...)`, `.Some(...)`, `.Ok(...)`; E0501, E0503), a value that is not used (E0508: `let _ = ...`), `try` on a value that is no `Result` or `Option` (E0515), an unused or repeated import (E0610, E0604) and a standard module used without its import (E0606).

`tessera1 fix <entry.tes>` applies, per round, every fix of a diagnostic that has exactly one fix and does not overlap another, then checks again, for at most 10 rounds (one fix can uncover the next error). It writes the files in canonical form and reports what is left; `--dry-run` and `--json` as for `edit`. A diagnostic whose repair needs a choice keeps its help text and no fix, so `fix` never guesses.

### 20.5 Deferred

Steps 5 to 7 wait until the effect of steps 1 to 4 on fix rounds and tokens has been measured with the harness. The candidate design: a typed reference (grammar, type rules, effect rules, built-in signatures) generated by `tessera1` itself, so it cannot drift from the implementation; contracts (`requires`/`ensures`) or `test` declarations required per public function and run by the compiler; and stable node ids carried through code generation into runtime error locations.

## 21. Tessera in the browser

A model writes a program, as text or as a JSON syntax tree (section 20.1), and a web page runs it, much like a WebAssembly module: no server, no Rust toolchain. Three parts make that possible: an interpreter in the compiler (21.1), the compiler built as WebAssembly (21.3) and a JavaScript library around it (21.4).

### 21.1 `tessera1 interpret`

```
tessera1 interpret <entry.tes | entry.json> [-- <args...>]
```

checks the program like `check` and, when it has no errors, runs `main` from the checked syntax tree without generating Rust. Arguments after `--` are the program's `args()`. The exit code is the program's: 0, the code given to `exit`, or 70 after a runtime error.

The interpreter offers the effects `io`, `env`, `time`, `random` and `log`. `main` declares every effect the program uses (section 9.2), so `interpret` decides from its `uses` clause, before anything runs: any other effect is E0707, with the effect as the location.

```
error[E0707]: `interpret` cannot run a program that uses `fs`
  --> notes.tes:1:28
   |
 1 | fn main() -> Unit uses io, fs {
   |                            ^^
   |
help: the interpreter, also in the browser, offers only the effects io, env, time, random and log: remove `fs` from `main` and the functions that need it, or compile the program with `tessera1 build`
```

`test` declarations and web applications (`handle`) are not interpreted.

### 21.2 Semantics

An interpreted program behaves as its compiled form: the same standard output, the same standard error, including every runtime error's code, message and location (section 12), and the same exit code. The test suite checks this for every test program in the effects above (`interpreter_matches_compiled_programs`).

- Built-in functions are called by the interpreter itself, so their results are the compiled ones by construction.
- Every runtime error a built-in or an operator can raise is checked by the interpreter before the operation, so that its location is the program's. The boundary tests in `tests/interpret/` cross each limit by the smallest step. One difference remains: `Decimal` `*` and `/` within one unit (10^-9) of the largest `Decimal` may be decided differently from the compiled program (section 19).
- Nested calls are limited to 100,000 natively; the environment variable `TESSERA_INTERPRET_DEPTH` sets another limit (the browser uses 700). Beyond it, R013 at the called function.
- The interpreter is slower than compiled code, and `set xs[i] = v` copies the list (section 19).

### 21.3 WebAssembly builds

```
tessera1 build <entry.tes> --wasm -o <file.wasm>
```

compiles a program with `main` for `wasm32-unknown-unknown` (install it with `rustup target add wasm32-unknown-unknown`), with `runtime/wasm.rs` as the crate root. The module imports six functions from the JavaScript host, module `tessera`, and exports `memory` and `tessera_main() -> i32`:

| Import | Meaning |
|---|---|
| `get(key, key_len) -> i32` | Makes the host's value for `key` ready; its length in bytes, or -1 when there is none |
| `take(destination)` | Copies the value made ready by `get` into the module's memory |
| `put(key, key_len, value, value_len) -> i32` | Gives a value to the host: 0 accepted, -1 refused |
| `now() -> f64`, `monotonic() -> f64` | Milliseconds since 1970 (UTC), and from an arbitrary start |
| `random(destination, len)` | Secure random bytes |

Keys: `args` (the arguments, separated by NUL), `stdin` (all of standard input, read once), `env/NAME`, `file/PATH` (`get` reads a file, `put` writes one), `stdout`, `stderr` and `exit` (the exit code as decimal text). A runtime error or `exit` ends the run with a trap after `exit` was put, since Rust panics abort on this target; the host treats that trap as the end of the run. `sleep` waits on the host's clock. Directories, `list_directory` and `remove_file` are not offered. Only `runtime/wasm.rs` calls the imports; it is one of the four runtime files allowed to contain `unsafe` (section 16).

`tessera1 build stage1/main.tes --wasm -o browser/tessera.wasm` builds the compiler itself; `build.sh` does so when the target is installed.

### 21.4 `browser/tessera.js`

A JavaScript module without dependencies (browsers, Web Workers, Deno and Node 18+). Every call runs the compiler's command line in a fresh instance of the module, against files kept in memory:

```js
import { Tessera } from "./tessera.js";
const tessera = await Tessera.load();                 // tessera.wasm next to tessera.js
const result = tessera.run(programTextOrTree, { stdin: "", args: [], modules: {} });
// { exitCode, stdout, stderr, diagnostics }: diagnostics as in `check --json` when it did not compile
tessera.check(program)   tessera.format(text)   tessera.ast(text)   tessera.render(tree)   tessera.grammar("gbnf")
tessera.exec(["fix", "main.tes", "--json"], { files: { "main.tes": text } })   // any command
```

A program given as an object, or as text starting with `{`, is a syntax tree. `onStdout` and `onStderr` receive output as it is written. `<script type="text/tessera">` elements run when the page loads `tessera.js?auto`, with their output in the element named by `data-output`. A program runs on the calling thread, so a long one belongs in a Web Worker. `browser/test/run.sh` checks the library in headless Chrome.

