
// ---- Stage 1 runtime additions (draft 0.13). Not used by stage 0; may refer to types the stage 1
// code generator always emits (`T_FileError`). Safe Rust only, like the rest of the runtime.

// Text
fn t_trim(t: &Rc<String>) -> Rc<String> { Rc::new(t.trim().to_string()) }
fn t_starts_with(t: &Rc<String>, part: &Rc<String>) -> bool { t.starts_with(part.as_str()) }
fn t_ends_with(t: &Rc<String>, part: &Rc<String>) -> bool { t.ends_with(part.as_str()) }
fn t_replace(t: &Rc<String>, old: &Rc<String>, new: &Rc<String>) -> Rc<String> {
    if old.is_empty() { return t.clone(); }
    let out = t.replace(old.as_str(), new.as_str());
    t_charge(out.len());
    Rc::new(out)
}
fn t_slice_text(t: Rc<String>, start: i64, end: i64, loc: &str) -> Rc<String> {
    let count = t.chars().count() as i64;
    if start < 0 || end < start || end > count {
        t_fail("R004", format!("invalid slice_text {}..{} of a text of length {}", start, end, count), loc)
    }
    Rc::new(t.chars().skip(start as usize).take((end - start) as usize).collect())
}
fn t_repeat(t: Rc<String>, times: i64, loc: &str) -> Rc<String> {
    if times < 0 { t_fail("R004", format!("repeat count {} is negative", times), loc) }
    t_charge(t.len().saturating_mul(times as usize));
    Rc::new(t.repeat(times as usize))
}
fn t_pad(t: &Rc<String>, width: i64, fill: &Rc<String>, left: bool, loc: &str) -> Rc<String> {
    let mut chars = fill.chars();
    let fill_char = match (chars.next(), chars.next()) {
        (Some(c), None) => c,
        _ => t_fail("R004", format!("the fill text must be exactly one character, found {:?}", fill.as_str()), loc),
    };
    let count = t.chars().count() as i64;
    if count >= width { return t.clone(); }
    let padding: String = std::iter::repeat(fill_char).take((width - count) as usize).collect();
    t_charge(padding.len());
    Rc::new(if left { padding + t.as_str() } else { t.to_string() + &padding })
}
fn t_pad_left(t: Rc<String>, width: i64, fill: Rc<String>, loc: &str) -> Rc<String> { t_pad(&t, width, &fill, true, loc) }
fn t_pad_right(t: Rc<String>, width: i64, fill: Rc<String>, loc: &str) -> Rc<String> { t_pad(&t, width, &fill, false, loc) }
fn t_lines(t: &Rc<String>) -> Rc<Vec<Rc<String>>> {
    Rc::new(t.lines().map(|line| Rc::new(line.to_string())).collect())
}

// Standard input
fn t_read_line(loc: &str) -> Option<Rc<String>> {
    let _ = std::io::stdout().flush();
    let mut buffer = Vec::new();
    let mut input = std::io::stdin().lock();
    match std::io::BufRead::read_until(&mut input, b'\n', &mut buffer) {
        Ok(0) => None,
        Ok(_) => {
            if buffer.last() == Some(&b'\n') { buffer.pop(); }
            if buffer.last() == Some(&b'\r') { buffer.pop(); }
            match String::from_utf8(buffer) {
                Ok(s) => Some(Rc::new(s)),
                Err(_) => t_fail("R008", "standard input is not valid UTF-8".to_string(), loc),
            }
        }
        Err(e) => t_fail("R006", format!("cannot read standard input: {}", e), loc),
    }
}
fn t_read_input(loc: &str) -> Rc<String> {
    let _ = std::io::stdout().flush();
    let mut buffer = Vec::new();
    if let Err(e) = std::io::Read::read_to_end(&mut std::io::stdin().lock(), &mut buffer) {
        t_fail("R006", format!("cannot read standard input: {}", e), loc)
    }
    t_charge(buffer.len());
    match String::from_utf8(buffer) {
        Ok(s) => Rc::new(s),
        Err(_) => t_fail("R008", "standard input is not valid UTF-8".to_string(), loc),
    }
}

