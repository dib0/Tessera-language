#![forbid(unsafe_code)]
#![allow(unused, dead_code, non_snake_case, non_camel_case_types, unreachable_code, unreachable_patterns, unused_parens, unused_braces, redundant_semicolons, while_true, clippy::all)]

use std::io::Write;
use std::rc::Rc;

// Runtime errors and exit unwind (as Rust panics with these payloads) to the host boundary:
// `t_run` turns them into a message and an exit code, a web host into a 500 response.
pub struct TFailure { pub code: String, pub message: String, pub location: String }
pub struct TExit { pub code: i32 }

fn t_fail(code: &str, message: String, location: &str) -> ! {
    std::panic::panic_any(TFailure { code: code.to_string(), message, location: location.to_string() })
}
fn t_report_failure(f: &TFailure) {
    #[cfg(target_arch = "wasm32")]
    { crate::wasm_write(2, format!("runtime error {}: {}\n  at {}\n", f.code, f.message, f.location).as_bytes()); return; }
    let _ = std::io::stdout().flush();
    eprintln!("runtime error {}: {}\n  at {}", f.code, f.message, f.location);
}
// In WebAssembly a panic aborts (a trap), so the hook itself reports the end of the run to the host.
#[cfg(target_arch = "wasm32")]
fn t_install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let payload = info.payload();
        if let Some(failure) = payload.downcast_ref::<TFailure>() {
            t_report_failure(failure);
            let _ = crate::wasm_put("exit", b"70");
        } else if let Some(exit) = payload.downcast_ref::<TExit>() {
            let _ = crate::wasm_put("exit", exit.code.to_string().as_bytes());
        } else {
            crate::wasm_write(2, format!("runtime error R000: internal error: {}\n", info).as_bytes());
            let _ = crate::wasm_put("exit", b"70");
        }
    }));
}
#[cfg(not(target_arch = "wasm32"))]
fn t_install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let payload = info.payload();
            if payload.is::<TFailure>() || payload.is::<TExit>() { return; }
            default_hook(info);
        }));
    });
}

// Limits of the current thread: stack depth, time and an allocation budget (0 = no limit).
thread_local! {
    static T_STACK_BASE: std::cell::Cell<usize> = std::cell::Cell::new(0);
    static T_STACK_LIMIT: std::cell::Cell<usize> = std::cell::Cell::new(0);
    static T_DEADLINE: std::cell::Cell<Option<std::time::Instant>> = std::cell::Cell::new(None);
    static T_TICKS: std::cell::Cell<u32> = std::cell::Cell::new(0);
    static T_MEMORY_LEFT: std::cell::Cell<i64> = std::cell::Cell::new(-1);
}
// The address of a local variable approximates the stack position; taking it is safe, it is never dereferenced.
fn t_stack_address() -> usize { let marker = 0u8; std::hint::black_box(&marker) as *const u8 as usize }
fn t_limits_begin(stack_bytes: usize, time_limit_ms: u64, memory_limit_bytes: u64) {
    T_STACK_BASE.with(|b| b.set(t_stack_address()));
    T_STACK_LIMIT.with(|l| l.set(stack_bytes / 8 * 7));
    T_DEADLINE.with(|d| d.set(if time_limit_ms == 0 { None } else { Some(std::time::Instant::now() + std::time::Duration::from_millis(time_limit_ms)) }));
    T_TICKS.with(|t| t.set(0));
    T_MEMORY_LEFT.with(|m| m.set(if memory_limit_bytes == 0 { -1 } else { memory_limit_bytes.min(i64::MAX as u64) as i64 }));
}
#[inline]
fn t_enter(loc: &str) {
    let base = T_STACK_BASE.with(|b| b.get());
    if base != 0 && base.abs_diff(t_stack_address()) > T_STACK_LIMIT.with(|l| l.get()) {
        t_fail("R013", "recursion is too deep: the stack is full".to_string(), loc)
    }
    t_tick(loc);
}
#[inline]
fn t_tick(loc: &str) {
    let n = T_TICKS.with(|t| { let n = t.get().wrapping_add(1); t.set(n); n });
    if n & 4095 == 0 {
        if let Some(deadline) = T_DEADLINE.with(|d| d.get()) {
            if std::time::Instant::now() > deadline { t_fail("R014", "the time limit was exceeded".to_string(), loc) }
        }
    }
}
// Counts bytes allocated by list and text operations against the budget (total per run or request).
#[inline]
fn t_charge(bytes: usize) {
    let left = T_MEMORY_LEFT.with(|m| m.get());
    if left >= 0 {
        let rest = left - bytes as i64;
        if rest < 0 { T_MEMORY_LEFT.with(|m| m.set(-1)); t_fail("R015", "the memory limit was exceeded".to_string(), "a list or text operation") }
        T_MEMORY_LEFT.with(|m| m.set(rest));
    }
}
fn t_env_number(name: &str) -> u64 { std::env::var(name).ok().and_then(|v| v.trim().parse::<u64>().ok()).unwrap_or(0) }
// TESSERA_FS_ROOT confines file access: paths are relative to it, and absolute paths, `..` and symbolic links leading outside are refused.
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

