# Tessera language reference

Tessera is a statically typed, expression-oriented language. Programs compile to safe native code. This reference is complete for single-file command-line programs.

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

- A program is one file with exactly one `fn main() -> Unit`. It may not take parameters.
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
- Keywords: `fn type enum let var set if else match for in while return break continue note uses and or not true false import pub with try`.
- Built-in function names cannot be used for your own functions.

## Types

| Type | Values |
|---|---|
| `Int` | 64-bit signed integer. Overflow is a runtime error. |
| `Bool` | `true`, `false` |
| `Text` | Immutable UTF-8 text, literal `"..."` with escapes `\n \t \\ \" \u{263A}` and interpolation `\(expression)` |
| `Unit` | `()`, the result of statements |
| `List[T]` | Immutable list, literal `[1, 2, 3]`; `[]` needs a known type |
| `Option[T]` | `.Some(value)` or `.None` |
| `Result[T, E]` | `.Ok(value)` or `.Error(error)` |

There are no implicit conversions, and conditions must be `Bool`.

Inside a text literal, `\(expression)` inserts the value of an `Int`, `Bool` or `Text` expression: `"found \(count) items in \(name)"`. The expression may contain calls and text literals: `"\(join(parts, ", "))"`. Braces `{}` in text have no special meaning. Prefer interpolation over `+` with `to_text`.

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
| `io` | `print`, `print_error` |
| `env` | `args`, `exit`, `environment_variable` |
| `fs` | `read_file`, `write_file`, `file_exists` |

## Bindings

```tessera
let total = 10                 // immutable
let names: List[Text] = []     // annotation where the type is not obvious
var count = 0                  // mutable
set count = count * 2          // every change uses `set`
set count += 1                 // short for `set count = count + 1`; also `-=`
let _ = compute()              // discard a value explicitly
```

- A `var` that is never `set` is an error. Parameters cannot be changed.
- `set x += e` works where `x + e` works (`Int`, `Text`), `set x -= e` for `Int`.
- No shadowing: a name cannot be declared again while it is in scope (including in nested blocks).
- An expression statement must not produce a value: use it, or `let _ = ...`.

## Operators

Precedence from high to low: call `f(x)`, index `xs[i]`, field `r.f`, `with`; unary `-x`, `not x`, `try x`; `* / %`; `+ -`; comparisons `== != < <= > >=`; `and`; `or`.

- `+` adds `Int` or concatenates `Text`. `< <= > >=` work on `Int` only. `==` compares any two values of the same type by value.
- Comparisons do not chain (`a < b < c` is an error). `and` and `or` cannot be mixed without parentheses. `not a == b` is an error: write `not (a == b)`.
- `/` truncates toward zero; division by zero is a runtime error. Indexing out of bounds is a runtime error.

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

- `if` without `else` must not produce a value. `for` iterates over a list. `return`, `break`, `continue` work as usual.

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

- Record literals give every field. Records are immutable; `with` makes a changed copy.
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
- `match` works on enums, `Option`, `Result`, `Int`, `Text` and `Bool`. One arm per line: `pattern => expression`, or one statement such as `pattern => set total += 1`. Use a block `{ ... }` for several statements. Statement arms make the match `Unit`.
- Patterns: `_`, a literal (`0`, `-1`, `"text"`, `true`), or `.Variant(binding, _)`. Patterns do not nest.
- A match must be exhaustive: every enum variant (or a final `_`); `Int` and `Text` need `_`.
- Enums may be recursive (`Node(left: Tree, right: Tree)`).

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
| `to_text` | `(value: Int or Bool) -> Text` |
| `parse_int` | `(text: Text) -> Option[Int]` (optional `-`/`+`, then digits; anything else, spaces or overflow give `.None`) |
| `length` | `(value: List[T] or Text) -> Int` (Text: number of characters) |
| `append` | `(list: List[T], item: T) -> List[T]` |
| `concat` | `(first: List[T], second: List[T]) -> List[T]` |
| `slice` | `(list: List[T], start: Int, end: Int) -> List[T]` (end exclusive) |
| `replace_at` | `(list: List[T], index: Int, item: T) -> List[T]` |
| `sort` | `(list: List[Int] or List[Text]) -> same type` (ascending; Text by code points, so `"Z"` before `"a"`) |
| `index_of` | `(list: List[T], item: T) -> Option[Int]`, or `(text: Text, part: Text) -> Option[Int]` (first position; Text: in characters) |
| `contains` | `(list: List[T], item: T) -> Bool`, or `(text: Text, part: Text) -> Bool` |
| `range` | `(start: Int, end: Int) -> List[Int]` (end exclusive) |
| `split` | `(text: Text, separator: Text) -> List[Text]` (keeps empty parts: `split("a,,b", ",")` is `["a", "", "b"]`; separator `""` gives the characters) |
| `join` | `(parts: List[Text], separator: Text) -> Text` |
| `code_points` | `(text: Text) -> List[Int]` (Unicode code points) |
| `from_code_point` | `(code: Int) -> Text` |
| `from_code_points` | `(codes: List[Int]) -> Text` |
| `value_or` | `(option: Option[T], default: T) -> T` |
| `args` | `() -> List[Text] uses env` (command-line arguments, without the program name) |
| `exit` | `(code: Int) -> Never uses env` (exit code 0 to 255) |
| `fail` | `(message: Text) -> Never` (stops with a runtime error; for impossible states) |

There is no string indexing and no map type. Characters are handled through `split(text, "")` or `code_points` (`"a"` is 97, `"0"` is 48, space is 32, newline is 10) and rebuilt with `join` or `from_code_point(s)`.

Building a list or text in a loop with `set xs = append(xs, item)` or `set text += piece` is efficient.

## Diagnostics

The compiler reports errors as `error[CODE]: message` with a location and usually a `help:` line. Common codes: E0202 unknown name, E0203 unknown function, E0204 name already defined (no shadowing), E0206 `var` never changed, E0211 built-in function redefined, E0301 missing effect, E0302 unused effect, E0501 type mismatch, E0505 type of `[]`/`.None`/`Result` unknown, E0508 unused value, E0509 match not exhaustive, E0518 variant type not known.
