//! Lexer: turns source text into tokens, including significant newlines.

use crate::diag::{Diagnostic, Span};

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Int(u64),
    Text(String),
    Ident(String),
    // Keywords
    Fn,
    Let,
    Var,
    Set,
    If,
    Else,
    For,
    In,
    While,
    Note,
    Uses,
    And,
    Or,
    Not,
    True,
    False,
    Type,
    Enum,
    Match,
    Return,
    Break,
    Continue,
    // Symbols
    Underscore,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    Arrow,
    FatArrow,
    Assign,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Newline,
    Eof,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Int(v) => format!("number `{}`", v),
            Tok::Text(_) => "text literal".into(),
            Tok::Ident(name) => format!("name `{}`", name),
            Tok::Newline => "end of line".into(),
            Tok::Eof => "end of file".into(),
            other => format!("`{}`", other.symbol()),
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Tok::Fn => "fn",
            Tok::Let => "let",
            Tok::Var => "var",
            Tok::Set => "set",
            Tok::If => "if",
            Tok::Else => "else",
            Tok::For => "for",
            Tok::In => "in",
            Tok::While => "while",
            Tok::Note => "note",
            Tok::Uses => "uses",
            Tok::And => "and",
            Tok::Or => "or",
            Tok::Not => "not",
            Tok::True => "true",
            Tok::False => "false",
            Tok::Type => "type",
            Tok::Enum => "enum",
            Tok::Match => "match",
            Tok::Return => "return",
            Tok::Break => "break",
            Tok::Continue => "continue",
            Tok::Underscore => "_",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::Comma => ",",
            Tok::Colon => ":",
            Tok::Dot => ".",
            Tok::Arrow => "->",
            Tok::FatArrow => "=>",
            Tok::Assign => "=",
            Tok::EqEq => "==",
            Tok::NotEq => "!=",
            Tok::Lt => "<",
            Tok::LtEq => "<=",
            Tok::Gt => ">",
            Tok::GtEq => ">=",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Star => "*",
            Tok::Slash => "/",
            Tok::Percent => "%",
            _ => "?",
        }
    }

    /// A line ending in one of these tokens continues on the next line.
    fn continues_line(&self) -> bool {
        matches!(
            self,
            Tok::Comma
                | Tok::Assign
                | Tok::Arrow
                | Tok::FatArrow
                | Tok::EqEq
                | Tok::NotEq
                | Tok::Lt
                | Tok::LtEq
                | Tok::Gt
                | Tok::GtEq
                | Tok::Plus
                | Tok::Minus
                | Tok::Star
                | Tok::Slash
                | Tok::Percent
                | Tok::And
                | Tok::Or
                | Tok::Not
                | Tok::Dot
                | Tok::LParen
                | Tok::LBracket
                | Tok::LBrace
        )
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

pub const RESERVED: &[&str] = &["module", "import", "pub", "try", "unsafe", "loop", "const", "trait", "impl"];

fn keyword(word: &str) -> Option<Tok> {
    Some(match word {
        "fn" => Tok::Fn,
        "let" => Tok::Let,
        "var" => Tok::Var,
        "set" => Tok::Set,
        "if" => Tok::If,
        "else" => Tok::Else,
        "for" => Tok::For,
        "in" => Tok::In,
        "while" => Tok::While,
        "note" => Tok::Note,
        "uses" => Tok::Uses,
        "and" => Tok::And,
        "or" => Tok::Or,
        "not" => Tok::Not,
        "true" => Tok::True,
        "false" => Tok::False,
        "type" => Tok::Type,
        "enum" => Tok::Enum,
        "match" => Tok::Match,
        "return" => Tok::Return,
        "break" => Tok::Break,
        "continue" => Tok::Continue,
        _ => return None,
    })
}

struct Lexer<'a> {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    col: u32,
    file: usize,
    tokens: Vec<Token>,
    diags: &'a mut Vec<Diagnostic>,
    brackets: Vec<(char, Span)>,
}