// Files, with failures as values
fn t_file_error(e: std::io::Error) -> Rc<T_FileError> {
    Rc::new(match e.kind() {
        std::io::ErrorKind::NotFound => T_FileError::V_NotFound,
        std::io::ErrorKind::PermissionDenied => T_FileError::V_PermissionDenied,
        _ => T_FileError::V_Other(Rc::new(e.to_string())),
    })
}
fn t_read_text(path: Rc<String>, loc: &str) -> Result<Rc<String>, Rc<T_FileError>> {
    match std::fs::read(t_check_path(&path, loc)) {
        Ok(bytes) => {
            t_charge(bytes.len());
            match String::from_utf8(bytes) {
                Ok(s) => Ok(Rc::new(s)),
                Err(_) => Err(Rc::new(T_FileError::V_Other(Rc::new(format!("`{}` is not valid UTF-8 text", path))))),
            }
        }
        Err(e) => Err(t_file_error(e)),
    }
}
fn t_write_text(path: Rc<String>, content: Rc<String>, loc: &str) -> Result<(), Rc<T_FileError>> {
    std::fs::write(t_check_path(&path, loc), content.as_bytes()).map_err(t_file_error)
}
fn t_read_bytes(path: Rc<String>, loc: &str) -> Result<Rc<Vec<u8>>, Rc<T_FileError>> {
    match std::fs::read(t_check_path(&path, loc)) {
        Ok(bytes) => { t_charge(bytes.len()); Ok(Rc::new(bytes)) }
        Err(e) => Err(t_file_error(e)),
    }
}
fn t_write_bytes(path: Rc<String>, data: Rc<Vec<u8>>, loc: &str) -> Result<(), Rc<T_FileError>> {
    std::fs::write(t_check_path(&path, loc), data.as_slice()).map_err(t_file_error)
}
fn t_list_directory(path: Rc<String>, loc: &str) -> Result<Rc<Vec<Rc<String>>>, Rc<T_FileError>> {
    let entries = std::fs::read_dir(t_check_path(&path, loc)).map_err(t_file_error)?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry.map_err(t_file_error)?;
        names.push(Rc::new(entry.file_name().to_string_lossy().to_string()));
    }
    names.sort();
    Ok(Rc::new(names))
}
fn t_create_directory(path: Rc<String>, loc: &str) -> Result<(), Rc<T_FileError>> {
    std::fs::create_dir_all(t_check_path(&path, loc)).map_err(t_file_error)
}
fn t_remove_file(path: Rc<String>, loc: &str) -> Result<(), Rc<T_FileError>> {
    std::fs::remove_file(t_check_path(&path, loc)).map_err(t_file_error)
}

// Logging: standard error, which hosts route to their error log.
fn t_log(level: Rc<String>, message: Rc<String>) {
    let _ = std::io::stdout().flush();
    eprintln!("[{}] {}", level, message);
}

// Maps (sorted by key, so output does not depend on hashing)
type TMap<K, V> = Rc<std::collections::BTreeMap<K, V>>;
fn t_map_new<K: Ord, V>(pairs: Vec<(K, V)>) -> TMap<K, V> { Rc::new(pairs.into_iter().collect()) }
fn t_map_get<K: Ord, V: Clone>(m: &TMap<K, V>, key: &K) -> Option<V> { m.get(key).cloned() }
fn t_map_put<K: Ord + Clone, V: Clone>(mut m: TMap<K, V>, key: K, value: V) -> TMap<K, V> {
    t_charge(std::mem::size_of::<K>() + std::mem::size_of::<V>());
    Rc::make_mut(&mut m).insert(key, value);
    m
}
fn t_map_remove<K: Ord + Clone, V: Clone>(mut m: TMap<K, V>, key: &K) -> TMap<K, V> {
    if m.contains_key(key) { Rc::make_mut(&mut m).remove(key); }
    m
}
fn t_map_keys<K: Ord + Clone, V>(m: &TMap<K, V>) -> Rc<Vec<K>> { Rc::new(m.keys().cloned().collect()) }
fn t_map_values<K: Ord, V: Clone>(m: &TMap<K, V>) -> Rc<Vec<V>> { Rc::new(m.values().cloned().collect()) }
fn t_map_length<K, V>(m: &TMap<K, V>) -> i64 { m.len() as i64 }
fn t_map_contains<K: Ord, V>(m: &TMap<K, V>, key: &K) -> bool { m.contains_key(key) }

