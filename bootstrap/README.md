# Bootstrap seed

`tessera1.rs` is the Rust code that the stage 1 compiler generates for its own
source. Compiling it with `rustc` gives a working Tessera compiler, which then builds
the real compiler from `stage1/`:

    rustc bootstrap/tessera1.rs  ->  bin/tessera-bootstrap  ->  bin/tessera1 (from stage1/)

This is how most self-hosted languages bootstrap: from a previous build of
themselves. Do not edit the file; it is generated.

## Where it comes from

The chain starts at the stage 0 compiler (`compiler/`, Rust). The first seed was made
on 2026-11-06 like this, and the result was byte-identical to what the stage 1
compiler produces for itself:

    stage 0  ->  flat module-free copy of stage 1 (git history, bootstrap/*.tes)
             ->  stage1/  ->  stage1 emit-rust stage1/main.tes  =  tessera1.rs

Every later seed is produced by a compiler that was itself built from the previous
seed, and `build.sh` checks on every build that `stage1/` compiles itself to a
fixpoint. The file is plain safe Rust (`#![forbid(unsafe_code)]`), so it can be
reviewed like any other generated code.

## When and how to update it

The seed only needs updating when `stage1/` should start using a language feature
the seed does not support yet. Do it in two steps:

1. Implement the feature in `stage1/` without using it there. Run `./build.sh`, then
       bin/tessera1 emit-rust stage1/main.tes -o bootstrap/tessera1.rs
   and commit. The new seed understands the feature.
2. Only then start using the feature in `stage1/`.
