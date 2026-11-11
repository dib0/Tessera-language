//! Tests for the Tessera-written compilers.
//!
//! Build chain: rustc builds the generated seed `bootstrap/tessera1.rs`, which builds
//! `stage1/`. Stage 0 (this crate) is the independent reference. Stage 1 must:
//! - produce byte-identical Rust to stage 0 for every program without modules or `ui`,
//! - report the same error code as stage 0 for every invalid program,
//! - reproduce itself (self-hosting fixpoint),
//! - implement modules (run and compile-fail tests in `tests/modules/`),
//! - compile the graphical tic-tac-toe example.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn stage0() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tessera"))
}

fn work_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("stage1");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn stage1_entry() -> PathBuf {
    root().join("stage1/main.tes")
}

fn rustc() -> PathBuf {
    if let Some(p) = std::env::var_os("TESSERA_RUSTC") {
        return PathBuf::from(p);
    }
    if Command::new("rustc").arg("--version").output().is_ok() {
        return PathBuf::from("rustc");
    }
    Path::new(&std::env::var_os("HOME").unwrap_or_default()).join(".cargo/bin/rustc")
}

fn check_build(output: std::process::Output, what: &str) {
    assert!(output.status.success(), "{} failed:\n{}", what, String::from_utf8_lossy(&output.stderr));
}

/// seed (bootstrap/tessera1.rs) -> bootstrap compiler -> stage 1, once per test run.
fn stage1() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let bootstrap = work_dir().join("tessera-bootstrap");
        check_build(
            Command::new(rustc())
                .args(["--edition", "2021", "-C", "opt-level=1", "--crate-name", "tessera_program", "-o"])
                .arg(&bootstrap)
                .arg(root().join("bootstrap/tessera1.rs"))
                .output()
                .unwrap(),
            "rustc building the bootstrap seed",
        );
        let out = work_dir().join("tessera1");
        check_build(
            Command::new(&bootstrap).arg("build").arg(stage1_entry()).arg("-o").arg(&out).arg("--opt").arg("1").output().unwrap(),
            "bootstrap building stage1/",
        );
        out
    })
}

fn tes_files(dir: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().map_or(false, |x| x == "tes"))
        .collect();
    files.sort();
    files
}

fn subdirectories(dir: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root().join(dir)).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs
}

fn emit(compiler: &Path, files: &[PathBuf], out: &Path) -> std::process::Output {
    Command::new(compiler).arg("emit-rust").args(files).arg("-o").arg(out).output().unwrap()
}

#[test]
fn stage0_and_stage1_programs_behave_identically() {
    // Stage 1 inserts stack and time checks (draft 0.13), so its Rust is no longer byte-identical to
    // that of the frozen stage 0; the programs must still behave the same: output, errors and exit code.
    let dir = work_dir();
    let mut programs: Vec<Vec<PathBuf>> = Vec::new();
    for d in ["tests/run", "tests/runtime_fail", "examples/valid", "examples/runtime"] {
        for f in tes_files(d) {
            programs.push(vec![f]);
        }
    }
    // Several files on the command line form one entry module, as in stage 0.
    programs.push(vec![root().join("tests/multi_file/trees.tes"), root().join("tests/multi_file/main.tes")]);
    let stage0_binary = PathBuf::from(env!("CARGO_BIN_EXE_tessera"));
    for files in programs {
        let mut results = Vec::new();
        for (name, compiler) in [("stage0", &stage0_binary), ("stage1", stage1())] {
            let exe = dir.join(format!("parity_{}", name));
            let build = Command::new(compiler).arg("build").args(&files).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap();
            check_build(build, &format!("{} build of {:?}", name, files));
            let run = Command::new(&exe).output().unwrap();
            results.push((run.status.code(), run.stdout, run.stderr));
        }
        assert!(results[0] == results[1], "{:?}: stage 0 and stage 1 programs behave differently", files);
    }
}

#[test]
fn stage1_reports_the_same_error_codes() {
    let mut files = tes_files("tests/compile_fail");
    files.extend(tes_files("examples/invalid"));
    for path in files {
        let code = path.file_name().unwrap().to_string_lossy().split('-').next().unwrap().to_string();
        let out = Command::new(stage1()).arg("check").arg("--json").arg(&path).output().unwrap();
        let json = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(1), "{}: expected exit code 1", path.display());
        assert!(json.contains(&format!("\"code\":\"{}\"", code)), "{}: expected {} in {}", path.display(), code, json);
    }
}

