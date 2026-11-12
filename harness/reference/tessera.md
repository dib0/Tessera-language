# Tessera language reference

Tessera is a statically typed, expression-oriented language. Programs compile to safe native code. This reference describes language draft 0.20 and is complete for command-line programs, programs split into modules, web applications and graphical programs. The authoritative specification is `language-design.md`.

## Program structure

```tessera
note "Optional design note; must be directly above the declaration it describes."
fn main() -> Unit uses io, env {
let names = args()
for name in names {
print("Hello, \(name)")
}
}
```

- Source files end in `.tes`. A program's entry file has exactly one `fn main() -> Unit` (no parameters), or `handle` for a web application (see `std/http`). Other files are modules (see Modules).
- Declarations: `fn`, `type` (record), `enum`. Any order. No global variables.
- Comments start with `//`. There are no block comments.
- Statements end at the end of a line. No semicolons. A line continues only if it ends with an operator, `=`, `,`, `(`, `[` or `{`, or inside open `(` or `[`.
- Blocks use braces. `else` must be on the same line as the `}` before it: `} else {`.
- Indentation has no meaning. Tabs are not allowed.

## Names

- Functions, parameters, variables, fields: `snake_case` (enforced).
- Types and enum variants: `PascalCase` (enforced).
- ASCII letters, digits and `_` only. Names start with a letter.
- `_` alone marks a binding that is not used: `let _ = f()`, `for _ in range(0, n)`, `.Variant(_, y)`. It cannot be read; `_x` is not a valid name.
- Keywords: `fn type enum let var set if else match for in while return break continue note uses and or not true false import pub with try`. Reserved: `module unsafe loop const trait impl`. A keyword is never a name: `let note = 1` is an error.
- Built-in function names cannot be used for your own functions, also not in modules (E0211): name it `fixed_to_int`, not `to_int`.

## Types

| Type | Values |
|---|---|
| `Int` | 64-bit signed integer. Overflow is a runtime error. |
| `Bool` | `true`, `false` |
| `Text` | Immutable UTF-8 text, literal `"..."` with escapes `\n \t \\ \" \u{263A}` and interpolation `\(expression)` |
| `Unit` | `()`, the result of statements |
| `List[T]` | List, literal `[1, 2, 3]`; `[]` needs a known type. A value: elements change only through `set xs[i] = v` on a `var` |
| `Option[T]` | `.Some(value)` or `.None` |
| `Result[T, E]` | `.Ok(value)` or `.Error(error)` |
| `Map[K, V]` | Immutable map sorted by key (`K` is `Int` or `Text`), literal `["a": 1, "b": 2]`; `[:]` is empty and needs a known type |
| `Decimal` | Exact number with 9 decimal places, literal `19.99`; overflow is a runtime error |
| `Float` | 64-bit floating point, always finite (overflow, division by zero and `sqrt(-1.0)` are runtime errors); literal `0.5` where a `Float` is expected, or with an exponent: `1e-9` |
| `Bytes` | Bytes 0 to 255, from `to_bytes(text)` or `bytes([1, 2])`; `b[i]` is an `Int`, `length(b)` |
| `fn(A, B) -> C` | A function value, e.g. `fn(Int) -> Text`; with effects `fn(Text) -> Unit uses io` |

There are no implicit conversions, and conditions must be `Bool`. `Decimal`, `Float` and `Int` do not mix: `price * decimal(quantity)`, `speed * float(count)`, `to_int(x)`.

A number like `0.5` is a `Float` where a `Float` is expected (annotation, parameter, field, return value, the other side of an operator with a `Float`, a list with a `Float`) and a `Decimal` everywhere else. So `var total = 0.0` is a `Decimal`: write `var total: Float = 0.0`. Use `Decimal` for money, `Float` for measurements and geometry.

Inside a text literal, `\(expression)` inserts the value of an `Int`, `Bool`, `Text`, `Decimal` or `Float` expression: `"found \(count) items in \(name)"`. The expression may contain calls and text literals: `"\(join(parts, ", "))"`. Braces `{}` in text have no special meaning. Prefer interpolation over `+` with `to_text`.

## Functions and effects

```tessera
fn square(x: Int) -> Int = x * x

fn describe(x: Int) -> Text {
let squared = square(x)
"\(x) squared is \(squared)"
}
```

