# Tessera Language Design

**Status:** Draft 0.2 (POC). Builds on `poc-language-design.md`.
**Scope:** The POC subset only. Anything not described here is not part of the language yet.

This document is the authoritative description of Tessera for the POC. When the compiler and this document disagree, the document wins and the compiler has a bug, or the document is updated through an explicit decision (see section 18).

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

## 3. Changes from the POC sketch

The sketch in `poc-language-design.md` was explicitly illustrative. This design changes it as follows:

| Sketch | This design | Reason |
|---|---|---|
| `i64`, `bool`, `str`, `unit` | `Int`, `Bool`, `Text`, `Unit` | Full words, consistent capitalized type names (principle 6). `Int` is still 64-bit. |
| `c ? a : b` | `if c { a } else { b }` | One conditional form instead of two (principle 3). |
| `!{io}` | `uses io` | Readable without knowing the notation. |
| `age:i64` | `age: Int` | Canonical spacing improves diff readability. Token cost to be measured, not assumed. |
| `adult` | `is_adult` | Convention, not a rule: boolean functions read best as questions. |

## 4. Source files and lexical structure

### 4.1 Files

- Source files use the extension `.tes` and must be valid UTF-8.
- A POC program is a single file. Modules and imports are deferred.
- The compiler rejects files containing a byte order mark, tabs used for indentation, or carriage returns. The formatter fixes all three.

### 4.2 Comments and notes

```tessera
// A comment explains code to the reader. Comments are preserved by the formatter
// but carry no meaning for the compiler.

note "A note records design intent. Notes are part of the program's metadata."
```

The difference is deliberate: comments are for reading, notes are for tooling. See section 10.

There are no block comments (`/* */`). Nesting rules and unterminated comments are a known source of parser bugs and confusing diffs.

### 4.3 Identifiers and naming

| Kind | Style | Example |
|---|---|---|
| Functions, parameters, variables | `snake_case` | `total_price`, `is_valid` |
| Types | `PascalCase` | `Int`, `List` |
| Effects | lowercase single word | `io` |

Naming style is **enforced by the compiler** (error, not warning). Consistent names make code easier for an LLM to predict and for a human to scan.

Identifiers are ASCII letters, digits, and `_`, starting with a letter. Non-ASCII identifiers are rejected to avoid homoglyph attacks (`а` Cyrillic vs `a` Latin). Text literals may contain any Unicode.

### 4.4 Keywords

```
fn  let  var  set  if  else  for  in  while  note  uses
and  or  not  true  false
```

Reserved for future use (rejected as identifiers): `type`, `match`, `return`, `break`, `continue`, `module`, `import`, `pub`, `try`, `unsafe`.

### 4.5 Literals

| Literal | Examples | Notes |
|---|---|---|
| Int | `0`, `42`, `1_000_000` | Decimal only in POC. `_` separators allowed between digits. Leading zeros rejected (`007` is an error). |
| Bool | `true`, `false` | |
| Text | `"hello"`, `"line\n"` | Escapes: `\n`, `\t`, `\\`, `\"`, `\u{1F600}`. No raw or multiline strings in POC. |
| Unit | `()` | Rarely written; it is the value of statements. |
| List | `[1, 2, 3]`, `[]` | Elements must share one type. Empty list needs a type annotation (section 6.1). |

Negative numbers are the unary `-` applied to a literal. The literal `-9223372036854775808` is handled as a special case so the full `Int` range is expressible.

### 4.6 Statement separation

Statements end at a newline. There are no semicolons. A line continues onto the next line only when:

- it ends inside an open `(` or `[`, or
- it ends with a binary operator (`+`, `and`, `==`, ...) or a comma.

```tessera
let total = base_price
    + shipping        // ERROR: line above ended without an operator

let total = base_price +
    shipping          // OK
```

This is the rule Go and Kotlin users already know. The formatter always puts the operator at the end of the line.

## 5. Types

### 5.1 Built-in types

| Type | Meaning |
|---|---|
| `Int` | Signed 64-bit integer. Overflow is a runtime error, never wraps silently. |
| `Bool` | `true` or `false`. |
| `Text` | Immutable UTF-8 text. |
| `Unit` | The type with one value `()`. Returned by functions that only perform effects. |
| `List[T]` | Immutable, ordered, bounds-checked sequence of `T`. `T` is any type, including `List[...]`. |

User-defined types (records, enums), generics on user functions, and an optional/result type are deferred. They are the first expansion candidates (section 13).

### 5.2 No implicit conversions

Nothing converts automatically. `"Age: " + 20` is a type error; write `"Age: " + to_text(20)`. `if 1 { ... }` is a type error; conditions must be `Bool`.

### 5.3 Type inference