fn t_add(a: i64, b: i64, loc: &str) -> i64 {
    match a.checked_add(b) { Some(v) => v, None => t_fail("R002", format!("integer overflow: {} + {}", a, b), loc) }
}
fn t_sub(a: i64, b: i64, loc: &str) -> i64 {
    match a.checked_sub(b) { Some(v) => v, None => t_fail("R002", format!("integer overflow: {} - {}", a, b), loc) }
}
fn t_mul(a: i64, b: i64, loc: &str) -> i64 {
    match a.checked_mul(b) { Some(v) => v, None => t_fail("R002", format!("integer overflow: {} * {}", a, b), loc) }
}
fn t_div(a: i64, b: i64, loc: &str) -> i64 {
    if b == 0 { t_fail("R003", format!("division by zero: {} / 0", a), loc) }
    match a.checked_div(b) { Some(v) => v, None => t_fail("R002", format!("integer overflow: {} / {}", a, b), loc) }
}
fn t_rem(a: i64, b: i64, loc: &str) -> i64 {
    if b == 0 { t_fail("R003", format!("remainder by zero: {} % 0", a), loc) }
    match a.checked_rem(b) { Some(v) => v, None => t_fail("R002", format!("integer overflow: {} % {}", a, b), loc) }
}
fn t_neg(a: i64, loc: &str) -> i64 {
    match a.checked_neg() { Some(v) => v, None => t_fail("R002", format!("integer overflow: -({})", a), loc) }
}

