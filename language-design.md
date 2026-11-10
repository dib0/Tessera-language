# Tessera Language Design

**Status:** Draft 0.18. Implemented by the stage 1 compiler in `stage1/` (written in Tessera, self-hosting, itself split into modules). The stage 0 compiler in `compiler/` (Rust) implements everything except the `ui` effect (section 9.4), modules (section 9.5), `Option` (section 5.5), `Result` and `try` (section 5.6), `with` (section 5.2), `.Variant` shorthand (section 5.2), text interpolation (section 4.5), `for _` (section 8.1), statement arms in `match` (section 7.5), `set x += e` (section 6.2), `+` on lists (section 7.1) the built-ins `parse_int`, `split`, `sort`, `index_of`, `contains`, `to_lower`, `to_upper` and `exit_with` (section 10), and everything added in drafts 0.13 to 0.18: `Map`, `Decimal`, `Bytes`, function values, `let ... else`, the effects `time`, `random`, `log`, `db` and `net`, web applications, `test` declarations, the standard modules, `set` on fields and elements, the bit operations, binary files, the standard modules added in draft 0.16 and `Float` (sections 5.1, 6.1, 6.2, 7.6, 9.6 to 9.8, 10).
**Scope:** Everything described here is implemented and tested unless marked *not implemented*. Anything not described here is not part of the language.

This document is the authoritative description of Tessera. When the compiler and this document disagree, the document wins and the compiler has a bug, or the document is updated through an explicit decision (see section 19).

---

## 1. Design principles

These principles decide trade-offs. When two choices are otherwise equal, pick the one higher on this list.

1. **Safe by construction.** Unsafe behavior must be impossible to write, not merely discouraged.
2. **Readable at review time.** A reviewer, human or LLM, should understand what a function can do from its signature and what a line does without context from elsewhere.
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

The difference is deliberate: comments are for reading, notes are for tooling. See section 11.

There are no block comments (`/* */`). Nesting rules and unterminated comments are a known source of parser bugs and confusing diffs.

### 4.3 Identifiers and naming

| Kind | Style | Example |
|---|---|---|
| Functions, parameters, variables, fields | `snake_case` | `total_price`, `is_valid` |
| Types, enum variants | `PascalCase` | `Token`, `TokenKind`, `LeftParen` |
| Effects | lowercase single word | `io` |

Naming style is **enforced by the compiler** (E0207, an error, not a warning). Consistent names make code easier for an LLM to predict and for a human to scan. The case of the first letter is also how the parser tells types (`Token { ... }`, `TokenKind.Plus`) from values.

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

**Performance guarantee.** `set x = f(x, ...)` where `x` is `Text`, a list, map, `Bytes`, record or enum (records and enums since draft 0.18), and `x` appears exactly once on the right side, updates `x` in place. Building a list or text with `append`, `concat`, `replace_at` or `+` in a loop is therefore linear, not quadratic. Setting a place (`set xs[i] = v`, `set r.field = v`) takes constant time, plus one copy of the list or record on the first change after a copy of it was made elsewhere (copy on write). A function that changes a list it receives, `fn plot(cells: List[Int], i: Int) -> List[Int] { var out = cells ... out }`, works in place too when `var out = cells` is a statement of the function body itself (not inside a loop or branch) and the parameter is used nowhere else; called as `set grid = plot(grid, i)`, no copy is made. These are implementation strategies that do not change the meaning of the program: no other code can observe the old value.

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
Key(name: Text)                      // "a", "A", "7", " ", "Escape", "Enter", "Backspace", "Tab", "Left", "Up", "Right", "Down", "Shift", "Control", "Alt"
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

The POC implementation talks to a local X11 display (Linux) directly over its socket, in safe Rust, without libraries. Other platforms are not supported yet.

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
| `range` | `(start: Int, end: Int) -> List[Int]` | `start` up to, not including, `end`. R004 if `start > end`. |
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
| `net_request` | `(method, url, header_lines: List[Text], body: Bytes) -> Result[Bytes, Text] uses net` | Plain HTTP only; the raw response, for `std/http`. |
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

The runtime has no external dependencies, so SQLite is reached through the `sqlite3` program (a persistent child process per database) and the HTTP client has no TLS. Both are interim choices of draft 0.13; see the roadmap.

### 10.1 Standard modules

`std/...` modules are part of the compiler and written in Tessera, so they are checked like any other module and can be reviewed in `std/`:

