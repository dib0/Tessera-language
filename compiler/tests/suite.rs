//! End-to-end tests over the `.tes` files in `tests/` and `examples/`.
//!
//! - `tests/run/*.tes` and `examples/valid/*.tes`: must compile and run with exit code 0;
//!   stdout must equal the `.out` file next to the source.
//! - `tests/compile_fail/EXXXX-*.tes` and `examples/invalid/`: must fail with that error code.
//! - `tests/runtime_fail/RXXX-*.tes` and `examples/runtime/`: must compile, then exit with
//!   code 70 and report that runtime error code.

use std::path::{Path, PathBuf};
use std::process::Command;

use tessera::diag::SourceFile;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn tes_files(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", dir, e))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map_or(false, |x| x == "tes"))
        .collect();
    files.sort();
    files
}

fn load(path: &Path) -> Vec<SourceFile> {
    vec![SourceFile { path: path.display().to_string(), text: std::fs::read_to_string(path).unwrap() }]
}

fn expected_code(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().split('-').next().unwrap().to_string()
}

/// A directory the test programs may write in: `tests/run/semantics.tes` keeps a
/// file next to itself, and the compiler is given absolute paths anyway.
fn scratch() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("scratch");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn tessera() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tessera"));
    command.current_dir(scratch());
    command
}

fn run_program(path: &Path) -> std::process::Output {
    tessera().arg("run").arg(path).arg("--opt").arg("0").output().unwrap()
}

#[test]
fn programs_run_with_expected_output() {
    let mut files = tes_files("tests/run");
    files.extend(tes_files("examples/valid"));
    assert!(!files.is_empty());
    for path in files {
        let out = run_program(&path);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{} failed:\n{}{}", path.display(), stdout, stderr);
        let expected_path = path.with_extension("out");
        let expected = std::fs::read_to_string(&expected_path).unwrap_or_else(|_| panic!("missing {}", expected_path.display()));
        assert_eq!(stdout, expected, "unexpected output from {}", path.display());
    }
}

#[test]
fn invalid_programs_report_the_expected_error() {
    let mut files = tes_files("tests/compile_fail");
    files.extend(tes_files("examples/invalid"));
    assert!(!files.is_empty());
    for path in files {
        let code = expected_code(&path);
        match tessera::compile_to_rust(&load(&path)) {
            Ok(_) => panic!("{} compiled, expected {}", path.display(), code),
            Err(diags) => {
                let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
                assert!(codes.contains(&code.as_str()), "{}: expected {}, got {:?}", path.display(), code, codes);
                for d in &diags {
                    assert!(d.span.line >= 1 && d.span.col >= 1, "{}: diagnostic without location: {:?}", path.display(), d);
                }
            }
        }
    }
}

#[test]
fn runtime_errors_stop_the_program_predictably() {
    let mut files = tes_files("tests/runtime_fail");
    files.extend(tes_files("examples/runtime"));
    for path in files {
        let code = expected_code(&path);
        let out = run_program(&path);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(70), "{}: expected exit code 70\n{}", path.display(), stderr);
        assert!(stderr.contains(&format!("runtime error {}", code)), "{}: expected {} in:\n{}", path.display(), code, stderr);
        assert!(stderr.contains(" in function `"), "{}: missing location in:\n{}", path.display(), stderr);
    }
}

#[test]
fn compiler_cli_exit_codes() {
    let bad = root().join("tests/compile_fail/E0202-unknown-name.tes");
    assert_eq!(tessera().arg("check").arg(&bad).output().unwrap().status.code(), Some(1));
    let good = root().join("examples/valid/receipt.tes");
    assert_eq!(tessera().arg("check").arg(&good).output().unwrap().status.code(), Some(0));
    assert_eq!(tessera().arg("check").output().unwrap().status.code(), Some(2));
    let json = tessera().arg("check").arg("--json").arg(&bad).output().unwrap();
    let text = String::from_utf8_lossy(&json.stdout);
    assert!(text.starts_with("{\"diagnostics\":[") && text.contains("\"code\":\"E0202\""), "{}", text);
}