// Bytes
fn t_to_bytes(t: &Rc<String>) -> Rc<Vec<u8>> { Rc::new(t.as_bytes().to_vec()) }
fn t_from_bytes(b: &Rc<Vec<u8>>) -> Option<Rc<String>> { String::from_utf8(b.to_vec()).ok().map(Rc::new) }
fn t_bytes(values: Rc<Vec<i64>>, loc: &str) -> Rc<Vec<u8>> {
    let mut out = Vec::with_capacity(values.len());
    for &v in values.iter() {
        if !(0..=255).contains(&v) { t_fail("R018", format!("byte value {} is outside 0..255", v), loc) }
        out.push(v as u8);
    }
    Rc::new(out)
}
fn t_byte_values(b: &Rc<Vec<u8>>) -> Rc<Vec<i64>> { Rc::new(b.iter().map(|&x| x as i64).collect()) }
fn t_bytes_index(b: &Rc<Vec<u8>>, i: i64, loc: &str) -> i64 {
    if i < 0 || (i as u64) >= (b.len() as u64) {
        t_fail("R001", format!("index {} is out of bounds for bytes of length {}", i, b.len()), loc)
    }
    b[i as usize] as i64
}

// Changing part of a variable (draft 0.14): `set xs[i] = v`, `set r.field = v`. `Rc::make_mut` copies
// a list or record only when another value still shares it, so the change is never observable elsewhere.
fn t_slot<'a, T: Clone>(l: &'a mut Rc<Vec<T>>, i: i64, loc: &str) -> &'a mut T {
    if i < 0 || (i as u64) >= (l.len() as u64) {
        t_fail("R001", format!("list index {} is out of bounds for a list of length {}", i, l.len()), loc)
    }
    if Rc::strong_count(l) > 1 { t_charge(l.len() * std::mem::size_of::<T>()); }
    &mut Rc::make_mut(l)[i as usize]
}
fn t_set_byte(b: &mut Rc<Vec<u8>>, i: i64, v: i64, loc: &str) {
    if i < 0 || (i as u64) >= (b.len() as u64) {
        t_fail("R001", format!("index {} is out of bounds for bytes of length {}", i, b.len()), loc)
    }
    if !(0..=255).contains(&v) { t_fail("R018", format!("byte value {} is outside 0..255", v), loc) }
    if Rc::strong_count(b) > 1 { t_charge(b.len()); }
    Rc::make_mut(b)[i as usize] = v as u8;
}
fn t_repeat_list<T: Clone>(l: Rc<Vec<T>>, times: i64, loc: &str) -> Rc<Vec<T>> {
    if times < 0 { t_fail("R004", format!("repeat count {} is negative", times), loc) }
    let total = l.len().saturating_mul(times as usize);
    t_charge(total.saturating_mul(std::mem::size_of::<T>()));
    let mut out = Vec::with_capacity(total);
    for _ in 0..times { out.extend(l.iter().cloned()); }
    Rc::new(out)
}

