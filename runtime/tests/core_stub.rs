#![forbid(unsafe_code)]
#![allow(unused, dead_code, non_snake_case, non_camel_case_types, unreachable_code, unreachable_patterns, unused_parens, unused_braces, redundant_semicolons, while_true, clippy::all)]

// Test stub of the core.rs items that runtime/web.rs relies on.

use std::io::Write;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct TFailure { pub code: String, pub message: String, pub location: String }
#[derive(Debug, Clone)]
pub struct TExit { pub code: i32 }

fn t_fail(code: &str, message: String, location: &str) -> ! {
    std::panic::panic_any(TFailure { code: code.to_string(), message, location: location.to_string() })
}
fn t_limits_begin(stack_bytes: usize, time_limit_ms: u64, memory_limit_bytes: u64) {}
fn t_install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.payload().is::<TFailure>() || info.payload().is::<TExit>() { return; }
        previous(info)
    }));
}
// Same semantics as core.rs: paths are relative to TESSERA_FS_ROOT; absolute paths, `..` and symlinks leading outside are refused.
fn t_check_path(path: &str, loc: &str) -> std::path::PathBuf {
    let root = match std::env::var_os("TESSERA_FS_ROOT") { Some(r) if !r.is_empty() => std::path::PathBuf::from(r), _ => return std::path::PathBuf::from(path) };
    let relative = std::path::Path::new(path);
    let plain = relative.components().all(|c| matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir));
    if !plain { t_fail("R016", format!("path `{}` is outside the allowed directory", path), loc) }
    let full = root.join(relative);
    if let Ok(canonical_root) = root.canonicalize() {
        if let Some(existing) = full.ancestors().find(|a| a.exists()) {
            if let Ok(real) = existing.canonicalize() {
                if !real.starts_with(&canonical_root) { t_fail("R016", format!("path `{}` is outside the allowed directory", path), loc) }
            }
        }
    }
    full
}
fn t_text(s: &str) -> Rc<String> { Rc::new(s.to_string()) }