#[test]
fn generated_rust_is_deterministic_and_forbids_unsafe() {
    let mut files = tes_files("tests/run");
    files.extend(tes_files("examples/valid"));
    for path in files {
        let a = tessera::compile_to_rust(&load(&path)).unwrap();
        let b = tessera::compile_to_rust(&load(&path)).unwrap();
        assert_eq!(a, b, "{}: output differs between builds", path.display());
        assert!(a.contains("#![forbid(unsafe_code)]"), "{}: missing forbid(unsafe_code)", path.display());
        for (i, line) in a.lines().enumerate() {
            let code = line.split("//").next().unwrap();
            let without_strings: String = code.split('"').step_by(2).collect();
            assert!(!without_strings.contains("unsafe ") && !without_strings.contains("unsafe{"), "{}: unsafe on generated line {}: {}", path.display(), i + 1, line);
        }
    }
}

#[test]
fn notes_are_exported() {
    let path = root().join("examples/valid/calculator.tes");
    let out = tessera().arg("notes").arg(&path).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(text.contains("\"item\": \"tokenize\""), "{}", text);
    assert!(text.contains("the final End token removes bounds checks"), "{}", text);
}

/// Small deterministic PRNG so the fuzz test is reproducible without dependencies.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

#[test]
fn fuzz_front_end_never_panics() {
    let mut corpus: Vec<String> = Vec::new();
    for dir in ["tests/run", "tests/compile_fail", "tests/runtime_fail", "examples/valid", "examples/invalid"] {
        for p in tes_files(dir) {
            corpus.push(std::fs::read_to_string(p).unwrap());
        }
    }
    let fragments = [
        "{", "}", "(", ")", "[", "]", "\"", "\n", " ", "match", "=>", "->", "fn", "note", "if", "else", "-", "9223372036854775808", "_", ".", ",",
        "\\u{", "é", "\t", "set", "return", "Type.", "and", "or", "not", "==", "<",
    ];
    let mut rng = Rng(0x5eed_1234_abcd_ef01);
    let start = std::time::Instant::now();
    let mut cases = 0;
    for round in 0..3000 {
        let base = &corpus[round % corpus.len()];
        let mut chars: Vec<char> = base.chars().collect();
        for _ in 0..1 + rng.below(6) {
            match rng.below(4) {
                0 if !chars.is_empty() => {
                    let i = rng.below(chars.len());
                    chars.remove(i);
                }
                1 => {
                    let i = rng.below(chars.len() + 1);
                    let frag = fragments[rng.below(fragments.len())];
                    for (k, c) in frag.chars().enumerate() {
                        chars.insert(i + k, c);
                    }
                }
                2 if chars.len() > 2 => {
                    let a = rng.below(chars.len());
                    let b = rng.below(chars.len());
                    chars.swap(a, b);
                }
                _ if !chars.is_empty() => {
                    let a = rng.below(chars.len());
                    let len = rng.below(20).min(chars.len() - a);
                    chars.drain(a..a + len);
                }
                _ => {}
            }
        }
        let text: String = chars.into_iter().collect();
        let files = vec![SourceFile { path: "fuzz.tes".into(), text }];
        let result = std::panic::catch_unwind(|| {
            let _ = tessera::compile_to_rust(&files);
        });
        assert!(result.is_ok(), "front end panicked on input:\n{}", files[0].text);
        cases += 1;
    }
    // Pathological nesting must be rejected, not overflow the stack.
    let deep = format!("fn main() -> Unit uses io = print(to_text({}1{}))", "(".repeat(5000), ")".repeat(5000));
    let files = vec![SourceFile { path: "deep.tes".into(), text: deep }];
    let diags = tessera::compile_to_rust(&files).unwrap_err();
    assert!(diags.iter().any(|d| d.code == "E0107"));
    assert!(start.elapsed().as_secs() < 60, "fuzzing {} cases took too long (possible hang)", cases);
}

#[test]
fn multi_file_program_shares_one_namespace() {
    let dir = root().join("tests/multi_file");
    let out = tessera().arg("run").arg("--opt").arg("0").arg(dir.join("trees.tes")).arg(dir.join("main.tes")).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let expected = std::fs::read_to_string(dir.join("expected.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
}