#[test]
fn stage1_is_self_hosting() {
    let dir = work_dir();
    let stage2 = dir.join("tessera2");
    check_build(Command::new(stage1()).arg("build").arg(stage1_entry()).arg("-o").arg(&stage2).arg("--opt").arg("1").output().unwrap(), "stage 1 building itself");
    let by_stage1 = dir.join("stage1_by_stage1.rs");
    let by_stage2 = dir.join("stage1_by_stage2.rs");
    check_build(emit(stage1(), &[stage1_entry()], &by_stage1), "stage 1 emit");
    check_build(emit(&stage2, &[stage1_entry()], &by_stage2), "stage 2 emit");
    assert!(std::fs::read(&by_stage1).unwrap() == std::fs::read(&by_stage2).unwrap(), "stage 1 does not reproduce itself");
}

#[test]
fn modules_compile_and_run() {
    let entry = root().join("tests/modules/run/main.tes");
    let exe = work_dir().join("modules_run");
    check_build(Command::new(stage1()).arg("build").arg(&entry).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap(), "building tests/modules/run");
    let out = Command::new(&exe).output().unwrap();
    let expected = std::fs::read_to_string(root().join("tests/modules/run/expected.out")).unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout), expected);
    // Names from other modules are prefixed in the generated code; the entry module's are not.
    let rust = std::fs::read_to_string(exe.with_extension("rs")).unwrap();
    assert!(rust.contains("fn f_geometry__shapes__area(") && rust.contains("struct T_geometry__shapes__Point") && rust.contains("fn f_main("));
}

#[test]
fn module_errors_are_reported() {
    let dirs = subdirectories("tests/modules/fail");
    assert!(dirs.len() >= 10);
    for dir in dirs {
        let code = dir.file_name().unwrap().to_string_lossy().split('-').next().unwrap().to_string();
        let out = Command::new(stage1()).arg("check").arg("--json").arg(dir.join("main.tes")).output().unwrap();
        let json = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(1), "{}: expected exit code 1", dir.display());
        assert!(json.contains(&format!("\"code\":\"{}\"", code)), "{}: expected {} in {}", dir.display(), code, json);
    }
}

#[test]
fn embedded_runtime_is_up_to_date() {
    let generated = work_dir().join("runtime_text.tes");
    // Every file in std/, in the order build.sh passes them (sorted by name).
    let std_modules: Vec<String> = tes_files("std")
        .iter()
        .map(|path| format!("std_{}={}", path.file_stem().unwrap().to_string_lossy(), path.display()))
        .collect();
    let out = stage0()
        .arg("run")
        .arg(root().join("tools/embed_runtime.tes"))
        .arg("--opt")
        .arg("0")
        .arg("--")
        .arg(&generated)
        .arg(format!("core_runtime={}", root().join("runtime/core.rs").display()))
        .arg(format!("ui_runtime={}", root().join("runtime/ui.rs").display()))
        .arg(format!("extra_runtime={}", root().join("runtime/extra.rs").display()))
        .arg(format!("web_runtime={}", root().join("runtime/web.rs").display()))
        .arg(format!("adapter_runtime={}", root().join("runtime/adapter.rs").display()))
        .args(&std_modules)
        .output()
        .unwrap();
    check_build(out, "embedding the runtime");
    let fresh = std::fs::read_to_string(&generated).unwrap();
    let committed = std::fs::read_to_string(root().join("stage1/runtime_text.tes")).unwrap();
    assert!(fresh == committed, "stage1/runtime_text.tes is stale: run build.sh");
}

#[test]
fn stage1_builds_the_graphical_tic_tac_toe() {
    let game = root().join("examples/tictactoe/tictactoe.tes");
    let exe = work_dir().join("tictactoe");
    check_build(Command::new(stage1()).arg("build").arg(&game).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap(), "building tic-tac-toe");
    let rust = std::fs::read_to_string(exe.with_extension("rs")).unwrap();
    assert!(rust.contains("#![forbid(unsafe_code)]") && rust.contains("fn t_open_window(") && rust.contains("fn f_rules__play("));
}

#[test]
fn stage1_only_programs_run_with_expected_output() {
    for path in tes_files("tests/stage1_run") {
        let exe = work_dir().join(path.file_stem().unwrap());
        check_build(Command::new(stage1()).arg("build").arg(&path).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap(), "building a stage 1 test program");
        let out = Command::new(&exe).output().unwrap();
        assert_eq!(out.status.code(), Some(0), "{}", path.display());
        let expected = std::fs::read_to_string(path.with_extension("out")).unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), expected, "{}", path.display());
    }
}