- Parameter and return types are always written. The last expression of a block is its value.
- No overloading, no default arguments, no methods: call `length(xs)`, not `xs.length()`.
- A function that uses an effect must declare it with `uses`; a function may only call functions whose effects it declares. Declaring an effect that is not used is an error (except in `main`).

| Effect | Allows |
|---|---|
| `io` | `print`, `print_error`, `read_line`, `read_input`, `exit_with` |
| `env` | `args`, `exit`, `exit_with`, `environment_variable` |
| `fs` | `read_file`, `write_file`, `read_text`, `write_text`, `read_bytes`, `write_bytes`, `file_exists`, `list_directory`, `create_directory`, `remove_file` |
| `process` | `run_process` |
| `time` | `now`, `monotonic`, `sleep` |
| `random` | `random_int`, `random_bytes`, `password_hash` |
| `log` | `log` |
| `db` | `db_open`, `db_query`, `db_execute` (SQLite) |
| `net` | `net_request` and the `std/http` client |
| `ui` | one window: drawing and events (see Windows) |
| `audio` | `play_sound` |

Functions without `uses` are pure. A function value with effects is called like any function; the caller declares its effects.

## Bindings

```tessera
let total = 10                 // immutable
let names: List[Text] = []     // annotation where the type is not obvious
var count = 0                  // mutable
set count = count * 2          // every change uses `set`
set count += 1                 // short for `set count = count + 1`; also `-=`
set p.x = 5                    // a field of a `var` record
set grid[y][x] += 1            // an element of a `var` list (also `Bytes`)
let _ = compute()              // discard a value explicitly
let n = parse_int(text) else exit_with(1, "error: not a number")  // the value of an Option/Result, or stop
```

- A `var` that is never `set` is an error. Parameters, `let` bindings and loop variables cannot be changed, also not in part.
- `set` on a field or element changes only that `var`: `let b = a` before `set a[0] = 9` keeps `b` unchanged. The change is in place, so filling a buffer made with `repeat([0], n)` is fast. Indexes out of bounds are a runtime error.
- `set x += e` works where `x + e` works (`Int`, `Text`, lists: `set xs += [item]`), `set x -= e` for `Int`. There is no `*=` or `/=`: write `set x = x * 2`.
- No shadowing: a name cannot be declared again while it is in scope (including in nested blocks).
- An expression statement must not produce a value: use it, or `let _ = ...`.
- `let x = value else stop`: `value` is an `Option` or `Result`; `x` is the value inside it. The `else` part must stop: `return ...`, `exit_with(...)`, `fail(...)`, `break` or `continue`.

## Operators

Precedence from high to low: call `f(x)`, index `xs[i]`, field `r.f`, `with`; unary `-x`, `not x`, `try x`; `* / %`; `+ -`; comparisons `== != < <= > >=`; `and`; `or`.

- `+ - * /` work on `Int`, `Decimal` and `Float` (never mixed); `%` on `Int` only. `+` also joins two `Text` values, two `Bytes`, or two lists of the same type (`[1, 2] + [3]`). `< <= > >=` work on `Int`, `Decimal` and `Float`. `==` compares any two values of the same type by value (lists element by element, records field by field).
- Comparisons do not chain (`a < b < c` is an error). `and` and `or` cannot be mixed without parentheses. `not a == b` is an error: write `not (a == b)`.
- Integer `/` truncates toward zero; `%` has the sign of the left operand. Division by zero and overflow are runtime errors, as is indexing out of bounds.

## Control flow

```tessera
let kind = if count == 0 { "none" } else if count == 1 { "one" } else { "many" }

for item in items {
if item < 0 {
continue
}
print("\(item)")
}

var i = 0
while i < 10 {
set i += 1
}

for index in range(0, length(items)) {
print("\(index): \(items[index])")
}
```

- `if` without `else` must not produce a value. `for` iterates over a list (evaluated once); `range(a, b)` is empty when `a >= b`. `return`, `break`, `continue` work as usual and fit anywhere a value is expected: `let x = if ok { 1 } else { return 0 }`.

## Records

```tessera
type Point {
x: Int
y: Int
}

let p = Point { x: 1, y: 2 }
let moved = p with { x: p.x + 10 }   // copy with some fields changed
print("\(moved.x)")
```

- Record literals give every field. `with` makes a changed copy; on a `var`, `set p.x = 5` changes one field.
- In `if`, `while`, `for` and `match` headers, wrap a record literal in parentheses.

