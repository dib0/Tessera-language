//! Tessera stage 0 compiler: Tessera source to safe Rust.

pub mod ast;
pub mod checker;
pub mod codegen;
pub mod diag;
pub mod lexer;
pub mod notes;
pub mod parser;
pub mod runtime;
pub mod types;

use diag::{Diagnostic, SourceFile};

pub struct Frontend {
    pub program: ast::Program,
    pub checked: Option<checker::Checked>,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lex and parse all files into one program. Files share one namespace.
pub fn parse(files: &[SourceFile], diags: &mut Vec<Diagnostic>) -> ast::Program {
    let mut items = Vec::new();
    let mut next_id = 0;
    for (index, file) in files.iter().enumerate() {
        let mut file_diags = Vec::new();
        let tokens = lexer::lex(&file.text, index, &mut file_diags);
        let lex_failed = !file_diags.is_empty();
        diags.append(&mut file_diags);
        if lex_failed {
            // Parsing broken token streams mostly produces follow-up noise.
            continue;
        }
        parser::parse_file(tokens, &mut next_id, diags, &mut items);
    }
    ast::Program { items, expr_count: next_id }
}

/// Run the full front end. Type checking only runs on a program that parsed cleanly.
pub fn frontend(files: &[SourceFile]) -> Frontend {
    let mut diagnostics = Vec::new();
    let program = parse(files, &mut diagnostics);
    let checked = if diagnostics.is_empty() {
        let checked = checker::check(&program, &mut diagnostics);
        Some(checked)
    } else {
        None
    };
    sort_diagnostics(&mut diagnostics);
    Frontend { program, checked, diagnostics }
}

fn sort_diagnostics(diags: &mut Vec<Diagnostic>) {
    diags.sort_by_key(|d| (d.span.file, d.span.line, d.span.col));
    diags.dedup_by(|a, b| a.span == b.span && a.code == b.code && a.message == b.message);
}

/// Compile to Rust source, or return diagnostics.
pub fn compile_to_rust(files: &[SourceFile]) -> Result<String, Vec<Diagnostic>> {
    let fe = frontend(files);
    if !fe.diagnostics.is_empty() {
        return Err(fe.diagnostics);
    }
    let checked = fe.checked.expect("checked program");
    Ok(codegen::generate(&fe.program, files, &checked.expr_types, &checked.type_defs))
}
