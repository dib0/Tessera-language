//! `tessera` command-line interface.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use tessera::diag::{render, render_json, SourceFile};

const USAGE: &str = "\
Tessera stage 0 compiler

Usage:
  tessera check <files...> [--json]           Check for errors without generating code
  tessera build <files...> -o <output>        Compile to a native executable
        [--opt 0|1|2|3] [--keep-rust <file.rs>]
  tessera run <files...> [-- <args...>]       Compile and run
  tessera emit-rust <files...> [-o <file.rs>] Write the generated Rust code
  tessera notes <files...>                    Export notes as JSON

All files given together form one program and share one namespace.

Exit codes: 0 ok, 1 compile errors, 2 usage or I/O error, 3 Rust backend failure.
Set TESSERA_RUSTC to choose the rustc binary.
";

struct Options {
    command: String,
    files: Vec<String>,
    output: Option<String>,
    json: bool,
    opt: String,
    keep_rust: Option<String>,
    program_args: Vec<String>,
}

fn parse_args() -> Result<Options, String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or_else(|| "missing command".to_string())?;
    let mut o = Options { command, files: vec![], output: None, json: false, opt: "2".into(), keep_rust: None, program_args: vec![] };
    while let Some(a) = args.next() {
        match a.as_str() {
            "-o" => o.output = Some(args.next().ok_or("`-o` needs a path")?),
            "--json" => o.json = true,
            "--opt" => {
                let level = args.next().ok_or("`--opt` needs a level")?;
                if !["0", "1", "2", "3"].contains(&level.as_str()) {
                    return Err(format!("invalid optimization level `{}`", level));
                }
                o.opt = level;
            }
            "--keep-rust" => o.keep_rust = Some(args.next().ok_or("`--keep-rust` needs a path")?),
            "--" => {
                o.program_args = args.by_ref().collect();
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option `{}`", flag)),
            file => o.files.push(file.to_string()),
        }
    }
    if o.files.is_empty() && o.command != "help" && o.command != "--help" {
        return Err("no source files given".into());
    }
    Ok(o)
}

fn load(files: &[String]) -> Result<Vec<SourceFile>, String> {
    let mut out = Vec::new();
    for path in files {
        if !path.ends_with(".tes") {
            return Err(format!("`{}` is not a Tessera source file (expected the `.tes` extension)", path));
        }
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read `{}`: {}", path, e))?;
        let text = String::from_utf8(bytes).map_err(|_| format!("`{}` is not valid UTF-8", path))?;
        out.push(SourceFile { path: path.clone(), text });
    }
    Ok(out)
}

fn rustc() -> PathBuf {
    if let Some(p) = std::env::var_os("TESSERA_RUSTC") {
        return PathBuf::from(p);
    }
    if Command::new("rustc").arg("--version").output().is_ok() {
        return PathBuf::from("rustc");
    }
    // `HOME` on unix, `USERPROFILE` on Windows: where rustup installs the toolchain.
    for home in ["HOME", "USERPROFILE"] {
        if let Some(home) = std::env::var_os(home) {
            for name in ["rustc", "rustc.exe"] {
                let p = Path::new(&home).join(".cargo/bin").join(name);
                if p.exists() {
                    return p;
                }
            }
        }
    }
    PathBuf::from("rustc")
}

/// The file name rustc writes. `-o` writes exactly the name it is given, so on
/// Windows a name without an extension gets `.exe`; without it the program
/// cannot be started from the shell.
fn executable_name(output: &Path) -> PathBuf {
    if !cfg!(windows) || output.extension().is_some() {
        return output.to_path_buf();
    }
    let mut name = output.as_os_str().to_os_string();
    name.push(".exe");
    PathBuf::from(name)
}

fn temp_dir() -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("tessera-{}-{}", std::process::id(), nanos));
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create temporary directory: {}", e))?;
    Ok(dir)
}

enum Failure {
    Compile,
    Usage(String),
    Backend(String),
}

fn compile(o: &Options, files: &[SourceFile]) -> Result<String, Failure> {
    match tessera::compile_to_rust(files) {
        Ok(rust) => Ok(rust),
        Err(diags) => {
            if o.json {
                println!("{}", render_json(&diags, files));
            } else {
                for d in &diags {
                    eprint!("{}\n", render(d, files));
                }
                eprintln!("{} error(s); no code was generated", diags.len());
            }
            Err(Failure::Compile)
        }
    }
}