- Function parameters and return types are **always written**. A signature is the review contract; it must be readable without inference.
- `let` and `var` types are inferred from the initializer and may be written explicitly: `let count: Int = 0`.

## 6. Bindings and mutation

### 6.1 Immutable bindings with `let`

```tessera
let greeting = "hello"
let scores: List[Int] = []
```

A `let` binding can never change.

### 6.2 Mutable bindings with `var` and `set`

```tessera
var total = 0
for price in prices {
    set total = total + price
}
```

Mutation is visible twice: at the declaration (`var`) and at every change (`set`). A reviewer can search for `set` to find every mutation in a file.

Rules:

- `set` only works on a `var` declared in the same function. Parameters are never mutable.
- A `var` that is never `set` is a compiler error (use `let`).
- Lists are immutable values. To build a list, create a new one: `set names = append(names, "Ada")`. In-place mutation of collections is deferred until the mutation model is reviewed.

### 6.3 No shadowing

A name cannot be redeclared while it is in scope, including in nested blocks:

```tessera
let count = 1
if ready {
    let count = 2    // ERROR E0204: `count` is already defined on line 1
}
```

Shadowing is a common cause of review mistakes where a reader assumes the outer value.

## 7. Expressions

Tessera is expression-oriented: blocks and `if` produce values.

### 7.1 Operators

From highest to lowest precedence:

| Precedence | Operators | Operand types | Result |
|---|---|---|---|
| 1 | `f(x)`, `xs[i]` | call, index | |
| 2 | `-x`, `not x` | `Int`, `Bool` | same |
| 3 | `*`, `/`, `%` | `Int` | `Int` |
| 4 | `+`, `-` | `Int`; `+` also on `Text` | same |
| 5 | `==`, `!=`, `<`, `<=`, `>`, `>=` | see below | `Bool` |
| 6 | `and` | `Bool` | `Bool` |
| 7 | `or` | `Bool` | `Bool` |

- All binary operators are left-associative, except comparisons, which **do not chain**: `a < b < c` is a syntax error.
- `and` and `or` short-circuit.
- `==` and `!=` work on any two values of the same type, comparing by value (lists element by element).
- `<`, `<=`, `>`, `>=` work on `Int` only in the POC.
- Mixing `and` and `or` without parentheses is an error: write `(a and b) or c`. This removes a common precedence mistake.
- Integer `/` truncates toward zero. `/` or `%` by zero is a runtime error.
- `+`, `-`, `*` that overflow `Int` are a runtime error.

`and`, `or`, `not` are words rather than `&&`, `||`, `!`, because they read as English and avoid confusion with `!` used elsewhere in other languages.

### 7.2 Blocks

A block `{ ... }` is a sequence of statements. Its value is the value of the last line if that line is an expression; otherwise `()`.

```tessera
let area = {
    let width = 4
    let height = 5
    width * height
}
```

### 7.3 `if`

```tessera
let kind = if count == 0 { "empty" } else { "has items" }

if verbose {
    print("done")
}
```

- The condition must be `Bool`. No parentheses around it; braces are always required.
- If the value of an `if` is used, `else` is required and both branches must have the same type.
- `else if` chains are allowed: `if a { ... } else if b { ... } else { ... }`.

### 7.4 Evaluation order

Evaluation is strictly left to right: function arguments, operands, and list elements are evaluated in written order. This is part of the language, not a backend detail, so programs behave deterministically.

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

- `for` iterates over a `List[T]`. The loop variable is an immutable binding of type `T`.
- `for` is a statement; its value is `()`.
- The list is evaluated once before the loop starts.

### 8.2 `while`

```tessera
var remaining = 10
while remaining > 0 {
    set remaining = remaining - 1
}
```

`while` is included so that loops with mutation are possible. `break`, `continue`, and `return` are reserved but deferred: they are the first thing to revisit after the sample programs show whether they are missed.

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
- No default arguments, no overloading, no variadic parameters. One name, one signature.

### 9.2 Effects with `uses`

An **effect** is a capability to act on the world outside the program. The POC has one effect:

| Effect | Allows |
|---|---|
| `io` | Writing to standard output and standard error. |

A function declares the effects it needs after the return type:

```tessera
fn greet(name: Text) -> Unit uses io {
    print("Hello, " + name)
}
```

Rules:

1. A function may only call functions whose effects are all in its own `uses` list. Calling `greet` from a function without `uses io` is error E0301.
2. A function that declares an effect it never uses is error E0302. Signatures must not overstate what code can do, or reviewers learn to ignore them.
3. `main` is the only entry point. It may declare any effect.
4. Functions without `uses` are **pure**: same inputs, same output, no observable action. They may still fail at runtime (section 11).

