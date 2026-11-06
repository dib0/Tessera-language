# Tessera: POC design

**Status:** Draft 0.1. Tessera is the working name for the language.

## Goal

Design a language that makes software functionality easy for LLMs to create, review, and maintain. Source is human-readable text for now. The compiler, not the source encoding, is the first POC. The language should make unsafe behavior difficult to express and preserve design rationale alongside code. Prefer compact, regular syntax over redundant boilerplate, but avoid cryptic abbreviations.

## Provisional design choices

- **Paradigm:** Expression-oriented, with functions and data types. Immutable by default; mutation is explicit. This is a starting point, not a commitment to functional purity.
- **Safety:** No raw pointers or unchecked memory access in the safe language. Bounds-checked collections. No `unsafe` in the POC. The compiler emits only safe Rust initially, using Rust as a backend rather than exposing Rust syntax as the language.
- **Effects:** Functions declare capabilities such as `io`, so reviewers can see which code can perform external actions.
- **Rationale notes:** A `note` declaration attaches design intent to the following item. The parser and formatter must preserve notes; notes do not affect runtime behavior.
- **Portability:** The POC runs on Linux. Cross-platform compilation is a later acceptance milestone, not something to claim before testing on Windows and macOS.

## Sketch of the source

```text
note "Keep age policy in one review point."
fn adult(age:i64)->bool = age>=18

fn label(name:str,age:i64)->str =
  (adult(age) ? "adult: " : "minor: ")+name

fn main()->unit !{io} = print(label("Ada",20))
```

This compact syntax is illustrative. The parser prototype should test that declarations, effects, and the `note` form remain unambiguous and easy for an LLM to edit. Measure compactness by token count with the intended model tokenizer, not by character count.

## POC language scope

Include only what is needed to compile a small, useful CLI program:

- Functions, parameters, return types, and local `let` bindings
- `i64`, `bool`, `str`, and `unit`
- Arithmetic, comparisons, string concatenation, and `if` expressions
- Lists with bounds-checked indexing and iteration
- Explicit `io` capability with a minimal print operation
- Compiler diagnostics with file, line, and actionable error text
- `note` metadata preserved in a machine-readable intermediate representation

Defer generics, concurrency, FFI, package management, GUI APIs, web frameworks, and user-defined unsafe operations.

## Compiler approach

Implement the compiler in Rust. Use a small pipeline: lexer/parser, typed intermediate representation, effect checks, then safe Rust code generation. Keep the language specification authoritative; do not let accidental Rust behavior define the language. Initially require Rust to be installed to compile generated output. Later, bundle the toolchain or replace the backend if distribution requirements justify it.

## Security and quality rules

- Reject type errors and undeclared effects before code generation.
- Generate no Rust `unsafe` blocks in the POC; add a test that scans generated output for them.
- Fuzz the lexer and parser, and test malformed input for crashes and hangs.
- Keep compiler dependencies small and pinned.
- Treat generated code as untrusted until it passes tests and review.

## POC acceptance criteria

1. A sample CLI program compiles and runs on Linux.
2. Invalid types, out-of-bounds access at runtime, and undeclared I/O are handled safely and predictably.
3. Notes survive parsing, formatting, and code generation metadata export.
4. The same source produces deterministic output across repeated builds.
5. Tests cover valid programs, invalid programs, parser fuzz cases, and generated-code safety.

## Next design decisions

After the first parser and sample program, review the syntax, mutation model, error handling, and effect notation. Only then expand the standard library or begin model training. The POC should test language usability and compiler correctness, not claim token savings or cross-platform support prematurely.
