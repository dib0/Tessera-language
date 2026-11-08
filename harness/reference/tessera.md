# Tessera language reference

Tessera is a statically typed, expression-oriented language. Programs compile to safe native code. This reference is complete for single-file command-line programs.

## Program structure

```tessera
note "Optional design note; must be directly above the declaration it describes."
fn main() -> Unit uses io, env {
    let names = args()
    for name in names {
        print("Hello, " + name)
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
- `_` alone marks a binding that is not used: `let _ = f()`, `for _ in range(0, n)`, `Type.Variant(_, y)`. It cannot be read; `_x` is not a valid name.
- Keywords: `fn type enum let var set if else match for in while return break continue note uses and or not true false import pub with try`.

## Types

| Type | Values |
|---|---|
| `Int` | 64-bit signed integer. Overflow is a runtime error. |
| `Bool` | `true`, `false` |
| `Text` | Immutable UTF-8 text, literal `"..."` with escapes `\n \t \\ \" \u{263A}` |
| `Unit` | `()`, the result of statements |
| `List[T]` | Immutable list, literal `[1, 2, 3]`; `[]` needs a known type |
| `Option[T]` | `Option.Some(value)` or `Option.None` |
| `Result[T, E]` | `Result.Ok(value)` or `Result.Error(error)` |

There are no implicit conversions: `"n = " + to_text(n)`, and conditions must be `Bool`.

## Functions and effects

```tessera
fn square(x: Int) -> Int = x * x

fn describe(x: Int) -> Text {
    let squared = square(x)
    to_text(x) + " squared is " + to_text(squared)
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
set count = count + 1          // every change uses `set`
let _ = compute()              // discard a value explicitly
```

- A `var` that is never `set` is an error. Parameters cannot be changed.
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
    print(to_text(item))
}

var i = 0
while i < 10 {
    set i = i + 1
}

for index in range(0, length(items)) {
    print(to_text(index) + ": " + items[index])
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
print(to_text(moved.x))
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
        Shape.Circle(radius) => 3 * radius * radius
        Shape.Square(side) => side * side
        Shape.Empty => 0
    }
}

fn total_area(shapes: List[Shape]) -> Int {
    var total = 0
    for shape in shapes {
        match shape {
            Shape.Empty => continue
            _ => set total = total + area(shape)
        }
    }
    total
}
```

- Variants are always written with their type: `Shape.Circle(2)`, `Shape.Empty`.
- `match` works on enums, `Option`, `Result`, `Int`, `Text` and `Bool`. One arm per line: `pattern => expression`, or one statement such as `pattern => set total = total + 1`. Use a block `{ ... }` for several statements. Statement arms make the match `Unit`.
- Patterns: `_`, a literal (`0`, `-1`, `"text"`, `true`), or `Type.Variant(binding, _)`. Patterns do not nest.
- A match must be exhaustive: every enum variant (or a final `_`); `Int` and `Text` need `_`.
- Enums may be recursive (`Node(left: Tree, right: Tree)`).

## Option, Result and try

```tessera
fn find_index(items: List[Text], wanted: Text) -> Option[Int] {
    for index in range(0, length(items)) {
        if items[index] == wanted {
            return Option.Some(index)
        }
    }
    Option.None
}

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

fn sum(texts: List[Text]) -> Result[Int, ParseError] {
    var total = 0
    for text in texts {
        set total = total + try parse_number(text)
    }
    Result.Ok(total)
}
```

- `Option.None`, `Result.Ok(x)` and `Result.Error(e)` need their full type from context (return type, parameter, or annotation): `let r: Result[Int, Text] = Result.Ok(1)`.
- `try value` unwraps `Result.Ok`/`Option.Some`, or returns the error/`Option.None` from the current function, which must return a matching `Result`/`Option`.
- `value_or(option, default)` returns the value inside an `Option` or the default.

## Built-in functions

| Function | Signature |
|---|---|
| `print`, `print_error` | `(value: Text) -> Unit uses io` (adds a newline) |
| `to_text` | `(value: Int or Bool) -> Text` |
| `length` | `(value: List[T] or Text) -> Int` (Text: number of characters) |
| `append` | `(list: List[T], item: T) -> List[T]` |
| `concat` | `(first: List[T], second: List[T]) -> List[T]` |
| `slice` | `(list: List[T], start: Int, end: Int) -> List[T]` (end exclusive) |
| `replace_at` | `(list: List[T], index: Int, item: T) -> List[T]` |
| `range` | `(start: Int, end: Int) -> List[Int]` (end exclusive) |
| `code_points` | `(text: Text) -> List[Int]` (Unicode code points) |
| `from_code_point` | `(code: Int) -> Text` |
| `from_code_points` | `(codes: List[Int]) -> Text` |
| `join` | `(parts: List[Text], separator: Text) -> Text` |
| `value_or` | `(option: Option[T], default: T) -> T` |
| `args` | `() -> List[Text] uses env` (command-line arguments, without the program name) |
| `exit` | `(code: Int) -> Never uses env` (exit code 0 to 255) |
| `fail` | `(message: Text) -> Never` (stops with a runtime error; for impossible states) |

There is no built-in sorting, searching, splitting, string indexing, number parsing or map type: write small functions for these. Text is processed through `code_points` (`"a"` is 97, `"0"` is 48, space is 32, newline is 10) and rebuilt with `from_code_point(s)`.

Building a list or text in a loop with `set xs = append(xs, item)` or `set text = text + piece` is efficient.

## Diagnostics

The compiler reports errors as `error[CODE]: message` with a location and usually a `help:` line. Common codes: E0202 unknown name, E0203 unknown function, E0204 name already defined (no shadowing), E0206 `var` never changed, E0301 missing effect, E0302 unused effect, E0501 type mismatch, E0505 type of `[]`/`Option.None`/`Result` unknown, E0508 unused value, E0509 match not exhaustive.