// Float (draft 0.17): IEEE 754 binary64 that is always finite. A result that would be infinite is
// R002 (overflow), division by zero R003, and one that is not a number R021, so NaN and infinity
// never reach a program.
fn t_float_finite(r: f64, what: &str, loc: &str) -> f64 {
    if r.is_nan() { t_fail("R021", format!("{} has no real result", what), loc) }
    if r.is_infinite() { t_fail("R002", format!("float overflow: {}", what), loc) }
    r
}
fn t_float_add(a: f64, b: f64, loc: &str) -> f64 { t_float_finite(a + b, &format!("{:?} + {:?}", a, b), loc) }
fn t_float_sub(a: f64, b: f64, loc: &str) -> f64 { t_float_finite(a - b, &format!("{:?} - {:?}", a, b), loc) }
fn t_float_mul(a: f64, b: f64, loc: &str) -> f64 { t_float_finite(a * b, &format!("{:?} * {:?}", a, b), loc) }
fn t_float_div(a: f64, b: f64, loc: &str) -> f64 {
    if b == 0.0 { t_fail("R003", format!("division by zero: {:?} / {:?}", a, b), loc) }
    t_float_finite(a / b, &format!("{:?} / {:?}", a, b), loc)
}
fn t_float_sqrt(a: f64, loc: &str) -> f64 {
    if a < 0.0 { t_fail("R021", format!("sqrt({:?}) has no real result", a), loc) }
    a.sqrt()
}
fn t_float_ln(a: f64, loc: &str) -> f64 {
    if a <= 0.0 { t_fail("R021", format!("ln({:?}) has no real result", a), loc) }
    a.ln()
}
fn t_float_exp(a: f64, loc: &str) -> f64 { t_float_finite(a.exp(), &format!("exp({:?})", a), loc) }
fn t_float_tan(a: f64, loc: &str) -> f64 { t_float_finite(a.tan(), &format!("tan({:?})", a), loc) }
fn t_float_pow(a: f64, b: f64, loc: &str) -> f64 { t_float_finite(a.powf(b), &format!("pow({:?}, {:?})", a, b), loc) }
fn t_float_to_int(a: f64, loc: &str) -> i64 {
    let t = a.trunc();
    if !(t >= -9223372036854775808.0 && t < 9223372036854775808.0) { t_fail("R002", format!("{:?} does not fit in Int", a), loc) }
    t as i64
}
fn t_float_round(a: f64, places: i64, loc: &str) -> f64 {
    if !(0..=15).contains(&places) { t_fail("R004", format!("round: places {} is outside 0..15", places), loc) }
    // Round the exact decimal expansion of `a` (at most 1074 decimals), half away from zero, then
    // take the nearest Float: 2.675 is stored as 2.67499999..., so it rounds to 2.67.
    let exact = format!("{:.1100}", a.abs());
    let point = exact.find('.').unwrap();
    let keep = point + 1 + places as usize;
    let mut digits: Vec<u8> = exact.as_bytes()[..keep].to_vec();
    if exact.as_bytes()[keep] >= b'5' {
        let mut i = digits.len();
        loop {
            if i == 0 { digits.insert(0, b'1'); break; }
            i -= 1;
            if digits[i] == b'.' { continue; }
            if digits[i] == b'9' { digits[i] = b'0'; } else { digits[i] += 1; break; }
        }
    }
    let magnitude: f64 = String::from_utf8(digits).unwrap().trim_end_matches('.').parse().unwrap();
    t_float_finite(magnitude.copysign(a), &format!("round({:?}, {})", a, places), loc)
}
fn t_float_to_text(a: f64) -> Rc<String> { Rc::new(format!("{:?}", a)) }
fn t_parse_float(t: &Rc<String>) -> Option<f64> {
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && b[i] == b'-' { i += 1; }
    let digits = |i: &mut usize| { let s = *i; while *i < b.len() && b[*i].is_ascii_digit() { *i += 1; } *i > s };
    if !digits(&mut i) { return None; }
    if i < b.len() && b[i] == b'.' { i += 1; if !digits(&mut i) { return None; } }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') { i += 1; }
        if !digits(&mut i) { return None; }
    }
    if i != b.len() { return None; }
    match t.parse::<f64>() { Ok(v) if v.is_finite() => Some(v), _ => None }
}
fn t_sort_floats(mut l: Rc<Vec<f64>>) -> Rc<Vec<f64>> { Rc::make_mut(&mut l).sort_by(|a, b| a.total_cmp(b)); l }
fn t_sort_by_float<A: Clone>(mut l: Rc<Vec<A>>, f: fn(A) -> f64) -> Rc<Vec<A>> {
    Rc::make_mut(&mut l).sort_by(|x, y| f(x.clone()).total_cmp(&f(y.clone())));
    l
}

