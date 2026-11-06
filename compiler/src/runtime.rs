//! Runtime support emitted at the top of every generated Rust program.
//! The source lives in `runtime/` so that the stage 1 compiler embeds exactly the same code.
//! Everything there is safe Rust; generated programs use `#![forbid(unsafe_code)]`.

pub const PRELUDE: &str = include_str!("../../runtime/core.rs");