fn t_text(s: &str) -> Rc<String> { Rc::new(s.to_string()) }
fn t_concat(mut a: Rc<String>, b: Rc<String>) -> Rc<String> { t_charge(b.len()); Rc::make_mut(&mut a).push_str(&b); a }
fn t_int_to_text(v: i64) -> Rc<String> { Rc::new(v.to_string()) }
fn t_bool_to_text(v: bool) -> Rc<String> { Rc::new(if v { "true".to_string() } else { "false".to_string() }) }
fn t_text_length(t: &Rc<String>) -> i64 { t.chars().count() as i64 }
fn t_code_points(t: &Rc<String>) -> Rc<Vec<i64>> { Rc::new(t.chars().map(|c| c as i64).collect()) }
fn t_from_code_point(c: i64, loc: &str) -> Rc<String> {
    match u32::try_from(c).ok().and_then(char::from_u32) {
        Some(ch) => Rc::new(ch.to_string()),
        None => t_fail("R005", format!("{} is not a valid unicode code point", c), loc),
    }
}
fn t_from_code_points(l: &Rc<Vec<i64>>, loc: &str) -> Rc<String> {
    let mut s = String::with_capacity(l.len());
    for &c in l.iter() {
        match u32::try_from(c).ok().and_then(char::from_u32) {
            Some(ch) => s.push(ch),
            None => t_fail("R005", format!("{} is not a valid unicode code point", c), loc),
        }
    }
    Rc::new(s)
}
fn t_join(parts: &Rc<Vec<Rc<String>>>, sep: &Rc<String>) -> Rc<String> {
    let mut s = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 { s.push_str(sep); }
        s.push_str(p);
    }
    Rc::new(s)
}
fn t_split(text: &Rc<String>, sep: &Rc<String>) -> Rc<Vec<Rc<String>>> {
    if sep.is_empty() { return Rc::new(text.chars().map(|c| Rc::new(c.to_string())).collect()); }
    Rc::new(text.split(sep.as_str()).map(|p| Rc::new(p.to_string())).collect())
}
fn t_to_lower(text: &Rc<String>) -> Rc<String> { Rc::new(text.to_lowercase()) }
fn t_to_upper(text: &Rc<String>) -> Rc<String> { Rc::new(text.to_uppercase()) }
fn t_parse_int(text: &Rc<String>) -> Option<i64> {
    // Only a leading `-`, matching what `to_text` produces; Rust's parser would also accept `+`.
    if text.starts_with('+') { return None; }
    text.parse::<i64>().ok()
}
fn t_text_index_of(text: &Rc<String>, part: &Rc<String>) -> Option<i64> {
    text.find(part.as_str()).map(|byte| text[..byte].chars().count() as i64)
}
fn t_text_contains(text: &Rc<String>, part: &Rc<String>) -> bool { text.contains(part.as_str()) }

fn t_list_length<T>(l: &Rc<Vec<T>>) -> i64 { l.len() as i64 }
fn t_index<T: Clone>(l: &Rc<Vec<T>>, i: i64, loc: &str) -> T {
    if i < 0 || (i as u64) >= (l.len() as u64) {
        t_fail("R001", format!("list index {} is out of bounds for a list of length {}", i, l.len()), loc)
    }
    l[i as usize].clone()
}
fn t_append<T: Clone>(mut l: Rc<Vec<T>>, x: T) -> Rc<Vec<T>> { t_charge(std::mem::size_of::<T>()); Rc::make_mut(&mut l).push(x); l }
fn t_concat_lists<T: Clone>(mut a: Rc<Vec<T>>, b: Rc<Vec<T>>) -> Rc<Vec<T>> { t_charge(b.len() * std::mem::size_of::<T>()); Rc::make_mut(&mut a).extend(b.iter().cloned()); a }
fn t_slice<T: Clone>(l: Rc<Vec<T>>, start: i64, end: i64, loc: &str) -> Rc<Vec<T>> {
    if start < 0 || end < start || (end as u64) > (l.len() as u64) {
        t_fail("R004", format!("invalid slice {}..{} of a list of length {}", start, end, l.len()), loc)
    }
    Rc::new(l[start as usize..end as usize].to_vec())
}
fn t_replace_at<T: Clone>(mut l: Rc<Vec<T>>, i: i64, x: T, loc: &str) -> Rc<Vec<T>> {
    if i < 0 || (i as u64) >= (l.len() as u64) {
        t_fail("R001", format!("list index {} is out of bounds for a list of length {}", i, l.len()), loc)
    }
    Rc::make_mut(&mut l)[i as usize] = x;
    l
}
fn t_range(start: i64, end: i64, loc: &str) -> Rc<Vec<i64>> {
    // Empty when start >= end (draft 0.20; before, start > end was R004).
    let _ = loc;
    if start >= end { return Rc::new(Vec::new()); }
    t_charge((end - start) as usize * 8);
    Rc::new((start..end).collect())
}
fn t_sort<T: Ord + Clone>(mut l: Rc<Vec<T>>) -> Rc<Vec<T>> { Rc::make_mut(&mut l).sort(); l }
fn t_index_of<T: PartialEq>(l: &Rc<Vec<T>>, x: &T) -> Option<i64> { l.iter().position(|y| y == x).map(|i| i as i64) }
fn t_contains<T: PartialEq>(l: &Rc<Vec<T>>, x: &T) -> bool { l.iter().any(|y| y == x) }
fn t_value_or<T>(option: Option<T>, default: T) -> T { match option { Some(v) => v, None => default } }
fn t_iter<T: Clone>(l: Rc<Vec<T>>) -> impl Iterator<Item = T> { (0..l.len()).map(move |i| l[i].clone()) }