// Sound (draft 0.19). A mixer thread streams 16-bit stereo samples at 22,050 per second to an
// audio player that reads them from standard input (`aplay`, `pw-cat`, or TESSERA_AUDIO_PLAYER).
// Without a player, sounds are dropped silently. Safe Rust: std threads, a mutex and a pipe.
struct TVoice { samples: Vec<u8>, position: u64, step: u64, left: i32, right: i32 }
struct TMixer { voices: Vec<TVoice>, running: bool }
static T_MIXER: std::sync::OnceLock<Option<std::sync::Arc<std::sync::Mutex<TMixer>>>> = std::sync::OnceLock::new();

fn t_start_audio() -> Option<std::sync::Arc<std::sync::Mutex<TMixer>>> {
    use std::process::{Command, Stdio};
    let mut candidates: Vec<Vec<String>> = Vec::new();
    if let Ok(custom) = std::env::var("TESSERA_AUDIO_PLAYER") {
        if !custom.trim().is_empty() { candidates.push(custom.split_whitespace().map(|s| s.to_string()).collect()); }
    }
    candidates.push(["aplay", "-q", "-t", "raw", "-f", "S16_LE", "-r", "22050", "-c", "2", "--buffer-time=60000", "-"].iter().map(|s| s.to_string()).collect());
    candidates.push(["pw-cat", "--playback", "--format=s16", "--rate=22050", "--channels=2", "--latency=40ms", "-"].iter().map(|s| s.to_string()).collect());
    for argv in candidates {
        let child = Command::new(&argv[0]).args(&argv[1..]).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
        if let Ok(mut child) = child {
            let mut input = match child.stdin.take() { Some(i) => i, None => continue };
            let mixer = std::sync::Arc::new(std::sync::Mutex::new(TMixer { voices: Vec::new(), running: true }));
            let shared = mixer.clone();
            std::thread::spawn(move || {
                use std::io::Write;
                let mut buffer = vec![0u8; 256 * 4];
                loop {
                    {
                        let mut m = match shared.lock() { Ok(m) => m, Err(_) => return };
                        if !m.running { return; }
                        for frame in 0..256 {
                            let (mut l, mut r) = (0i32, 0i32);
                            for v in m.voices.iter_mut() {
                                let index = (v.position >> 16) as usize;
                                if index < v.samples.len() {
                                    let s = (v.samples[index] as i32 - 128) * 256;
                                    l += s * v.left / 100;
                                    r += s * v.right / 100;
                                    v.position += v.step;
                                }
                            }
                            m.voices.retain(|v| ((v.position >> 16) as usize) < v.samples.len());
                            let l = l.clamp(-32768, 32767) as i16;
                            let r = r.clamp(-32768, 32767) as i16;
                            buffer[frame * 4..frame * 4 + 2].copy_from_slice(&l.to_le_bytes());
                            buffer[frame * 4 + 2..frame * 4 + 4].copy_from_slice(&r.to_le_bytes());
                        }
                    }
                    // Writing blocks while the player's buffer is full, which paces the loop.
                    if input.write_all(&buffer).is_err() {
                        if let Ok(mut m) = shared.lock() { m.running = false; }
                        return;
                    }
                }
            });
            return Some(mixer);
        }
    }
    None
}

