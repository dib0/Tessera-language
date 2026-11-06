#![forbid(unsafe_code)]
#![allow(unused, dead_code, non_snake_case, non_camel_case_types, unreachable_code, unreachable_patterns, unused_parens, unused_braces, redundant_semicolons, while_true, clippy::all)]

use std::io::Write;
use std::rc::Rc;

fn t_fail(code: &str, message: String, location: &str) -> ! {
    let _ = std::io::stdout().flush();
    eprintln!("runtime error {}: {}\n  at {}", code, message, location);
    std::process::exit(70)
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
fn t_concat(mut a: Rc<String>, b: Rc<String>) -> Rc<String> { Rc::make_mut(&mut a).push_str(&b); a }
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

fn t_list_length<T>(l: &Rc<Vec<T>>) -> i64 { l.len() as i64 }
fn t_index<T: Clone>(l: &Rc<Vec<T>>, i: i64, loc: &str) -> T {
    if i < 0 || (i as u64) >= (l.len() as u64) {
        t_fail("R001", format!("list index {} is out of bounds for a list of length {}", i, l.len()), loc)
    }
    l[i as usize].clone()
}
fn t_append<T: Clone>(mut l: Rc<Vec<T>>, x: T) -> Rc<Vec<T>> { Rc::make_mut(&mut l).push(x); l }
fn t_concat_lists<T: Clone>(mut a: Rc<Vec<T>>, b: Rc<Vec<T>>) -> Rc<Vec<T>> { Rc::make_mut(&mut a).extend(b.iter().cloned()); a }
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
    if start > end { t_fail("R004", format!("invalid range({}, {}): start is greater than end", start, end), loc) }
    Rc::new((start..end).collect())
}
fn t_value_or<T>(option: Option<T>, default: T) -> T { match option { Some(v) => v, None => default } }
fn t_iter<T: Clone>(l: Rc<Vec<T>>) -> impl Iterator<Item = T> { (0..l.len()).map(move |i| l[i].clone()) }

fn t_print(t: Rc<String>) {
    let out = std::io::stdout();
    let mut lock = out.lock();
    let _ = lock.write_all(t.as_bytes());
    let _ = lock.write_all(b"\n");
}
fn t_print_error(t: Rc<String>) {
    let _ = std::io::stdout().flush();
    let err = std::io::stderr();
    let mut lock = err.lock();
    let _ = lock.write_all(t.as_bytes());
    let _ = lock.write_all(b"\n");
}
fn t_read_file(path: Rc<String>, loc: &str) -> Rc<String> {
    match std::fs::read_to_string(path.as_str()) {
        Ok(s) => Rc::new(s),
        Err(e) => t_fail("R006", format!("cannot read file `{}`: {}", path, e), loc),
    }
}
fn t_write_file(path: Rc<String>, content: Rc<String>, loc: &str) {
    if let Err(e) = std::fs::write(path.as_str(), content.as_bytes()) {
        t_fail("R007", format!("cannot write file `{}`: {}", path, e), loc)
    }
}
fn t_args(loc: &str) -> Rc<Vec<Rc<String>>> {
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
    std::process::exit(code as i32)
}
fn t_user_fail(message: Rc<String>, loc: &str) -> ! { t_fail("R010", message.to_string(), loc) }
fn t_file_exists(path: Rc<String>) -> bool { std::path::Path::new(path.as_str()).is_file() }
fn t_environment_variable(name: Rc<String>) -> Rc<String> {
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

fn t_run(entry: fn()) {
    let child = std::thread::Builder::new().stack_size(512 * 1024 * 1024).spawn(move || {
        entry();
        let _ = std::io::stdout().flush();
    });
    match child {
        Ok(handle) => {
            if handle.join().is_err() { std::process::exit(70) }
        }
        Err(_) => { eprintln!("runtime error R000: could not start the main thread"); std::process::exit(70) }
    }
}