## Enums and match

```tessera
enum Shape {
Circle(radius: Int)
Square(side: Int)
Empty
}

fn area(shape: Shape) -> Int {
match shape {
.Circle(radius) => 3 * radius * radius
.Square(side) => side * side
.Empty => 0
}
}

fn total_area(shapes: List[Shape]) -> Int {
var total = 0
for shape in shapes {
match shape {
.Empty => continue
_ => set total += area(shape)
}
}
total
}

let first: Shape = .Square(2)
let shapes = [Shape.Circle(1), .Empty]
```

- Where the type is known, write a variant without its type: `.Circle(2)`, `.Some(x)`, `.Ok(v)`. The type is known in patterns, function arguments, return values, annotated bindings, record fields, list elements after the first, and on the right of `==`/`!=`. Otherwise write `Shape.Circle(2)` (`let s = .Empty` is error E0518).
- `match` works on enums, `Option`, `Result`, `Int`, `Text` and `Bool`. One arm per line: `pattern => expression`, or one `set`, `let`, `for` or `while` statement such as `pattern => set total += 1`. Use a block `{ ... }` for several statements. Statement arms make the match `Unit`.
- Patterns: `_`, a literal (`0`, `-1`, `"text"`, `true`), or `.Variant(binding, _)`. Patterns do not nest: `.Error(.NotFound)` is not a pattern; match `.Error(e)` and compare `e == .NotFound`.
- A match must be exhaustive: every enum variant (or a final `_`); `Int` and `Text` need `_`.
- Enums may be recursive (`Node(left: Tree, right: Tree)`).

## Maps and functions as values

```tessera
fn word_counts(words: List[Text]) -> Map[Text, Int] {
var counts: Map[Text, Int] = [:]
for word in words {
set counts = put(counts, word, value_or(get(counts, word), 0) + 1)
}
counts
}

type Item {
name: Text
price: Int
}

fn price_of(item: Item) -> Int = item.price
fn is_cheap(item: Item) -> Bool = item.price < 100

fn cheapest_names(items: List[Item]) -> List[Text] = map(sort_by(filter(items, is_cheap), price_of), name_of)
fn name_of(item: Item) -> Text = item.name
```

- `get(map, key)` gives an `Option`; `put` and `remove` give a changed map; `keys` and `values` are in key order; `contains(map, key)` and `length(map)` work too. `set m = put(m, k, v)` updates in place.
- A function name without `(...)` is a function value; so are built-ins such as `to_text` and `trim`: `join(map(numbers, to_text), " ")`. A parameter `f: fn(Int) -> Int` is called as `f(x)`.

## Option, Result and try

```tessera
enum AgeError {
NotANumber(text: Text)
OutOfRange(age: Int)
}

fn parse_age(text: Text) -> Result[Int, AgeError] {
let age = match parse_int(text) {
.Some(n) => n
.None => return .Error(.NotANumber(text))
}
if age < 0 or age > 150 {
return .Error(.OutOfRange(age))
}
.Ok(age)
}

fn total_age(texts: List[Text]) -> Result[Int, AgeError] {
var total = 0
for text in texts {
set total += try parse_age(text)
}
.Ok(total)
}

fn first_long(words: List[Text]) -> Option[Text] {
for word in words {
if length(word) > 5 {
return .Some(word)
}
}
.None
}
```

- `.None`, `.Ok(x)` and `.Error(e)` need their full type from context (return type, parameter, or annotation): `let r: Result[Int, Text] = .Ok(1)`.
- `try value` unwraps `.Ok`/`.Some`, or returns the error/`.None` from the current function, which must return a matching `Result`/`Option`.
- `value_or(option, default)` returns the value inside an `Option` or the default: `value_or(parse_int(text), 0)`.

## Built-in functions