| Module | Contents |
|---|---|
| `std/http` | `Request`, `Response`, `Header`; `text`, `html` (with a Content-Security-Policy), `json`, `redirect`, `not_found`, `with_header`, `with_cookie` (HttpOnly, Secure, SameSite=Lax); `header`, `text_body`, `form`, `cookies`, `cookie`, `match_path`; `sign`/`verify` (HMAC-signed values), `csrf_token`/`check_csrf`; the client `fetch` and `get_text` (effect `net`); `request` and `response_text` for tests. |
| `std/json` | `enum Json`, `parse`, `encode` (keys sorted), `quote`, `field`, `as_text`, `as_int`, `as_decimal`, `as_bool`, `as_list`, `text_field`, `int_field`, `from_int`. |
| `std/url` | `percent_encode`, `percent_decode`, `parse_query`, `parse_query_all`, `encode_query`, `path_segments`. |
| `std/html` | `escape`. |
| `std/bytes` | `read_u16_le`, `read_i16_le`, `read_u32_le`, `read_i32_le` (`(data: Bytes, at: Int) -> Int`), `read_ascii(data, at, count) -> Text` (stops at the first zero byte; bytes above 127 become `?`). Reading past the end is R001. |
| `std/math` | `min`, `max`, `abs` (R002 for the smallest `Int`), `clamp(value, low, high)` (R010 if `low > high`), `isqrt(value)` (largest `r` with `r * r <= value`; R010 for a negative value). |
| `std/fixed` | 16.16 fixed point (section 7.6): `one`, `from_int`, `floor_int` (the integer part, rounded toward negative infinity; `to_int` is the built-in for `Decimal`), `mul`, `div` (rounds toward zero; R003 for 0), `abs`; `sine(index, count)` and `cosine(index, count)`, the value at angle `2π · index / count` rounded to the nearest unit (computed with 28 fraction bits, so within 0.0003 of a unit from exact; only values that are almost exactly halfway can round the other way), and `sine_table(count)`, `cosine_table(count)` for indexes `0` to `count - 1`. Integer arithmetic only, so every platform computes the same tables. |
| `std/palette` | `read(data: Bytes, at: Int) -> List[Int]`: the 256 colors of a 768-byte RGB palette, packed like `rgb(red, green, blue)`; `red`, `green`, `blue` take them apart. |
| `std/ppm` | `encode(width, height, pixels: List[Int], palette: List[Int]) -> Bytes`: a binary PPM (P6) image of palette indexes, row by row from the top left. R010 if the number of pixels is not `width * height` or a side is not positive; R001 for an index outside the palette. Write it with `write_bytes`. |

The modules added in draft 0.16 use no effects; their tests are in `tests/std/`.

Graphics (effect `ui`, section 9.4):

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
| R004 | Invalid `range` or `slice` bounds, or another argument outside its allowed range |
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
- Every error suggests a concrete fix where one exists.
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

## 14. Canonical formatting

`tessera1 format <files...>` applies the layout rules that do not need a full pretty printer: no indentation, no trailing spaces, at most one blank line in a row and exactly one final newline; `--check` only reports files that would change. Because text literals cannot span lines, this never changes the meaning of a program. The remaining rules below (spacing inside lines, line length) are *not implemented yet*:

- No indentation: every line starts in the first column. Indentation has no meaning in Tessera, which is written and read mainly by models; braces carry the structure. Leaving it out saves about 12% of the tokens of a program (section 19). Tabs are never allowed.
- One space around binary operators and after commas and colons: `fn f(a: Int, b: Int) -> Int`.
- Opening brace on the same line.
- Maximum line length 100; longer expressions break after an operator.
- One blank line between declarations. Notes sit directly above their declaration.
- Formatting twice gives the same result as formatting once; comments and notes are preserved.

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
- Generated Rust starts with `#![forbid(unsafe_code)]`, so rustc itself rejects any `unsafe`.
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
```

Imported modules are found automatically; only the entry file is given.

Compiler exit codes: 0 ok, 1 compile errors, 2 usage error, 3 Rust backend failure (always a compiler bug). Stage 1 reports an unreadable source file as runtime error R006 (exit code 70).

Both compilers embed the same runtime `runtime/core.rs` (plus `runtime/ui.rs` for programs that use `ui`). Stage 1 adds `runtime/extra.rs` to every program and `runtime/web.rs` to programs that use decimals, time, randomness, crypto, the database, the network or `handle`; `runtime/adapter.rs` is the C interface of `--library` builds and the only runtime code with `unsafe`. Stage 1 carries all of them, and the standard modules in `std/`, as `stage1/runtime_text.tes`, generated by `tools/embed_runtime.tes`.

Since draft 0.13 stage 1 inserts stack and time checks into generated functions and loops, so its Rust differs from that of the frozen stage 0. The parity test therefore compares behaviour: every program shared by both compilers must give the same output, error messages and exit code.

## 17. Bootstrapping plan

1. **Stage 0** (done): the compiler in `compiler/`, written in Rust.
2. **Stage 1** (done): the compiler in `stage1/`, about 5,100 lines of Tessera, compiled by stage 0. Until draft 0.12 it generated byte-identical Rust to stage 0 for every program without `ui`; since draft 0.13 (stack and time checks) the shared programs are tested to behave identically instead, and to report the same error codes.
3. **Stage 2** (done): stage 1 compiling its own source produces exactly the Rust that stage 0 produces for it, so a stage 2 binary is identical in behavior to stage 1. `build.sh` checks this fixpoint on every build.
4. **From now on** language changes are made in stage 1. Stage 0 stays frozen at draft 0.3 plus `process` and `file_exists`. Draft 0.4's `ui` effect was the first feature that exists only in stage 1.
5. **Bootstrap chain.** When stage 1 itself started using modules (draft 0.5), stage 0 could no longer build it, so a module-free copy of stage 1 bridged the gap; in draft 0.6 that copy was generated from `stage1/`. Once stage 1's own source used `Option` and `with`, no copy that stage 0 can build could build stage 1 any more without a growing chain of snapshots. Since draft 0.7 the bootstrap is therefore a **seed**: `bootstrap/tessera1.rs`, the Rust code stage 1 generates for itself. `rustc` compiles the seed into a compiler that builds `stage1/`, and `build.sh` checks that stage 1 then rebuilds itself to a fixpoint. The first seed was produced through the full chain from stage 0 (stage 0, then the flat copy, then stage 1) and was byte-identical to stage 1's own output. To let stage 1 use a new feature: implement it without using it, regenerate the seed, then use it (see `bootstrap/README.md`). Stage 0 remains the independent reference implementation for the parity tests.

## 18. Deferred, in expected order

1. The rest of the formatter (section 14) and typed IR export with notes.
2. User-defined generics (typed JSON decoding, generic containers).
3. Closures, nested patterns, `Set`, constants.
4. TLS and a native SQLite or PostgreSQL client, which need a decision on runtime dependencies.
5. Notes on statements.

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