fn t_print(t: Rc<String>) {
    #[cfg(target_arch = "wasm32")]
    { crate::wasm_write(1, format!("{}\n", t).as_bytes()); return; }
    let out = std::io::stdout();
    let mut lock = out.lock();
    let _ = lock.write_all(t.as_bytes());
    let _ = lock.write_all(b"\n");
}
fn t_print_error(t: Rc<String>) {
    #[cfg(target_arch = "wasm32")]
    { crate::wasm_write(2, format!("{}\n", t).as_bytes()); return; }
    let _ = std::io::stdout().flush();
    let err = std::io::stderr();
    let mut lock = err.lock();
    let _ = lock.write_all(t.as_bytes());
    let _ = lock.write_all(b"\n");
}
fn t_read_file(path: Rc<String>, loc: &str) -> Rc<String> {
    #[cfg(target_arch = "wasm32")]
    match t_wasm_read(&path) {
        Ok(bytes) => match String::from_utf8(bytes) { Ok(s) => return Rc::new(s), Err(_) => t_fail("R006", format!("cannot read file `{}`: stream did not contain valid UTF-8", path), loc) },
        Err(e) => t_fail("R006", format!("cannot read file `{}`: {}", path, e), loc),
    }
    match std::fs::read_to_string(t_check_path(&path, loc)) {
        Ok(s) => Rc::new(s),
        Err(e) => t_fail("R006", format!("cannot read file `{}`: {}", path, e), loc),
    }
}
fn t_write_file(path: Rc<String>, content: Rc<String>, loc: &str) {
    #[cfg(target_arch = "wasm32")]
    { if let Err(e) = t_wasm_write(&path, content.as_bytes()) { t_fail("R007", format!("cannot write file `{}`: {}", path, e), loc) } return; }
    if let Err(e) = std::fs::write(t_check_path(&path, loc), content.as_bytes()) {
        t_fail("R007", format!("cannot write file `{}`: {}", path, e), loc)
    }
}
fn t_args(loc: &str) -> Rc<Vec<Rc<String>>> {
    #[cfg(target_arch = "wasm32")]
    {
        let joined = crate::wasm_get("args").unwrap_or_default();
        if joined.is_empty() { return Rc::new(Vec::new()); }
        let mut out = Vec::new();
        for part in joined.split(|&b| b == 0) {
            match String::from_utf8(part.to_vec()) { Ok(s) => out.push(Rc::new(s)), Err(_) => t_fail("R008", "a command-line argument is not valid UTF-8".to_string(), loc) }
        }
        return Rc::new(out);
    }
    let mut out = Vec::new();
    for a in std::env::args_os().skip(1) {
        match a.into_string() {
            Ok(s) => out.push(Rc::new(s)),
            Err(_) => t_fail("R008", "a command-line argument is not valid UTF-8".to_string(), loc),
        }
    }
    Rc::new(out)
}
fn t_exit(code: i64, loc: &str) -> ! {
    if !(0..=255).contains(&code) { t_fail("R009", format!("exit code {} is outside 0..255", code), loc) }
    let _ = std::io::stdout().flush();
    std::panic::panic_any(TExit { code: code as i32 })
}
fn t_exit_with(code: i64, message: Rc<String>, loc: &str) -> ! {
    if !(0..=255).contains(&code) { t_fail("R009", format!("exit code {} is outside 0..255", code), loc) }
    let _ = std::io::stdout().flush();
    #[cfg(target_arch = "wasm32")]
    crate::wasm_write(2, format!("{}\n", message).as_bytes());
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("{}", message);
    std::panic::panic_any(TExit { code: code as i32 })
}
fn t_user_fail(message: Rc<String>, loc: &str) -> ! { t_fail("R010", message.to_string(), loc) }
#[cfg(target_arch = "wasm32")]
fn t_file_exists(path: Rc<String>) -> bool { crate::wasm_get(&format!("file/{}", path)).is_some() }
#[cfg(not(target_arch = "wasm32"))]
fn t_file_exists(path: Rc<String>) -> bool { t_check_path(&path, "file_exists").is_file() }
fn t_environment_variable(name: Rc<String>) -> Rc<String> {
    #[cfg(target_arch = "wasm32")]
    return Rc::new(crate::wasm_get(&format!("env/{}", name)).and_then(|v| String::from_utf8(v).ok()).unwrap_or_default());
    match std::env::var(name.as_str()) { Ok(v) => Rc::new(v), Err(_) => Rc::new(String::new()) }
}
fn t_run_process(program: Rc<String>, arguments: Rc<Vec<Rc<String>>>) -> i64 {
    let _ = std::io::stdout().flush();
    let mut command = std::process::Command::new(program.as_str());
    for a in arguments.iter() { command.arg(a.as_str()); }
    match command.status() {
        Ok(status) => match status.code() { Some(c) => c as i64, None => -1 },
        Err(_) => -1,
    }
}
fn t_rgb(red: i64, green: i64, blue: i64, loc: &str) -> i64 {
    for (name, v) in [("red", red), ("green", green), ("blue", blue)] {
        if !(0..=255).contains(&v) { t_fail("R011", format!("{} component {} is outside 0..255", name, v), loc) }
    }
    (red << 16) | (green << 8) | blue
}