The effect list is written `uses io` for one effect and `uses io, net` for several (once more effects exist). Expected future effects: `fs` (files), `net` (network), `env` (environment variables and arguments), `time`, `random`. Splitting `io` this finely is deliberate: "this function can read files" and "this function can open network connections" are very different review questions.

### 9.3 `main`

```tessera
fn main() -> Unit uses io { ... }
```

`main` takes no parameters and returns `Unit` in the POC. Command-line arguments and exit codes are deferred to the `env` effect.

## 10. Notes

A **note** records design intent: why something exists, what constraint it encodes, what must not change without review.

```tessera
note "Age limit is set by legal requirement NL-2024-17; do not change without legal review."
note "Kept pure so it can be unit tested without stubs."
fn is_adult(age: Int) -> Bool = age >= 18
```

Rules:

- A note attaches to the **next function declaration**. Several notes may precede one function; they keep their order.
- A note that is not followed by a function is error E0401 (a note must never silently drift away from what it describes).
- Notes have no effect on type checking, effects, or runtime behavior.
- The parser, formatter, and code generator must preserve notes exactly, including order and text.
- In the POC notes attach to functions only. Notes on statements, types, and modules are expected later.

### 10.1 Metadata export

`tessera notes program.tes` writes the notes as JSON:

```json
{
  "file": "program.tes",
  "notes": [
    {
      "item": "is_adult",
      "kind": "function",
      "line": 3,
      "text": [
        "Age limit is set by legal requirement NL-2024-17; do not change without legal review.",
        "Kept pure so it can be unit tested without stubs."
      ]
    }
  ]
}
```

The same structure is embedded in the typed intermediate representation, so later tools (review bots, documentation generators, model training pipelines) can rely on it.

## 11. Runtime errors

Some errors can only be detected while running. In the POC there is no way to catch them; the program stops in a defined way.

| Situation | Error code |
|---|---|
| List index out of bounds | R001 |
| Integer overflow | R002 |
| Division or remainder by zero | R003 |
| `range` with start greater than end | R004 |

When a runtime error occurs:

1. Nothing more is written to standard output.
2. A message is written to standard error:
   ```
   runtime error R001: list index 5 is out of bounds for a list of length 3
     at program.tes:12:15 in function `label`
   ```
3. The process exits with code **70** (`EX_SOFTWARE`).

Exit code and message format are defined by Tessera, not inherited from the Rust backend.

Recoverable error handling (a result type and propagation syntax) is the most important deferred feature. It is postponed so it can be designed after we see real POC programs, not guessed at now.

## 12. Standard library (POC)

The POC standard library is a fixed set of built-in functions. They are ordinary functions with ordinary signatures; there is no method syntax (`xs.length()`) in the POC.

| Function | Signature | Notes |
|---|---|---|
| `print` | `(value: Text) -> Unit uses io` | Writes `value` and a newline to standard output. |
| `print_error` | `(value: Text) -> Unit uses io` | Same, to standard error. |
| `to_text` | `(value: Int) -> Text` | Decimal representation. Also accepts `Bool` (`"true"` / `"false"`). |
| `length` | `(list: List[T]) -> Int` | Number of elements. |
| `length` | `(text: Text) -> Int` | Number of Unicode scalar values (not bytes). |
| `append` | `(list: List[T], item: T) -> List[T]` | Returns a new list. |
| `range` | `(start: Int, end: Int) -> List[Int]` | `start` up to but not including `end`. |

`to_text`, `length`, and `append` are the only overloaded or generic functions, and only because user-defined generics are deferred. Users cannot write overloads.

## 13. Compiler diagnostics

Diagnostics are part of the language design: an LLM fixing code relies on them as much as a person does.

Format:

```
error[E0301]: `greet` uses effect `io`, but `format_report` does not declare it
  --> report.tes:14:5
   |
14 |     greet(name)
   |     ^^^^^^^^^^^ this call needs `io`
   |
help: add `uses io` to the signature of `format_report` on line 11,
      or move the call to a function that already uses `io`
```

Rules:

- Every diagnostic has a stable code (`E0301`), a file, a line, a column, and a one-line message.
- Every error suggests at least one concrete fix where one exists.
- Error codes are never reused for a different meaning.
- `tessera check --json` emits the same diagnostics as JSON for tools and agents.
- The compiler reports all errors it can find in one run, not just the first, but never cascades errors caused by an earlier error.

Initial error code ranges:

| Range | Area |
|---|---|
| E01xx | Lexing and parsing |
| E02xx | Names and bindings |
| E03xx | Effects |
| E04xx | Notes |
| E05xx | Types |
| R0xx | Runtime |

## 14. Canonical formatting