#[test]
fn stage1_only_runtime_errors_stop_the_program_predictably() {
    let files = tes_files("tests/stage1_runtime_fail");
    assert!(files.len() >= 5);
    for path in files {
        let code = path.file_name().unwrap().to_string_lossy().split('-').next().unwrap().to_string();
        let exe = work_dir().join(format!("runtime_fail_{}", path.file_stem().unwrap().to_string_lossy()));
        check_build(Command::new(stage1()).arg("build").arg(&path).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap(), "building a stage 1 runtime-error program");
        let out = Command::new(&exe).output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(70), "{}: expected exit code 70\n{}", path.display(), stderr);
        assert!(stderr.contains(&format!("runtime error {}", code)), "{}: expected {} in:\n{}", path.display(), code, stderr);
        assert!(stderr.contains(" in function `"), "{}: missing location in:\n{}", path.display(), stderr);
        assert!(out.stdout.is_empty(), "{}: printed after the error", path.display());
    }
}

fn build_host_program(name: &str) -> PathBuf {
    let exe = work_dir().join(format!("host_{}", name));
    let source = root().join(format!("tests/stage1_host/{}.tes", name));
    check_build(Command::new(stage1()).arg("build").arg(&source).arg("-o").arg(&exe).arg("--opt").arg("1").output().unwrap(), "building a host access program");
    exe
}

#[test]
fn binary_files_are_read_and_written_as_values() {
    let exe = build_host_program("files");
    let output_path = work_dir().join("host_files_output.bin");
    let _ = std::fs::remove_file(&output_path);
    let out = Command::new(&exe).arg(root().join("tests/stage1_host/fixture.bin")).arg(&output_path).output().unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    // Expected values computed independently from the fixture: b"PWAD", i32 3, i32 -2, bytes 0..=255.
    assert_eq!(String::from_utf8_lossy(&out.stdout), concat!(
        "length 268 kind PWAD a 3 b -2 sum 33962 last 255\n",
        "missing: not found\n",
        "round trip identical, original unchanged\n",
        "write error: not found\n"));
    let mut expected = std::fs::read(root().join("tests/stage1_host/fixture.bin")).unwrap();
    expected[0] = 0;
    *expected.last_mut().unwrap() = 128;
    assert_eq!(std::fs::read(&output_path).unwrap(), expected);
}

#[test]
fn arguments_come_in_and_exit_codes_go_out() {
    let exe = build_host_program("exit_code");
    let out = Command::new(&exe).args(["3", "doom", "e1m1"]).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "2 words: doom|e1m1\n");
    let usage = Command::new(&exe).output().unwrap();
    assert_eq!(usage.status.code(), Some(64));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("usage:"));
    assert_eq!(Command::new(&exe).arg("0").output().unwrap().status.code(), Some(0));
}

#[test]
fn frame_pacing_keeps_35_frames_per_second() {
    let exe = build_host_program("pacing");
    let out = Command::new(&exe).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let value = |label: &str| -> i64 {
        text.lines().find_map(|l| l.strip_prefix(label)).unwrap_or_else(|| panic!("no `{}` in {}", label, text)).trim().parse().unwrap()
    };
    // 35 paced frames take one second; 10 naive sleeps of 35 ms take at least 350 ms.
    let paced = value("paced");
    let naive = value("naive");
    assert!((995..=1100).contains(&paced), "35 paced frames took {} ms", paced);
    assert!((350..=500).contains(&naive), "10 sleeps of 35 ms took {} ms", naive);
}

#[test]
fn console_input_is_echoed_until_the_end() {
    use std::io::Write;
    let exe = build_host_program("echo");
    let mut child = Command::new(&exe).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(b"first\n\nthird with spaces\r\nlast").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "1: first\n2: \n3: third with spaces\n4: last\nend after 4 lines\n");
    let empty = Command::new(&exe).stdin(std::process::Stdio::null()).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&empty.stdout), "end after 0 lines\n");
}

#[test]
fn standard_module_tests_pass() {
    let files = tes_files("tests/std");
    assert!(files.len() >= 5);
    for path in files {
        let exe = work_dir().join(format!("std_{}", path.file_stem().unwrap().to_string_lossy()));
        let out = Command::new(stage1()).arg("test").arg(&path).arg("-o").arg(&exe).output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(0), "{}:\n{}{}", path.display(), stdout, String::from_utf8_lossy(&out.stderr));
        assert!(stdout.contains(" passed, 0 failed"), "{}:\n{}", path.display(), stdout);
    }
}

