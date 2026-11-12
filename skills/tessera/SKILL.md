---
name: tessera
description: Write, fix, review and run programs in Tessera, a statically typed, effect-checked language designed for LLMs (files ending in .tes, compiled with tessera1). Use whenever the task involves reading or writing .tes code, Tessera modules or std/ modules, Tessera web applications (`handle`), Tessera ui/audio programs, or interpreting Tessera compiler diagnostics (error[E....]) and runtime errors (R0..).
---

# Tessera

Tessera is not in your training data. Do not guess syntax from Rust, Swift or Python: it has its own rules (no indentation, `set` for every change, effects in signatures, no shadowing, no closures) and the compiler rejects near misses.

## Before writing code

1. Read `reference.md` in this skill directory completely. It is the language reference the measurement harness gives to models, and with it alone they pass every harness task on the first attempt. Read it once per session, before the first `.tes` edit.
2. For a detail the reference does not settle (exact error-code meaning, runtime limits, grammar, why a rule exists), read the relevant section of `language-design.md` in this directory, the authoritative specification. Use its heading list (`grep -n '^#'`) to jump; do not read all of it.
3. For a standard module (`import std/...`), read its source in the Tessera checkout's `std/` directory when you need more than the summary in the reference.
4. In an existing project, read a few of its `.tes` files first and match their conventions.

## Toolchain

The compiler is `tessera1`. Find it with `command -v tessera1`, otherwise use `bin/tessera1` in a Tessera-language checkout (build it there with `./build.sh`; it needs a Rust toolchain, often in `~/.cargo/bin`, which non-login shells may not have on `PATH`).

```
tessera1 check main.tes            # type and effect check; fast, use after every edit
tessera1 build main.tes -o prog    # native executable (also writes prog.rs)
tessera1 test main.tes             # runs `test "name" { assert(...) }` blocks
tessera1 format --check files...   # canonical layout
```

Give only the entry file; imported modules are found from its directory. Compiler exit codes: 0 ok, 1 compile errors, 2 usage error. A compiled program exits with 70 on a runtime error.

Loop: write, `check`, read every diagnostic (each has a code and usually a `help:` line that says exactly what to change), fix all of them, repeat; then `build` or `test` and run it. The compiler reports all errors in one pass, so fix them together.

## Canonical style

- No indentation at all: every line starts in column 0; braces carry the structure. Tabs are a compile error.
- `.Variant` instead of `Type.Variant` where the type is known; `"\(x)"` interpolation instead of `+ to_text(x)`.
- `let n = parse_int(t) else exit_with(1, "error: ...")` instead of a four-line `match`.
- `set xs += [item]`, `set total += x`; errors to standard error with `exit_with(code, message)`.
- Prefer `Result`/`Option` and `try` for expected failures; `fail(...)` only for impossible states.
- Exhaustive `match` listing every variant rather than `_`, so new variants cause errors where they must be handled.
- `note "..."` directly above a declaration records design intent (it is part of the program's metadata, unlike `//` comments).

## Mistakes the compiler will reject

- Changing a value without `set`, or `set` on a `let`/parameter/loop variable (E0205). A `var` never `set` is also an error (E0206).
- Reusing a name in a nested block, loop variable or match binding: there is no shadowing (E0204). The same holds for a local named like an imported module (`let keys` next to `import std/keys`).
- Keywords as names: `note`, `type`, `match`, `in`, `with`, ... (E0110).
- Naming your function like a built-in, also inside a module (`to_int`, `draw_line`, `text_width`): E0211. Pick another name.
- Declaring an effect that is not used (E0302, except in `main`) or calling something whose effect is not declared (E0301).
- An expression statement whose value is dropped (E0508): use it or write `let _ = ...`.
- `else` on a new line instead of `} else {` (E0111); a line break before a binary operator (the statement already ended).
- Mixing `Int`, `Decimal` and `Float` (E0503). `var total = 0.0` is a `Decimal`; write `var total: Float = 0.0` for a float.
- `.None`, `.Ok(x)`, `.Error(e)` or `[]` without a known type (E0505/E0518). `if c { Result.Ok(1) } else { Result.Error("no") }` needs an annotation on the binding.
- Nested patterns such as `.Error(.NotFound)`: match `.Error(e)`, then `if e == .NotFound`.
- `*=`, `/=`, `_x` names, closures, methods (`xs.length()`), default arguments, overloading, string indexing (`text[0]`), chained comparisons, `and` mixed with `or` without parentheses: none exist.
- A record literal in an `if`/`while`/`for`/`match` header needs parentheses.

## Performance traps (compile fine, run slowly)

Values never share state. Large lists or records are updated in place only when threaded as `set x = f(x, ...)` (also through `if`/`match` branches, `return f(x)`, or a function body starting with `var out = param`). `let y = f(x)` while `x` is still used later copies the whole value. In a hot loop (frame buffers, big lists), keep the `set x = f(x)` shape.

## Web applications

An entry file with `fn handle(request: http.Request) -> http.Response` and no `main` is a web application (`import std/http`). `handle` may not use `io` or `ui`; log with `log` (`uses log`). Test it without a server: `prog request GET /path`, or `prog serve --port 8080`. `db_open` starts an `sqlite3` child process, so open the database once per request, not per query.

## When the user is working on the Tessera compiler itself

Changing the language, `stage1/`, `runtime/`, `std/` or the bootstrap seed follows the rules in the Tessera-language repository's `CLAUDE.md` (stage 0 is frozen, seed regeneration before using new features in stage 1, fixpoint check, embedded runtime). Read that file first.