// WebAssembly: the files of the host. Missing files are NotFound, refused writes PermissionDenied.
#[cfg(target_arch = "wasm32")]
fn t_wasm_read(path: &str) -> std::io::Result<Vec<u8>> {
    crate::wasm_get(&format!("file/{}", path)).ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
}
#[cfg(target_arch = "wasm32")]
fn t_wasm_write(path: &str, data: &[u8]) -> std::io::Result<()> {
    if crate::wasm_put(&format!("file/{}", path), data) { Ok(()) } else { Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)) }
}
// The stack size `tessera1 build --wasm` links with; R013 is raised before it is used up.
#[cfg(target_arch = "wasm32")]
const T_WASM_STACK: usize = 64 * 1024 * 1024;
// WebAssembly has no threads: `main` runs on the caller's stack, and a runtime error or `exit`
// ends in a trap after the panic hook has told the host the exit code.
#[cfg(target_arch = "wasm32")]
fn t_run(entry: fn()) {
    t_install_panic_hook();
    t_limits_begin(T_WASM_STACK, 0, 0);
    entry();
}
#[cfg(not(target_arch = "wasm32"))]
fn t_run(entry: fn()) {
    t_install_panic_hook();
    let stack = 512 * 1024 * 1024;
    let time_limit = t_env_number("TESSERA_TIME_LIMIT_MS");
    let memory_limit = t_env_number("TESSERA_MEMORY_LIMIT_MB").saturating_mul(1024 * 1024);
    let child = std::thread::Builder::new().stack_size(stack).spawn(move || {
        t_limits_begin(stack, time_limit, memory_limit);
        entry();
        let _ = std::io::stdout().flush();
    });
    match child {
        Ok(handle) => {
            if let Err(payload) = handle.join() {
                let _ = std::io::stdout().flush();
                if let Some(failure) = payload.downcast_ref::<TFailure>() {
                    t_report_failure(failure);
                    std::process::exit(70)
                }
                if let Some(exit) = payload.downcast_ref::<TExit>() { std::process::exit(exit.code) }
                std::process::exit(70)
            }
        }
        Err(_) => { eprintln!("runtime error R000: could not start the main thread"); std::process::exit(70) }
    }
}