#[test]
fn stage1_builds_tesseradoom() {
    // The largest example program must keep compiling. With game data in examples/tesseradoom/WAD
    // (never committed, see its README) its milestone checks run as well.
    let dir = root().join("examples/tesseradoom");
    let exe = work_dir().join("tesseradoom");
    check_build(Command::new(stage1()).arg("build").arg(dir.join("main.tes")).arg("-o").arg(&exe).arg("--opt").arg("1").output().unwrap(), "building examples/tesseradoom");
    // Without arguments it opens the game's window, so the test asks for an unknown command.
    let usage = Command::new(&exe).arg("no-such-command").output().unwrap();
    assert_eq!(usage.status.code(), Some(64), "tesseradoom with an unknown command should print its usage");
    if dir.join("WAD/freedoom1.wad").exists() && Command::new("python3").arg("--version").output().is_ok() {
        let checks = Command::new(dir.join("checks/run_checks.sh")).output().unwrap();
        assert!(checks.status.success(), "tesseradoom milestone checks failed:\n{}", String::from_utf8_lossy(&checks.stdout));
    }
}

#[test]
fn stage1_only_errors_are_reported() {
    let files = tes_files("tests/stage1_fail");
    assert!(files.len() >= 10);
    for path in files {
        let code = path.file_name().unwrap().to_string_lossy().split('-').next().unwrap().to_string();
        let out = Command::new(stage1()).arg("check").arg("--json").arg(&path).output().unwrap();
        let json = String::from_utf8_lossy(&out.stdout);
        assert_eq!(out.status.code(), Some(1), "{}", path.display());
        assert!(json.contains(&format!("\"code\":\"{}\"", code)), "{}: expected {} in {}", path.display(), code, json);
    }
}

#[test]
fn web_application_runs_in_every_host() {
    let dir = work_dir();
    let exe = dir.join("web_app");
    let db = dir.join("web_app.db");
    let _ = std::fs::remove_file(&db);
    let app = root().join("tests/web/app.tes");
    check_build(Command::new(stage1()).arg("build").arg(&app).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap(), "building the web application");
    let run = |args: &[&str]| Command::new(&exe).args(args).env("APP_DB", &db).output().unwrap();
    let text = |out: &std::process::Output| String::from_utf8_lossy(&out.stdout).to_string();

    let home = run(&["request", "GET", "/?name=<b>"]);
    assert!(text(&home).starts_with("HTTP/1.1 200") && text(&home).contains("&lt;b&gt;") && text(&home).contains("content-security-policy"), "{}", text(&home));
    let post = run(&["request", "POST", "/items", "-H", "content-type: application/x-www-form-urlencoded", "--body", "name=Ada"]);
    assert!(text(&post).starts_with("HTTP/1.1 303"), "{}", text(&post));
    let list = run(&["request", "GET", "/items"]);
    assert!(text(&list).contains("[{\"id\":\"1\",\"name\":\"Ada\"}]"), "{}", text(&list));
    let fail = run(&["request", "GET", "/fail"]);
    assert!(text(&fail).starts_with("HTTP/1.1 500") && String::from_utf8_lossy(&fail.stderr).contains("runtime error R003"), "{}", text(&fail));

    // CGI: the same executable, started by a web server with the request in the environment.
    let cgi = Command::new(&exe).env("GATEWAY_INTERFACE", "CGI/1.1").env("REQUEST_METHOD", "GET").env("REQUEST_URI", "/nope").output().unwrap();
    assert!(text(&cgi).starts_with("Status: 404"), "{}", text(&cgi));

    // `tessera1 test` runs the test declarations and reports failures with exit code 1.
    let passing = Command::new(stage1()).arg("test").arg(&app).arg("-o").arg(dir.join("web_tests")).output().unwrap();
    assert_eq!(passing.status.code(), Some(0), "{}", text(&passing));
    assert!(text(&passing).contains("2 passed, 0 failed"), "{}", text(&passing));
    let failing = Command::new(stage1()).arg("test").arg(root().join("tests/web/failing_test.tes")).arg("-o").arg(dir.join("failing_tests")).output().unwrap();
    assert_eq!(failing.status.code(), Some(1), "{}", text(&failing));
    assert!(text(&failing).contains("1 passed, 1 failed") && text(&failing).contains("R019"), "{}", text(&failing));

    // The shared library for the nginx, Apache and IIS modules.
    let library = dir.join("libweb_app.so");
    check_build(Command::new(stage1()).arg("build").arg(&app).arg("--library").arg("-o").arg(&library).arg("--opt").arg("0").output().unwrap(), "building the web application as a library");
    assert!(library.exists());
    let adapter = std::fs::read_to_string(dir.join("libweb_app.so.build/adapter.rs")).unwrap();
    let generated = std::fs::read_to_string(dir.join("libweb_app.so.build/app.rs")).unwrap();
    assert!(adapter.contains("tessera_handle") && generated.starts_with("// Generated") && generated.contains("#![forbid(unsafe_code)]") && !generated.contains("unsafe {"));
}