| Function | Signature |
|---|---|
| `print`, `print_error` | `(value: Text) -> Unit uses io` (adds a newline) |
| `to_text` | `(value: Int, Bool, Decimal or Float) -> Text` (`Float` always shows a `.` or exponent: `1.0`) |
| `parse_int` | `(text: Text) -> Option[Int]` (optional `-`, then digits; anything else, including `+`, spaces or overflow, gives `.None`) |
| `length` | `(value: List[T], Text, Map or Bytes) -> Int` (Text: number of characters) |
| `slice` | `(list: List[T] or Bytes, start: Int, end: Int) -> same type` (end exclusive) |
| `replace_at` | `(list: List[T], index: Int, item: T) -> List[T]` |
| `sort` | `(list: List[Int] or List[Text]) -> same type` (ascending; Text by code points, so `"Z"` before `"a"`) |
| `index_of` | `(list: List[T], item: T) -> Option[Int]`, or `(text: Text, part: Text) -> Option[Int]` (first position; Text: in characters) |
| `contains` | `(list: List[T], item: T) -> Bool`, or `(text: Text, part: Text) -> Bool` |
| `range` | `(start: Int, end: Int) -> List[Int]` (end exclusive) |
| `split` | `(text: Text, separator: Text) -> List[Text]` (keeps empty parts: `split("a,,b", ",")` is `["a", "", "b"]`; separator `""` gives the characters) |
| `join` | `(parts: List[Text], separator: Text) -> Text` |
| `to_lower`, `to_upper`, `trim` | `(text: Text) -> Text` (Unicode rules; `trim` removes outer whitespace) |
| `starts_with`, `ends_with` | `(text: Text, part: Text) -> Bool` |
| `replace` | `(text: Text, old: Text, new: Text) -> Text` (every occurrence) |
| `slice_text` | `(text: Text, start: Int, end: Int) -> Text` (characters, end exclusive) |
| `repeat` | `(text: Text, times: Int) -> Text`, or `(list: List[T], times: Int) -> List[T]` (`repeat([0], 3)` is `[0, 0, 0]`) |
| `pad_left`, `pad_right` | `(text: Text, width: Int, fill: Text) -> Text` (`pad_left("7", 3, "0")` is `"007"`) |
| `lines` | `(text: Text) -> List[Text]` |
| `map`, `filter`, `sort_by` | `(list: List[T], f: fn(T) -> U) -> List[U]`; `f: fn(T) -> Bool`; key `fn(T) -> Int/Text/Decimal` (stable) |
| `get`, `put`, `remove`, `keys`, `values` | maps, see above |
| `float`, `floor`, `ceil`, `sqrt`, `exp`, `ln`, `sin`, `cos`, `tan`, `pi()` | `Int -> Float`; the rest `Float -> Float` (`floor`/`ceil` stay `Float`; `to_int` truncates) |
| `atan2`, `pow` | `(y: Float, x: Float) -> Float`; `(base: Float, exponent: Float) -> Float` |
| `parse_float` | `(text: Text) -> Option[Float]` |
| `decimal`, `to_int`, `round`, `parse_decimal` | `Int -> Decimal`; `Decimal -> Int` (toward zero; also `Float -> Int`); `(Decimal, places: Int) -> Decimal` (also for `Float`); `Text -> Option[Decimal]` |
| `read_line`, `read_input` | `() -> Option[Text] uses io` (`.None` at end of input); `() -> Text uses io` (all of standard input) |
| `read_text`, `write_text` | `(path: Text) -> Result[Text, FileError] uses fs`; `(path: Text, content: Text) -> Result[Unit, FileError] uses fs` (`FileError` is `.NotFound`, `.PermissionDenied` or `.Other(message)`) |
| `list_directory`, `create_directory`, `remove_file` | `(path: Text) -> Result[List[Text], FileError]` (sorted), `-> Result[Unit, FileError]`; all `uses fs` |
| `read_file`, `write_file`, `file_exists` | `(path: Text) -> Text`; `(path: Text, content: Text) -> Unit`; `(path: Text) -> Bool`; all `uses fs` (`read_file`/`write_file` stop with a runtime error on failure; prefer `read_text`/`write_text`) |
| `read_bytes`, `write_bytes` | `(path: Text) -> Result[Bytes, FileError] uses fs`; `(path: Text, data: Bytes) -> Result[Unit, FileError] uses fs` (binary files) |
| `now`, `monotonic`, `sleep` | `() -> Int uses time` (milliseconds since 1970 UTC); `() -> Int uses time` (milliseconds, for measuring); `(ms: Int) -> Unit uses time` |
| `format_time`, `parse_time`, `http_date` | `(ms: Int) -> Text` (`2026-11-08T14:52:38.123Z`); `(text: Text) -> Option[Int]` (RFC 3339); `(ms: Int) -> Text` (`Sun, 08 Nov 2026 14:52:38 GMT`) |
| `random_int`, `random_bytes` | `(low: Int, high: Int) -> Int uses random` (high exclusive); `(n: Int) -> Bytes uses random` (secure, not repeatable; use `std/rng` for repeatable numbers) |
| `environment_variable` | `(name: Text) -> Text uses env` (`""` when not set) |
| `run_process` | `(program: Text, arguments: List[Text]) -> Int uses process` (waits; exit code, `-1` if it could not start) |
| `log` | `(level: Text, message: Text) -> Unit uses log` (standard error / server error log) |
| `code_points` | `(text: Text) -> List[Int]` (Unicode code points) |
| `from_code_point` | `(code: Int) -> Text` |
| `from_code_points` | `(codes: List[Int]) -> Text` |
| `value_or` | `(option: Option[T], default: T) -> T` |
| `args` | `() -> List[Text] uses env` (command-line arguments, without the program name) |
| `exit` | `(code: Int) -> Never uses env` (exit code 0 to 255) |
| `exit_with` | `(code: Int, message: Text) -> Never uses io, env` (prints `message` and a newline to standard error, then exits) |
| `fail` | `(message: Text) -> Never` (stops with a runtime error; for impossible states) |
| `bit_and`, `bit_or`, `bit_xor` | `(a: Int, b: Int) -> Int` (64-bit two's complement; `bit_and(-1, 255)` is `255`) |
| `shift_left`, `shift_right` | `(value: Int, amount: Int) -> Int` (amount 0 to 63; `shift_left` is a runtime error when bits would be lost, so sign-extend with `if v >= 32768 { v - 65536 } else { v }`; `shift_right` is arithmetic: `shift_right(-5, 1)` is `-3`) |
| `bytes`, `byte_values` | `(values: List[Int]) -> Bytes`; `(b: Bytes) -> List[Int]` |
| `to_bytes`, `from_bytes` | `(text: Text) -> Bytes` (UTF-8); `(b: Bytes) -> Option[Text]` |
| `sha256`, `hmac_sha256`, `equal_constant_time` | `(data: Bytes) -> Bytes`; `(key: Bytes, message: Bytes) -> Bytes`; `(a: Bytes, b: Bytes) -> Bool` |
| `base64_encode`, `hex_encode` / `base64_decode`, `hex_decode` | `(b: Bytes) -> Text` / `(text: Text) -> Option[Bytes]` |
| `password_hash`, `password_verify` | `(password: Text) -> Text uses random` (PBKDF2); `(password: Text, hash: Text) -> Bool` |
| `db_open`, `db_query`, `db_execute` | `(path: Text) -> Result[Database, Text]`; `(db: Database, sql: Text, params: List[Text]) -> Result[List[Map[Text, Text]], Text]`; same arguments `-> Result[Int, Text]` (changed rows); all `uses db`. One statement per call, parameters `?1`, `?2`, never pasted into the SQL; NULL columns are absent from the row |
| `rgb` | `(red: Int, green: Int, blue: Int) -> Int` (a color, each part 0 to 255) |
| `assert` | `(condition: Bool) -> Unit` (in tests) |

There is no string indexing. Characters are handled through `split(text, "")` or `code_points` (`"a"` is 97, `"0"` is 48, space is 32, newline is 10) and rebuilt with `join` or `from_code_point(s)`.

Building a list, text or map in a loop with `set xs += [item]`, `set text += piece` or `set m = put(m, k, v)` is efficient.

## Performance

Values never share state, but the compiler avoids copies:

- `set x = f(x, ...)` with `x` used once on the right updates `x` in place (lists, text, maps, `Bytes`, records, enums). So does `set c = if a { f(c) } else { g(c) }`, and a variable used once in a `return` value or a function's final expression.
- `set xs[i] = v` and `set r.field = v` take constant time.
- A function that changes a list or record it receives works in place when its body starts with `var out = param` (not inside a loop or branch) and `param` is used nowhere else; call it as `set grid = plot(grid, i)`.
- Anything else copies: `let y = f(x)` keeps `x` alive, so a large buffer passed that way is copied. Thread large state through `set x = f(x)`.

## Modules

```tessera
// geometry/shapes.tes
pub type Point {
x: Int
y: Int
}

pub fn shifted(p: Point, dx: Int) -> Point = p with { x: p.x + dx }
```

```tessera
// main.tes
import geometry/shapes uses nothing

fn main() -> Unit uses io {
let p = shapes.shifted(shapes.Point { x: 1, y: 2 }, 3)
print("\(p.x)")
}
```

- Every file except the entry file is a module, named by its path relative to the entry file, without `.tes`. Only the entry file has `main`.
- Declarations are private unless marked `pub`. A public function or type may not mention a private type.
- `import a/b` makes the module available as `b`; every use is qualified: `b.f(x)`, `b.Type`, `b.Type.Variant(1)`, also in types and patterns. There are no aliases, so a local name cannot be the same as an imported module's name (`let keys = ...` next to `import std/keys` is E0204).
- Imports come first, every import must be used, and modules cannot import each other in a cycle.
- `import a/b uses nothing` (or `uses fs`, ...) promises that the module's public functions use no other effects; the compiler checks it.
- Only the entry file is given to the compiler; imported modules are found automatically.

Error messages belong on standard error: `exit_with(1, "error: \(reason)")` reports and stops in one call, and its type `Never` fits anywhere, for example as a `match` arm that has no value to produce.

## Standard modules

`import std/json` (also `std/url`, `std/html`, `std/http`) and use names through the last path segment: `json.parse(text)`.

- `std/json`: `enum Json { Null, Bool(value), Number(value: Decimal), String(value), Array(items), Object(fields: Map[Text, Json]) }`; `parse(text) -> Result[Json, Text]`, `encode(json) -> Text`, `field(json, name) -> Option[Json]`, `as_text`, `as_int`, `as_decimal`, `as_bool` (each `-> Option[...]`), `as_list(json) -> List[Json]`, `text_field(json, name)`, `int_field(json, name)`.
- `std/url`: `percent_encode`, `percent_decode -> Option[Text]`, `parse_query(text) -> Map[Text, Text]`, `path_segments(path) -> List[Text]`.
- `std/html`: `escape(text)`; use it for every user value placed in HTML.
- `std/url`: also `parse_query_all(text) -> Map[Text, List[Text]]`, `encode_query(map) -> Text`.
- `std/bytes`: `read_u16_le`, `read_i16_le`, `read_u32_le`, `read_i32_le` (`(data: Bytes, at: Int) -> Int`), `read_ascii(data, at, count) -> Text` (stops at a zero byte).
- `std/math`: `min`, `max`, `clamp(value, low, high)`, `abs`, `isqrt` (integer square root).
- `std/fixed`: 16.16 fixed point: `one()`, `from_int`, `floor_int`, `mul`, `div`, `abs`, `sine(index, count)`, `cosine(index, count)` (angle `2π · index / count`), `sine_table(count)`, `cosine_table(count)`.
- `std/palette`: `read(data: Bytes, at: Int) -> List[Int]` (256 colors from 768 RGB bytes, packed like `rgb`), `red`, `green`, `blue`.
- `std/ppm`: `encode(width, height, pixels: List[Int], palette: List[Int]) -> Bytes` (P6 image of palette indexes; write it with `write_bytes`).
- `std/raster`: `Image { width, height, pixels }` with `blank`, `from_pixels`, `fill`, `line`, `blit(image, source, x, y, transparent)`, `stretch(image, source, x, y, w, h, transparent)`, `remap(image, table)`, `Font` with `text` and `measure` (use as `set image = raster.fill(image, ...)`).
- `std/sound`: `Sound { samples, rate }`, `from_wav(bytes)`, `tone(hz, ms, loudness)`, `position(dx, dy, right_x, right_y, range) -> [volume, pan]`, `play(sound, volume, pan)` (`uses audio`; the built-in is `play_sound(samples, rate, volume, pan)`).
- `std/keys`: `Keys`, `none()`, `update(keys, event)`, `held(keys, name)`, `any_held(keys, names)`, `typed(keys, word)`, `forget_typed`, `release_all` (names compare in lower case).
- `std/plane`: fixed-point 2D geometry: `intersect`, `crosses`, `orient`, `circle_touches`, `direction`.
- `std/rng`: repeatable random numbers: `next(seed)`, `byte(seed)` and `roll(seed, low, high)`; the last two return `[value, next seed]`.
- `std/http`: a web application has `fn handle(request: http.Request) -> http.Response` instead of `main` (no `io`). `Request` has `method` (`"GET"`), `path` (decoded), `query: Map[Text, Text]`, `headers: Map[Text, Text]` (lowercase names), `body: Bytes`. Responses: `http.text(status, body)`, `http.html(status, body)`, `http.json(status, body)`, `http.redirect(location)`, `http.not_found()`, `http.bytes_response(status, content_type, body)`; `http.match_path("/items/:id", request.path) -> Option[Map[Text, Text]]`, `http.form(request)`, `http.cookie(request, name)`, `http.with_header(response, name, value)`, `http.with_cookie(response, name, value, max_age)`, `http.header(request, name) -> Option[Text]`, `http.text_body(request)`, `http.cookies(request)`, `http.sign(secret, value)`/`http.verify(secret, signed) -> Option[Text]`, `http.csrf_token(secret, session_id)`/`http.check_csrf(secret, session_id, token)`. Client (`uses net`, plain HTTP): `http.get_text(url) -> Result[Text, Text]`, `http.fetch(method, url, headers, body)`. In tests: `http.request(method, target, body)` and `http.response_text(response)`.
- A web application is built with `tessera1 build app.tes -o app` and runs as CGI, as a server (`app serve --port 8080`), FastCGI (`app fastcgi --port N`) or once (`app request GET /path`); `--library` builds a module for nginx, Apache or IIS. A runtime error in a request gives that request a 500.

Tests: `test "name" { assert(condition) }` at the top level of a file, run with `tessera1 test file.tes` (each test on its own thread; every effect except `ui`). `build` ignores tests.

Windows (`uses ui`, one window, X11): `open_window(title, width, height)`, `fill_rectangle(x, y, w, h, color)`, `draw_line(x1, y1, x2, y2, thickness, color)`, `draw_circle(x, y, radius, thickness, color)`, `fill_circle(x, y, radius, color)`, `draw_text(x, y, text, color)`, `text_width(text)`, `draw_pixels(x, y, width, colors, scale)` (a whole image of `rgb` colors), `present()`; `next_event()` waits, `poll_event()` returns `Option[WindowEvent]` at once. `WindowEvent` is `.Click(x, y, button)`, `.Key(name)`, `.KeyUp(name)` or `.Close`. Key names: `"a"`, `"A"`, `"7"`, `" "`, `"Escape"`, `"Enter"`, `"Backspace"`, `"Tab"`, `"Left"`, `"Up"`, `"Right"`, `"Down"`, `"Shift"`, `"Control"`, `"Alt"`, `"F1"` to `"F12"`, `"Delete"`, `"Home"`, `"End"`, `"PageUp"`, `"PageDown"`; `KeyUp` uses the lowercase letter. Drawing is off-screen until `present()`.

Sound (`uses audio`): `play_sound(samples: Bytes, rate: Int, volume: Int, pan: Int)` starts a sound and returns at once: 8-bit unsigned mono samples (128 is silence), rate 1,000 to 96,000, volume 0 to 100, pan -100 (left) to 100 (right); up to 16 at once. Without an audio player (`aplay` or `pw-cat`) nothing is heard and the program continues.

## Tools

```
tessera1 check main.tes [--json]
tessera1 build main.tes -o program [--opt 3] [--library]
tessera1 test main.tes
tessera1 format files... [--check]
```

`build` keeps the generated Rust as `program.rs`. Compiler exit codes: 0 ok, 1 compile errors, 2 usage error.

Runtime errors (`R001` index out of bounds, `R002` overflow, `R003` division by zero, `R004` argument out of range, `R010` `fail`, `R019` `assert`, ...) cannot be caught: the program prints `runtime error R001: ...` with the location to standard error and exits with code 70. Expected failures are `Result` values.

## Diagnostics

The compiler reports errors as `error[CODE]: message` with a location and usually a `help:` line. Common codes: E0110 syntax error, E0111 `else` not on the line of `}`, E0202 unknown name, E0203 unknown function, E0204 name already defined (no shadowing), E0205 `set` on a non-`var`, E0206 `var` never changed, E0207 naming style, E0211 built-in function redefined, E0301 missing effect, E0302 unused effect, E0501 type mismatch, E0503 operator on wrong types (e.g. `Int` with `Decimal`), E0505 type of `[]`/`.None`/`Result` unknown, E0506 branches differ, E0508 unused value, E0509 match not exhaustive, E0518 variant type not known, E0607 private declaration used, E0610 unused import.
