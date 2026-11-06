# Bootstrap compiler

Generated, module-free source of the stage 1 compiler. Stage 0 (Rust, frozen at
draft 0.3) can build it because it uses no modules and none of the newer language
features; it implements all of them, so it can build the real compiler in `stage1/`:

    stage 0 (Rust)  ->  bootstrap/ (Tessera, flat)  ->  stage1/ (Tessera, modules)

Do not edit these files. They are produced by `tools/flatten.tes`, which removes
`import` lines, `pub` markers and module qualifiers from `stage1/`.

When to regenerate: when you add a language feature to `stage1/` and want the
compiler's own source to use it. Do it in two steps:

1. Implement the feature in `stage1/` without using it there, and run
       compiler/target/release/tessera run tools/flatten.tes -- stage1 bootstrap \
           source lexer ast parser checker codegen runtime_text main
   so `bootstrap/` understands the feature but is still buildable by stage 0.
2. Only then start using the feature in `stage1/`.

Draft 0.6 was done this way: `bootstrap/` supports `Option` and `with`, and
`stage1/` uses them.
