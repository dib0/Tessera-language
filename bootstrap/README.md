# Bootstrap compiler

A frozen copy of the stage 1 compiler from just before `stage1/` was split into
modules. It is written without modules, so the stage 0 compiler (Rust, frozen at
draft 0.3) can still build it, and it implements modules, so it can build the
modular compiler in `stage1/`:

    stage 0 (Rust)  ->  bootstrap/ (Tessera, no modules)  ->  stage1/ (Tessera, modules)

Do not develop here. Update this copy only when `stage1/` starts using a language
feature that this compiler does not support yet; then replace it with a version of
the compiler that both supports the feature and can itself be built by the
previous link in the chain.
