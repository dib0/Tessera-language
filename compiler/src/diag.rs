//! Source files, spans, and diagnostics.

use std::fmt::Write;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub file: usize,
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    /// Exclusive end column.
    pub end_col: u32,
}

impl Span {
    pub fn to(self, other: Span) -> Span {
        Span { end_line: other.end_line, end_col: other.end_col, ..self }
    }
}

pub struct SourceFile {
    pub path: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub label: Option<String>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn new(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic { code, message: message.into(), span, label: None, help: None }
    }
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

pub fn render(diag: &Diagnostic, files: &[SourceFile]) -> String {
    let mut out = String::new();
    let file = &files[diag.span.file];
    let _ = writeln!(out, "error[{}]: {}", diag.code, diag.message);
    let _ = writeln!(out, "  --> {}:{}:{}", file.path, diag.span.line, diag.span.col);
    let line_text = file.text.lines().nth(diag.span.line.saturating_sub(1) as usize);
    if let Some(line_text) = line_text {
        let number = diag.span.line.to_string();
        let pad = " ".repeat(number.len());
        let _ = writeln!(out, "{} |", pad);
        let _ = writeln!(out, "{} | {}", number, line_text);
        let line_len = line_text.chars().count() as u32;
        let start = diag.span.col.max(1);
        let end = if diag.span.end_line == diag.span.line { diag.span.end_col } else { line_len + 1 };
        let width = end.saturating_sub(start).max(1);
        let mut marker = format!("{} | {}{}", pad, " ".repeat((start - 1) as usize), "^".repeat(width as usize));
        if let Some(label) = &diag.label {
            marker.push(' ');
            marker.push_str(label);
        }
        let _ = writeln!(out, "{}", marker);
        let _ = writeln!(out, "{} |", pad);
    }
    if let Some(help) = &diag.help {
        let _ = writeln!(out, "help: {}", help);
    }
    out
}

pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn render_json(diags: &[Diagnostic], files: &[SourceFile]) -> String {
    let mut out = String::from("{\"diagnostics\":[");
    for (i, d) in diags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"severity\":\"error\",\"code\":{},\"message\":{},\"file\":{},\"line\":{},\"column\":{},\"end_line\":{},\"end_column\":{},\"label\":{},\"help\":{}}}",
            json_string(d.code),
            json_string(&d.message),
            json_string(&files[d.span.file].path),
            d.span.line,
            d.span.col,
            d.span.end_line,
            d.span.end_col,
            d.label.as_deref().map(json_string).unwrap_or_else(|| "null".into()),
            d.help.as_deref().map(json_string).unwrap_or_else(|| "null".into()),
        );
    }
    out.push_str("]}");
    out
}