pub fn lex(text: &str, file: usize, diags: &mut Vec<Diagnostic>) -> Vec<Token> {
    let mut lexer = Lexer {
        chars: text.chars().collect(),
        pos: 0,
        line: 1,
        col: 1,
        file,
        tokens: Vec::new(),
        diags,
        brackets: Vec::new(),
    };
    lexer.run();
    lexer.tokens
}

impl<'a> Lexer<'a> {
    fn peek(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn here(&self) -> Span {
        Span { file: self.file, line: self.line, col: self.col, end_line: self.line, end_col: self.col + 1 }
    }

    fn span_from(&self, line: u32, col: u32) -> Span {
        Span { file: self.file, line, col, end_line: self.line, end_col: self.col }
    }

    fn push(&mut self, tok: Tok, span: Span) {
        self.tokens.push(Token { tok, span });
    }

    fn error(&mut self, diag: Diagnostic) {
        self.diags.push(diag);
    }

    fn newline(&mut self, span: Span) {
        if let Some((open, _)) = self.brackets.last() {
            if *open == '(' || *open == '[' {
                return;
            }
        }
        match self.tokens.last() {
            None => {}
            Some(t) if t.tok == Tok::Newline || t.tok.continues_line() => {}
            Some(_) => self.push(Tok::Newline, span),
        }
    }

    fn run(&mut self) {
        if self.peek(0) == Some('\u{feff}') {
            let span = self.here();
            self.error(Diagnostic::new("E0101", span, "file starts with a byte order mark").help("save the file as UTF-8 without BOM"));
            self.bump();
        }
        while let Some(c) = self.peek(0) {
            let (line, col) = (self.line, self.col);
            match c {
                '\n' => {
                    let span = self.here();
                    self.bump();
                    self.newline(span);
                }
                ' ' => {
                    self.bump();
                }
                '\t' => {
                    let span = self.here();
                    self.error(Diagnostic::new("E0101", span, "tab characters are not allowed").help("indent with 4 spaces; write `\\t` inside text literals"));
                    self.bump();
                }
                '\r' => {
                    let span = self.here();
                    self.error(Diagnostic::new("E0101", span, "carriage return characters are not allowed").help("use Unix line endings (LF)"));
                    self.bump();
                }
                '/' if self.peek(1) == Some('/') => {
                    while let Some(c) = self.peek(0) {
                        if c == '\n' {
                            break;
                        }
                        if c == '\r' || c == '\t' {
                            // still report: comments obey the same file rules
                            let span = self.here();
                            self.error(Diagnostic::new("E0101", span, "tab or carriage return characters are not allowed"));
                        }
                        self.bump();
                    }
                }
                '"' => self.text_literal(),
                '0'..='9' => self.number(),
                c if c.is_ascii_alphabetic() => self.word(),
                '_' => {
                    if self.peek(1).map_or(false, |n| n.is_ascii_alphanumeric() || n == '_') {
                        let span = self.here();
                        self.error(Diagnostic::new("E0103", span, "names must start with a letter"));
                        self.bump();
                        while self.peek(0).map_or(false, |n| n.is_ascii_alphanumeric() || n == '_') {
                            self.bump();
                        }
                    } else {
                        self.bump();
                        let span = self.span_from(line, col);
                        self.push(Tok::Underscore, span);
                    }
                }
                c if c.is_alphabetic() => {
                    let span = self.here();
                    self.error(
                        Diagnostic::new("E0103", span, format!("non-ASCII letter `{}` in a name", c))
                            .help("names use ASCII letters, digits and `_` only; this prevents look-alike characters (homoglyphs)"),
                    );
                    self.bump();
                }
                _ => self.symbol(),
            }
        }
        let span = self.here();
        for (open, open_span) in std::mem::take(&mut self.brackets).into_iter().rev() {
            self.error(Diagnostic::new("E0102", open_span, format!("`{}` is never closed", open)).label("opened here"));
        }
        self.newline(span);
        if self.tokens.last().map_or(false, |t| t.tok != Tok::Newline) {
            self.push(Tok::Newline, span);
        }
        self.push(Tok::Eof, span);
    }

    fn word(&mut self) {
        let (line, col) = (self.line, self.col);
        let mut word = String::new();
        while let Some(c) = self.peek(0) {
            if c.is_ascii_alphanumeric() || c == '_' {
                word.push(c);
                self.bump();
            } else if c.is_alphabetic() {
                let span = self.here();
                self.error(
                    Diagnostic::new("E0103", span, format!("non-ASCII letter `{}` in a name", c))
                        .help("names use ASCII letters, digits and `_` only; this prevents look-alike characters (homoglyphs)"),
                );
                self.bump();
            } else {
                break;
            }
        }
        let span = self.span_from(line, col);
        if let Some(tok) = keyword(&word) {
            self.push(tok, span);
        } else {
            if RESERVED.contains(&word.as_str()) {
                self.error(Diagnostic::new("E0108", span, format!("`{}` is a reserved word", word)).help("choose a different name"));
            }
            self.push(Tok::Ident(word), span);
        }
    }

    fn number(&mut self) {
        let (line, col) = (self.line, self.col);
        let mut digits = String::new();
        let mut raw = String::new();
        while let Some(c) = self.peek(0) {
            if c.is_ascii_digit() {
                digits.push(c);
                raw.push(c);
                self.bump();
            } else if c == '_' {
                raw.push(c);
                self.bump();
            } else {
                break;
            }
        }
        let mut bad_suffix = false;
        while self.peek(0).map_or(false, |c| c.is_alphanumeric()) {
            bad_suffix = true;
            raw.push(self.bump().unwrap());
        }
        let span = self.span_from(line, col);
        if bad_suffix {
            self.error(Diagnostic::new("E0106", span, format!("invalid number `{}`", raw)).help("numbers are decimal digits only, optionally separated by `_`"));
            self.push(Tok::Int(0), span);
            return;
        }
        if raw.ends_with('_') || raw.contains("__") {
            self.error(Diagnostic::new("E0106", span, format!("invalid number `{}`", raw)).help("`_` may only appear between digits, e.g. `1_000_000`"));
        }
        if digits.len() > 1 && digits.starts_with('0') {
            self.error(Diagnostic::new("E0106", span, format!("number `{}` has a leading zero", raw)).help("leading zeros are not allowed; write the number without them"));
        }
        let value = match digits.parse::<u64>() {
            Ok(v) if v <= 1u64 << 63 => v,
            _ => {
                self.error(Diagnostic::new("E0106", span, format!("number `{}` does not fit in `Int`", raw)).help("`Int` is a signed 64-bit integer: -9223372036854775808 to 9223372036854775807"));
                0
            }
        };
        self.push(Tok::Int(value), span);
    }

    fn text_literal(&mut self) {
        let (line, col) = (self.line, self.col);
        self.bump(); // opening quote
        let mut value = String::new();
        loop {
            match self.peek(0) {
                None | Some('\n') => {
                    let span = self.span_from(line, col);
                    self.error(Diagnostic::new("E0109", span, "text literal is not closed").help("add a closing `\"` on the same line; use `\\n` for line breaks"));
                    break;
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    let esc_span = self.here();
                    self.bump();
                    match self.bump() {
                        Some('n') => value.push('\n'),
                        Some('t') => value.push('\t'),
                        Some('\\') => value.push('\\'),
                        Some('"') => value.push('"'),
                        Some('u') => {
                            if self.peek(0) != Some('{') {
                                self.error(Diagnostic::new("E0109", esc_span, "invalid unicode escape").help("write unicode escapes as `\\u{1F600}`"));
                                continue;
                            }
                            self.bump();
                            let mut hex = String::new();
                            while let Some(c) = self.peek(0) {
                                if c == '}' || c == '"' || c == '\n' {
                                    break;
                                }
                                hex.push(c);
                                self.bump();
                            }
                            if self.peek(0) == Some('}') {
                                self.bump();
                            }
                            let ok = hex.len() >= 1 && hex.len() <= 6 && hex.chars().all(|c| c.is_ascii_hexdigit());
                            match (ok, u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)) {
                                (true, Some(c)) => value.push(c),
                                _ => {
                                    self.error(Diagnostic::new("E0109", esc_span, format!("`\\u{{{}}}` is not a valid unicode scalar value", hex)));
                                }
                            }
                        }
                        other => {
                            let shown = other.map(|c| c.to_string()).unwrap_or_default();
                            self.error(Diagnostic::new("E0109", esc_span, format!("unknown escape `\\{}`", shown)).help("valid escapes are `\\n`, `\\t`, `\\\\`, `\\\"` and `\\u{...}`"));
                        }
                    }
                }
                Some(c) if c.is_control() => {
                    let span = self.here();
                    self.error(Diagnostic::new("E0109", span, "control character inside a text literal").help("use an escape such as `\\t` or `\\u{...}`"));
                    self.bump();
                }
                Some(c) => {
                    value.push(c);
                    self.bump();
                }
            }
        }
        let span = self.span_from(line, col);
        self.push(Tok::Text(value), span);
    }

