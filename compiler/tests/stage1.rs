//! Tests for the stage 1 compiler (the Tessera compiler written in Tessera, in `stage1/`).
//!
//! Stage 0 builds stage 1 once; then stage 1 must:
//! - produce byte-identical Rust to stage 0 for every valid program,
//! - report the same error code as stage 0 for every invalid program,
//! - reproduce itself (self-hosting fixpoint),
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

fn stage1_sources() -> Vec<PathBuf> {
    ["source", "lexer", "ast", "parser", "checker", "codegen", "runtime_text", "main"]
        .iter()
        .map(|name| root().join("stage1").join(format!("{}.tes", name)))
        .collect()
}

fn work_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("stage1");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build the stage 1 compiler with stage 0 (once per test run).
fn stage1() -> &'static PathBuf {
    static BINARY: OnceLock<PathBuf> = OnceLock::new();
    BINARY.get_or_init(|| {
        let out = work_dir().join("tessera1");
        let status = stage0().arg("build").args(stage1_sources()).arg("-o").arg(&out).arg("--opt").arg("1").output().unwrap();
        assert!(status.status.success(), "stage 0 could not build stage 1:\n{}", String::from_utf8_lossy(&status.stderr));
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

fn emit(compiler: &Command, files: &[PathBuf], out: &Path) -> std::process::Output {
    let mut cmd = Command::new(compiler.get_program());
    cmd.arg("emit-rust").args(files).arg("-o").arg(out).output().unwrap()
}

#[test]
fn stage1_generates_identical_rust() {
    let dir = work_dir();
    let mut programs: Vec<Vec<PathBuf>> = Vec::new();
    for d in ["tests/run", "tests/runtime_fail", "examples/valid", "examples/runtime"] {
        for f in tes_files(d) {
            programs.push(vec![f]);
        }
    }
    programs.push(vec![root().join("tests/multi_file/trees.tes"), root().join("tests/multi_file/main.tes")]);
    for files in programs {
        let a = dir.join("by_stage0.rs");
        let b = dir.join("by_stage1.rs");
        assert!(emit(&stage0(), &files, &a).status.success());
        let out = emit(&Command::new(stage1()), &files, &b);
        assert!(out.status.success(), "{:?}: stage 1 failed:\n{}", files, String::from_utf8_lossy(&out.stderr));
        assert!(std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap(), "{:?}: stage 0 and stage 1 output differ", files);
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
    let sources = stage1_sources();
    let by_stage0 = dir.join("stage1_by_stage0.rs");
    let by_stage1 = dir.join("stage1_by_stage1.rs");
    assert!(emit(&stage0(), &sources, &by_stage0).status.success());
    let out = emit(&Command::new(stage1()), &sources, &by_stage1);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(std::fs::read(&by_stage0).unwrap() == std::fs::read(&by_stage1).unwrap(), "stage 1 does not reproduce itself");
}

#[test]
fn embedded_runtime_is_up_to_date() {
    let dir = work_dir();
    let generated = dir.join("runtime_text.tes");
    let out = stage0()
        .arg("run")
        .arg(root().join("tools/embed_runtime.tes"))
        .arg("--opt")
        .arg("0")
        .arg("--")
        .arg(root().join("runtime/core.rs"))
        .arg(root().join("runtime/ui.rs"))
        .arg(&generated)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let fresh = std::fs::read_to_string(&generated).unwrap();
    let committed = std::fs::read_to_string(root().join("stage1/runtime_text.tes")).unwrap();
    assert!(fresh == committed, "stage1/runtime_text.tes is stale: rerun tools/embed_runtime.tes (see build.sh)");
}

#[test]
fn stage1_builds_the_graphical_tic_tac_toe() {
    let game = root().join("examples/tictactoe/tictactoe.tes");
    // Stage 0 does not implement the `ui` effect; stage 1 does.
    let check0 = stage0().arg("check").arg("--json").arg(&game).output().unwrap();
    assert!(String::from_utf8_lossy(&check0.stdout).contains("E0303"));
    let exe = work_dir().join("tictactoe");
    let out = Command::new(stage1()).arg("build").arg(&game).arg("-o").arg(&exe).arg("--opt").arg("0").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(exe.exists());
    let rust = std::fs::read_to_string(exe.with_extension("rs")).unwrap();
    assert!(rust.contains("#![forbid(unsafe_code)]") && rust.contains("fn t_open_window("));
}