`tessera format` produces exactly one layout for any program. The compiler does not reject unformatted code, but CI is expected to run `tessera format --check`.

- 4 spaces indentation, no tabs.
- One space around binary operators and after commas and colons: `fn f(a: Int, b: Int) -> Int`.
- Opening brace on the same line.
- Maximum line length 100; longer expressions break after an operator.
- One blank line between functions. Notes sit directly above their function with no blank line between.
- Formatting twice gives the same result as formatting once.

## 15. Grammar (EBNF)

Newline handling follows section 4.6. `NEWLINE` below means a statement-ending newline.

```ebnf
program        = { note_list function } ;
note_list      = { "note" TEXT NEWLINE } ;
function       = "fn" NAME "(" [ params ] ")" "->" type [ effects ] body ;
params         = param { "," param } [ "," ] ;
param          = NAME ":" type ;
effects        = "uses" NAME { "," NAME } ;
body           = "=" expression | block ;

type           = TYPE_NAME [ "[" type "]" ] ;

block          = "{" { statement NEWLINE } [ expression ] "}" ;
statement      = let_stmt | var_stmt | set_stmt | for_stmt | while_stmt | expression ;
let_stmt       = "let" NAME [ ":" type ] "=" expression ;
var_stmt       = "var" NAME [ ":" type ] "=" expression ;
set_stmt       = "set" NAME "=" expression ;
for_stmt       = "for" NAME "in" expression block ;
while_stmt     = "while" expression block ;

expression     = or_expr ;
or_expr        = and_expr { "or" and_expr } ;         (* no mixing with and, see 7.1 *)
and_expr       = comparison { "and" comparison } ;
comparison     = additive [ compare_op additive ] ;   (* no chaining *)
additive       = multiplicative { ( "+" | "-" ) multiplicative } ;
multiplicative = unary { ( "*" | "/" | "%" ) unary } ;
unary          = ( "-" | "not" ) unary | postfix ;
postfix        = primary { "(" [ args ] ")" | "[" expression "]" } ;
args           = expression { "," expression } [ "," ] ;
primary        = INT | TEXT | "true" | "false" | "(" ")" | NAME
               | "(" expression ")" | list | block | if_expr ;
list           = "[" [ args ] "]" ;
if_expr        = "if" expression block [ "else" ( if_expr | block ) ] ;
compare_op     = "==" | "!=" | "<" | "<=" | ">" | ">=" ;
```

Known grammar risk to test in the parser prototype: a block `{` directly after an `if` or `while` condition, versus a block expression used as the condition. The rule is that the first `{` after the condition starts the body; a block as a condition must be parenthesized. Rust uses the same rule.

## 16. Compilation model

```
source.tes
  -> lexer / parser       -> syntax tree (with notes and comments)
  -> name resolution      -> E02xx
  -> type check           -> E05xx
  -> effect check         -> E03xx
  -> typed IR (JSON-exportable, includes notes)
  -> Rust code generation (safe Rust only, no `unsafe`)
  -> rustc                -> native executable
```

Guarantees the compiler must keep:

- No code is generated for a program with any error.
- Generated Rust contains no `unsafe` (enforced by a test that scans output).
- The same source and compiler version produce byte-identical generated Rust.
- Tessera semantics (overflow, evaluation order, runtime error format, exit codes) are implemented explicitly, never by relying on Rust defaults.

## 17. Deferred, in expected order

1. Recoverable errors: a result type and a propagation operator.
2. `return`, `break`, `continue`.
3. Records and enums with `match`.
4. More effects: `fs`, `env` (arguments, exit code), `time`, `random`, `net`.
5. Modules and imports.
6. User-defined generics.
7. Text interpolation (`"Hello, {name}"`).
8. Notes on statements and types.

Not planned for the POC at all: concurrency, FFI, `unsafe`, package management, GUI, web frameworks.

## 18. Open questions

These should be answered by writing sample programs against this draft, not by debate:

1. **Token cost.** Does readable spacing and `uses io` instead of `!{io}` cost meaningfully more tokens with the target tokenizer? Measure on the sample programs.
2. **Mandatory `else` for unused `if` values.** Is the `else`-optional rule for statements clear enough, or should every `if` be treated the same?
3. **Error on unused effects (E0302).** Helpful or annoying during refactoring? Could be a warning instead.
4. **`var` lists.** Is "rebuild with `append`" acceptable for POC programs, or does it make simple programs awkward enough to need in-place mutation sooner?
5. **Note placement.** Are notes better above the function (current) or inside it as the first line? Above matches doc comments in other languages; inside can't become detached.
6. **Naming enforcement as error.** Correct for LLM-generated code, but may frustrate people porting code. Keep as error unless samples show real friction.