    fn symbol(&mut self) {
        let (line, col) = (self.line, self.col);
        let c = self.bump().unwrap();
        let next = self.peek(0);
        let tok = match (c, next) {
            ('-', Some('>')) => {
                self.bump();
                Tok::Arrow
            }
            ('=', Some('>')) => {
                self.bump();
                Tok::FatArrow
            }
            ('=', Some('=')) => {
                self.bump();
                Tok::EqEq
            }
            ('!', Some('=')) => {
                self.bump();
                Tok::NotEq
            }
            ('<', Some('=')) => {
                self.bump();
                Tok::LtEq
            }
            ('>', Some('=')) => {
                self.bump();
                Tok::GtEq
            }
            ('(', _) => Tok::LParen,
            (')', _) => Tok::RParen,
            ('[', _) => Tok::LBracket,
            (']', _) => Tok::RBracket,
            ('{', _) => Tok::LBrace,
            ('}', _) => Tok::RBrace,
            (',', _) => Tok::Comma,
            (':', _) => Tok::Colon,
            ('.', _) => Tok::Dot,
            ('=', _) => Tok::Assign,
            ('<', _) => Tok::Lt,
            ('>', _) => Tok::Gt,
            ('+', _) => Tok::Plus,
            ('-', _) => Tok::Minus,
            ('*', _) => Tok::Star,
            ('/', _) => Tok::Slash,
            ('%', _) => Tok::Percent,
            _ => {
                let span = self.span_from(line, col);
                let help = match c {
                    '!' => "use `not` for negation and `!=` for inequality",
                    '&' => "use `and` instead of `&&`",
                    '|' => "use `or` instead of `||`",
                    ';' => "statements end at the end of a line; remove the `;`",
                    '\'' => "text literals use double quotes",
                    _ => "remove this character",
                };
                self.error(Diagnostic::new("E0103", span, format!("unexpected character `{}`", c)).help(help));
                return;
            }
        };
        let span = self.span_from(line, col);
        match tok {
            Tok::LParen => self.brackets.push(('(', span)),
            Tok::LBracket => self.brackets.push(('[', span)),
            Tok::LBrace => self.brackets.push(('{', span)),
            Tok::RParen | Tok::RBracket | Tok::RBrace => {
                let expected = match tok {
                    Tok::RParen => '(',
                    Tok::RBracket => '[',
                    _ => '{',
                };
                match self.brackets.last() {
                    Some((open, _)) if *open == expected => {
                        self.brackets.pop();
                    }
                    Some((open, open_span)) => {
                        let open_span = *open_span;
                        let open = *open;
                        self.error(
                            Diagnostic::new("E0102", span, format!("`{}` does not match the open `{}`", c, open))
                                .help(format!("the `{}` was opened on line {}", open, open_span.line)),
                        );
                        self.brackets.pop();
                    }
                    None => {
                        self.error(Diagnostic::new("E0102", span, format!("`{}` has no matching opening bracket", c)));
                    }
                }
                // A newline right before a closing bracket inside braces is a statement end;
                // the parser tolerates it.
            }
            _ => {}
        }
        self.push(tok, span);
    }
}
