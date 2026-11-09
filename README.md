<h1 align="center">
  <img src="resources/tessera-logo.svg" alt="Tessera" width="260" />
</h1>

<p align="center"><strong>A compact, safety-first programming language designed for LLM-assisted software development.</strong></p>

The Tessera language aims to make it easy for language models to create, review, and maintain software without sacrificing clear behavior or safety. Its source is text, its syntax is intended to be compact and regular, and design rationale is preserved alongside code. **This is for humans that care about functionality of software, not about the code.**

> **Status:** Working proof of concept. The compiler is self-hosting: it is written in Tessera and compiles itself. The language is not yet stable; see [`language-design.md`](language-design.md) for the current specification (draft 0.4).

## Design goals

- **Compact, regular source:** Reduce unnecessary boilerplate without relying on cryptic abbreviations. Measure compactness using the intended model tokenizer, not character count alone.
- **Reviewable programs:** Make behavior, data flow, and external effects clear to an LLM reviewer.
- **Safety by default:** The planned safe subset excludes raw pointers and unchecked memory access, and uses bounds-checked collections.
- **Preserved rationale:** Attach design notes to declarations and preserve them through compiler tooling.
- **Portable native software:** The long-term goal is to compile for Linux, Windows, and macOS, with libraries for scripts, web applications, and desktop applications.
- **Self-hosting:** The long-term goal is to implement the Tessera compiler in Tessera itself.

These are design goals, not guarantees of the current prototype.

## Syntax

```tessera
note "Keep the age policy in one review point."
fn is_adult(age: Int) -> Bool = age >= 18

fn label(name: Text, age: Int) -> Text {
let category = if is_adult(age) { "adult" } else { "minor" }
category + ": " + name
}

fn main() -> Unit uses io = print(label("Ada", 20))
```

The `note` form records design intent and is preserved by the compiler. `uses io` declares that a function performs I/O; a function without `uses` cannot act on the outside world, and the compiler checks this. Mutation is explicit (`var` and `set`), there are no implicit conversions, and `match` must handle every case.

The original compact sketch (`fn adult(age:i64)->bool`, `!{io}`) was replaced by this more readable form; the reasons are in section 3 of the specification.

## Compiler bootstrap

Tessera became self-hosting through staged compilation:

1. **Stage 0** (done, `compiler/`): an initial compiler in Rust. It compiles Tessera to safe Rust, then uses `rustc` to produce an executable.
2. **Stage 1** (done, `stage1/`): the Tessera compiler written in Tessera (about 5,100 lines), compiled by stage 0.
3. **Stage 2** (done): stage 1 compiles its own source to exactly the same Rust as stage 0 does. The same tests run against both compilers, and `build.sh` checks this fixpoint on every build.

New language features are now added to stage 1 only: the `ui` effect for simple graphical programs (used by [`examples/tictactoe/`](examples/tictactoe/)), modules (`import`, `pub`), `Option[T]`, `Result[T, E]` with `try`, record updates (`value with { field: x }`), `.Variant` where the type is known, text interpolation (`"found \(count) items"`), `for _` and one-statement `match` arms, `set x += e`, `+` on lists, and the built-ins `parse_int`, `split`, `sort`, `index_of`, `contains`, `to_lower`, `to_upper` and `exit_with`. Stage 1 itself uses these, so it is built from a seed: [`bootstrap/tessera1.rs`](bootstrap/) is the Rust code stage 1 generates for itself, compiled with `rustc`. The first seed was produced through the full chain from stage 0; stage 0 remains the independent reference implementation.

Rust is the initial implementation and code-generation path, not part of Tessera's source language design. Native code generation can be considered after the language and compiler behavior are established.

## Getting started

Requires Linux and Rust (`rustc` and `cargo`, from https://rustup.rs). Graphical programs need an X11 session.

```sh
./build.sh                                    # stage 0, stage 1, self-hosting check
bin/tessera1 build examples/tictactoe/tictactoe.tes -o bin/tictactoe
bin/tictactoe
bin/tessera1 check examples/invalid/E0301-undeclared-effect.tes
(cd compiler && cargo test --release)         # tests for all compilers
```

## Web applications

A program whose entry file has `fn handle(request: http.Request) -> http.Response` instead of `main` is a web application. The same program runs as a CGI program, as an HTTP server (also behind IIS with HttpPlatformHandler), as a FastCGI server for nginx and Apache, or in-process as a shared library loaded by the modules for nginx, Apache and IIS in [`hosts/`](hosts/README.md). Each request runs on its own thread with stack, time and memory limits; a runtime error gives that request a 500 response instead of ending the server.

```sh
bin/tessera1 build tests/web/app.tes -o app && ./app serve --port 8080   # or: ./app request GET /
bin/tessera1 build tests/web/app.tes --library -o libapp.so             # for the server modules
bin/tessera1 test tests/web/app.tes                                      # run the `test` declarations
```

What applications needed beyond command-line programs, and what draft 0.13 implemented, is in [`application-roadmap.md`](application-roadmap.md).

## Measuring language changes

Language changes are measured, not guessed: [`harness/`](harness/) counts real Claude tokens of Tessera code per language variant, and lets a model solve 21 programming tasks per variant while measuring compile success, test success, fix rounds and tokens. See [`harness/README.md`](harness/README.md).

## Initial POC scope

The first prototype covered a small, useful command-line program (it has since grown; see the specification):

- Functions, parameters, return types, and local bindings
- Basic types: `i64`, `bool`, `str`, and `unit`
- Arithmetic, comparisons, conditionals, and string concatenation
- Lists, iteration, and checked indexing
- Explicit I/O effects and a minimal print operation
- Design notes preserved in compiler metadata
- Clear diagnostics and tests for invalid programs

Generics, concurrency, foreign-function interfaces, package management, GUI APIs, and web frameworks are outside the initial POC scope.

## Safety and security

Tessera is in very early stages and must not (yet) be treated as a security guarantee. The compiler, generated programs, and runtime will need testing and review. The initial design intends to avoid generating Rust `unsafe` code and to test that property, but that does not by itself prove the compiler or resulting software secure.