fn build_binary(o: &Options, rust: &str, output: &Path) -> Result<(), Failure> {
    let dir = temp_dir().map_err(Failure::Usage)?;
    let src = dir.join("main.rs");
    std::fs::write(&src, rust).map_err(|e| Failure::Usage(format!("cannot write generated code: {}", e)))?;
    if let Some(keep) = &o.keep_rust {
        std::fs::write(keep, rust).map_err(|e| Failure::Usage(format!("cannot write `{}`: {}", keep, e)))?;
    }
    let result = Command::new(rustc())
        .arg("--edition")
        .arg("2021")
        .arg("-C")
        .arg(format!("opt-level={}", o.opt))
        .arg("--crate-name")
        .arg("tessera_program")
        .arg("-o")
        .arg(output)
        .arg(&src)
        .output();
    let _ = std::fs::remove_dir_all(&dir);
    match result {
        Err(e) => Err(Failure::Backend(format!("cannot run rustc: {} (install Rust from https://rustup.rs or set TESSERA_RUSTC)", e))),
        Ok(out) if !out.status.success() => Err(Failure::Backend(format!(
            "internal compiler error: the generated Rust code did not compile. This is a bug in the Tessera compiler.\n{}",
            String::from_utf8_lossy(&out.stderr)
        ))),
        Ok(_) => Ok(()),
    }
}

fn run(o: Options) -> Result<i32, Failure> {
    let files = load(&o.files).map_err(Failure::Usage)?;
    match o.command.as_str() {
        "check" => {
            let fe = tessera::frontend(&files);
            if o.json {
                println!("{}", render_json(&fe.diagnostics, &files));
            } else {
                for d in &fe.diagnostics {
                    eprint!("{}\n", render(d, &files));
                }
                if fe.diagnostics.is_empty() {
                    eprintln!("ok: no errors");
                } else {
                    eprintln!("{} error(s)", fe.diagnostics.len());
                }
            }
            Ok(if fe.diagnostics.is_empty() { 0 } else { 1 })
        }
        "emit-rust" => {
            let rust = compile(&o, &files)?;
            match &o.output {
                Some(path) => std::fs::write(path, rust).map_err(|e| Failure::Usage(format!("cannot write `{}`: {}", path, e)))?,
                None => print!("{}", rust),
            }
            Ok(0)
        }
        "build" => {
            let output = o.output.clone().ok_or_else(|| Failure::Usage("`build` needs `-o <output>`".into()))?;
            let rust = compile(&o, &files)?;
            build_binary(&o, &rust, &executable_name(Path::new(&output)))?;
            Ok(0)
        }
        "run" => {
            let rust = compile(&o, &files)?;
            let dir = temp_dir().map_err(Failure::Usage)?;
            let exe = executable_name(&dir.join("program"));
            let built = build_binary(&o, &rust, &exe);
            if let Err(e) = built {
                let _ = std::fs::remove_dir_all(&dir);
                return Err(e);
            }
            let status = Command::new(&exe).args(&o.program_args).status();
            let _ = std::fs::remove_dir_all(&dir);
            match status {
                Ok(s) => Ok(s.code().unwrap_or(70)),
                Err(e) => Err(Failure::Usage(format!("cannot start the program: {}", e))),
            }
        }
        "notes" => {
            let fe = tessera::frontend(&files);
            if !fe.diagnostics.is_empty() {
                for d in &fe.diagnostics {
                    eprint!("{}\n", render(d, &files));
                }
                return Err(Failure::Compile);
            }
            print!("{}", tessera::notes::export(&fe.program, &files));
            Ok(0)
        }
        other => Err(Failure::Usage(format!("unknown command `{}`", other))),
    }
}

fn main() -> ExitCode {
    // Deeply nested programs recurse deeply in the compiler; give it room.
    let handle = std::thread::Builder::new().stack_size(256 * 1024 * 1024).spawn(|| {
        let o = match parse_args() {
            Ok(o) => o,
            Err(e) => {
                eprintln!("error: {}\n\n{}", e, USAGE);
                return 2;
            }
        };
        if o.command == "help" || o.command == "--help" {
            print!("{}", USAGE);
            return 0;
        }
        match run(o) {
            Ok(code) => code,
            Err(Failure::Compile) => 1,
            Err(Failure::Usage(e)) => {
                eprintln!("error: {}", e);
                2
            }
            Err(Failure::Backend(e)) => {
                eprintln!("error: {}", e);
                3
            }
        }
    });
    let code = match handle {
        Ok(h) => h.join().unwrap_or(101),
        Err(_) => 101,
    };
    ExitCode::from(code.clamp(0, 255) as u8)
}
