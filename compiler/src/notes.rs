//! Machine-readable export of `note` metadata.

use std::fmt::Write;

use crate::ast::{Item, Note, Program};
use crate::diag::{json_string, SourceFile};

pub fn export(program: &Program, files: &[SourceFile]) -> String {
    let mut entries: Vec<String> = Vec::new();
    for item in &program.items {
        let (kind, name, span, notes): (&str, &str, _, &Vec<Note>) = match item {
            Item::Function(f) => ("function", &f.name, f.name_span, &f.notes),
            Item::Record(r) => ("type", &r.name, r.name_span, &r.notes),
            Item::Enum(e) => ("enum", &e.name, e.name_span, &e.notes),
        };
        if notes.is_empty() {
            continue;
        }
        let texts: Vec<String> = notes.iter().map(|n| json_string(&n.text)).collect();
        let mut entry = String::new();
        let _ = write!(
            entry,
            "    {{\"file\": {}, \"item\": {}, \"kind\": {}, \"line\": {}, \"text\": [{}]}}",
            json_string(&files[span.file].path),
            json_string(name),
            json_string(kind),
            span.line,
            texts.join(", ")
        );
        entries.push(entry);
    }
    if entries.is_empty() {
        return "{\n  \"notes\": []\n}\n".into();
    }
    format!("{{\n  \"notes\": [\n{}\n  ]\n}}\n", entries.join(",\n"))
}