fn t_play_sound(samples: &Rc<Vec<u8>>, rate: i64, volume: i64, pan: i64, loc: &str) {
    if !(1000..=96000).contains(&rate) || !(0..=100).contains(&volume) || !(-100..=100).contains(&pan) {
        t_fail("R004", format!("play_sound: rate {} (1000..96000), volume {} (0..100) or pan {} (-100..100) out of range", rate, volume, pan), loc)
    }
    let mixer = match T_MIXER.get_or_init(t_start_audio) { Some(m) => m, None => return };
    if let Ok(mut m) = mixer.lock() {
        if !m.running || samples.is_empty() || volume == 0 { return; }
        if m.voices.len() >= 16 { m.voices.remove(0); }
        // Equal-power-ish panning: the far side fades to a third at full pan.
        let left = (volume * (300 - (pan.max(0) * 2)) / 300) as i32;
        let right = (volume * (300 + (pan.min(0) * 2)) / 300) as i32;
        let step = ((rate as u64) << 16) / 22050;
        m.voices.push(TVoice { samples: samples.to_vec(), position: 0, step, left, right });
    }
}

// Bit operations (draft 0.14). Shifts never lose bits silently: `shift_left` is R002 when the
// result does not fit, like `*`; `shift_right` is arithmetic (rounds toward negative infinity).
fn t_shift_amount(n: i64, loc: &str) -> u32 {
    if !(0..=63).contains(&n) { t_fail("R020", format!("shift amount {} is outside 0..63", n), loc) }
    n as u32
}
fn t_shift_left(a: i64, n: i64, loc: &str) -> i64 {
    let n = t_shift_amount(n, loc);
    let r = a << n;
    if (r >> n) != a { t_fail("R002", format!("shift_left({}, {}) does not fit in Int", a, n), loc) }
    r
}
fn t_shift_right(a: i64, n: i64, loc: &str) -> i64 { a >> t_shift_amount(n, loc) }

// Functions as values
fn t_map_list<A: Clone, B>(l: Rc<Vec<A>>, f: fn(A) -> B) -> Rc<Vec<B>> {
    t_charge(l.len() * std::mem::size_of::<B>());
    Rc::new(l.iter().cloned().map(f).collect())
}
fn t_filter<A: Clone>(l: Rc<Vec<A>>, f: fn(A) -> bool) -> Rc<Vec<A>> {
    Rc::new(l.iter().cloned().filter(|x| f(x.clone())).collect())
}
fn t_sort_by<A: Clone, K: Ord>(mut l: Rc<Vec<A>>, f: fn(A) -> K) -> Rc<Vec<A>> {
    Rc::make_mut(&mut l).sort_by_key(|x| f(x.clone()));
    l
}

// Tests
fn t_assert(condition: bool, loc: &str) {
    if !condition { t_fail("R019", "assertion failed".to_string(), loc) }
}
fn t_run_tests(tests: &[(&str, fn())]) {
    t_install_panic_hook();
    let mut failed = 0;
    for (name, test) in tests.iter() {
        let test = *test;
        let outcome = std::thread::Builder::new().stack_size(64 * 1024 * 1024).spawn(move || {
            t_limits_begin(64 * 1024 * 1024, 10_000, 0);
            test();
        });
        let result = match outcome { Ok(handle) => handle.join(), Err(_) => { eprintln!("runtime error R000: could not start a test thread"); std::process::exit(70) } };
        match result {
            Ok(()) => println!("ok      {}", name),
            Err(payload) => {
                failed += 1;
                if let Some(f) = payload.downcast_ref::<TFailure>() {
                    println!("FAILED  {}\n        runtime error {}: {}\n        at {}", name, f.code, f.message, f.location);
                } else if payload.downcast_ref::<TExit>().is_some() {
                    println!("FAILED  {}\n        the test called exit", name);
                } else {
                    println!("FAILED  {}", name);
                }
            }
        }
    }
    println!("{} passed, {} failed", tests.len() - failed, failed);
    let _ = std::io::stdout().flush();
    std::process::exit(if failed == 0 { 0 } else { 1 });
}
