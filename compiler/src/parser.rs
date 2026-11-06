//! Recursive-descent parser. Produces an AST and diagnostics; never panics on bad input.

use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::lexer::{Tok, Token};

const MAX_DEPTH: usize = 200;

type PResult<T> = Result<T, ()>;

pub struct Parser<'a> {
    toks: Vec<Token>,
    pos: usize,
    diags: &'a mut Vec<Diagnostic>,
    next_id: &'a mut usize,
    depth: usize,
}

pub fn parse_file(toks: Vec<Token>, next_id: &mut usize, diags: &mut Vec<Diagnostic>, items: &mut Vec<Item>) {
    let mut p = Parser { toks, pos: 0, diags, next_id, depth: 0 };
    loop {
        p.skip_newlines();
        if p.at(&Tok::Eof) {
            break;
        }
        let before = p.pos;
        match p.item() {
            Ok(item) => items.push(item),
            Err(()) => {
                p.recover_top();
                if p.pos == before {
                    p.bump();
                }
            }
        }
    }
}

fn is_type_name(name: &str) -> bool {
    name.chars().next().map_or(false, |c| c.is_ascii_uppercase())
}

impl<'a> Parser<'a> {
    // ---------- token helpers ----------

    fn peek(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn peek_at(&self, offset: usize) -> &Tok {
        let i = (self.pos + offset).min(self.toks.len() - 1);
        &self.toks[i].tok
    }

    fn span(&self) -> Span {
        self.toks[self.pos].span
    }

    fn prev_span(&self) -> Span {
        if self.pos == 0 {
            self.toks[0].span
        } else {
            self.toks[self.pos - 1].span
        }
    }

    fn at(&self, tok: &Tok) -> bool {
        self.peek() == tok
    }

    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn skip_newlines(&mut self) {
        while self.at(&Tok::Newline) {
            self.bump();
        }
    }

    fn error<T>(&mut self, diag: Diagnostic) -> PResult<T> {
        self.diags.push(diag);
        Err(())
    }

    fn unexpected<T>(&mut self, expected: &str) -> PResult<T> {
        let found = self.peek().describe();
        let span = self.span();
        let mut d = Diagnostic::new("E0110", span, format!("expected {}, found {}", expected, found));
        if *self.peek() == Tok::Comma {
            d = d.help("separate items here with a line break instead of a comma");
        }
        self.error(d)
    }

    fn expect(&mut self, tok: Tok) -> PResult<Span> {
        if *self.peek() == tok {
            Ok(self.bump().span)
        } else {
            let expected = format!("`{}`", tok.symbol());
            self.unexpected(&expected)
        }
    }

    fn ident(&mut self, what: &str) -> PResult<(String, Span)> {
        if let Tok::Ident(name) = self.peek().clone() {
            let span = self.bump().span;
            Ok((name, span))
        } else {
            self.unexpected(what)
        }
    }

    fn mk(&mut self, kind: ExprKind, span: Span) -> Expr {
        let id = *self.next_id;
        *self.next_id += 1;
        Expr { id, kind, span }
    }

    fn end_of_line(&mut self, what: &str) -> PResult<()> {
        match self.peek() {
            Tok::Newline => {
                self.bump();
                Ok(())
            }
            Tok::RBrace | Tok::Eof => Ok(()),
            _ => self.unexpected(&format!("end of line after {}", what)),
        }
    }

    /// Skip to the start of the next top-level declaration.
    fn recover_top(&mut self) {
        let mut depth: i64 = 0;
        loop {
            match self.peek() {
                Tok::Eof => return,
                Tok::LBrace => depth += 1,
                Tok::RBrace => depth -= 1,
                Tok::Newline if depth <= 0 => {
                    if matches!(self.peek_at(1), Tok::Fn | Tok::Type | Tok::Enum | Tok::Note) {
                        self.bump();
                        return;
                    }
                }
                _ => {}
            }
            self.bump();
        }
    }

    // ---------- declarations ----------

    fn item(&mut self) -> PResult<Item> {
        let mut notes = Vec::new();
        while self.at(&Tok::Note) {
            let start = self.bump().span;
            match self.peek().clone() {
                Tok::Text(text) => {
                    let end = self.bump().span;
                    notes.push(Note { text, span: start.to(end) });
                }
                _ => return self.unexpected("a text literal after `note`"),
            }
            self.end_of_line("a note")?;
            let note_line = notes.last().unwrap().span.line;
            if !self.at(&Tok::Eof) && self.span().line > note_line + 1 {
                let span = notes.last().unwrap().span;
                return self.error(
                    Diagnostic::new("E0401", span, "a note must be directly above the declaration it describes")
                        .help("remove the blank line between the note and the declaration"),
                );
            }
        }
        match self.peek() {
            Tok::Fn => self.function(notes).map(Item::Function),
            Tok::Type => self.record(notes).map(Item::Record),
            Tok::Enum => self.enum_decl(notes).map(Item::Enum),
            _ if !notes.is_empty() => {
                let span = notes.last().unwrap().span;
                self.error(
                    Diagnostic::new("E0401", span, "a note must be followed by a declaration")
                        .help("place the note directly above a `fn`, `type` or `enum`, or remove it"),
                )
            }
            Tok::Let | Tok::Var => {
                let span = self.span();
                self.error(Diagnostic::new("E0110", span, "bindings are only allowed inside functions").help("Tessera has no global variables; use a function that returns the value"))
            }
            _ => self.unexpected("`fn`, `type`, `enum` or `note`"),
        }
    }

    fn function(&mut self, notes: Vec<Note>) -> PResult<Function> {
        self.expect(Tok::Fn)?;
        let (name, name_span) = self.ident("a function name")?;
        self.expect(Tok::LParen)?;
        let mut params = Vec::new();
        while !self.at(&Tok::RParen) {
            let (pname, pspan) = self.ident("a parameter name")?;
            self.expect(Tok::Colon)?;
            let ty = self.type_expr()?;
            params.push(Param { name: pname, span: pspan, ty });
            if self.at(&Tok::Comma) {
                self.bump();
            } else if !self.at(&Tok::RParen) {
                return self.unexpected("`,` or `)`");
            }
        }
        self.expect(Tok::RParen)?;
        if !self.at(&Tok::Arrow) {
            let span = self.span();
            return self.error(Diagnostic::new("E0110", span, "missing return type").help("write the return type after `->`, e.g. `-> Unit`"));
        }
        self.bump();
        let ret = self.type_expr()?;
        let mut effects = Vec::new();
        if self.at(&Tok::Uses) {
            self.bump();
            loop {
                let (effect, span) = self.ident("an effect name")?;
                effects.push((effect, span));
                if self.at(&Tok::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let body = match self.peek() {
            Tok::Assign => {
                self.bump();
                self.expr(true)?
            }
            Tok::LBrace => {
                let block = self.block()?;
                let span = block.span;
                self.mk(ExprKind::Block(block), span)
            }
            _ => return self.unexpected("`=` or `{` to start the function body"),
        };
        Ok(Function { notes, name, name_span, params, ret, effects, body })
    }

    fn type_expr(&mut self) -> PResult<TypeExpr> {
        let (name, span) = self.ident("a type")?;
        let mut args = Vec::new();
        let mut end = span;
        if self.at(&Tok::LBracket) {
            self.bump();
            loop {
                args.push(self.type_expr()?);
                if self.at(&Tok::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
            end = self.expect(Tok::RBracket)?;
        }
        Ok(TypeExpr { name, args, span: span.to(end) })
    }

    fn fields_block(&mut self) -> PResult<Vec<Field>> {
        self.expect(Tok::LBrace)?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) {
                self.bump();
                break;
            }
            let (name, span) = self.ident("a field name")?;
            self.expect(Tok::Colon)?;
            let ty = self.type_expr()?;
            fields.push(Field { name, span, ty });
            self.end_of_line("a field")?;
        }
        Ok(fields)
    }

    fn record(&mut self, notes: Vec<Note>) -> PResult<RecordDecl> {
        self.expect(Tok::Type)?;
        let (name, name_span) = self.ident("a type name")?;
        let fields = self.fields_block()?;
        Ok(RecordDecl { notes, name, name_span, fields })
    }

    fn enum_decl(&mut self, notes: Vec<Note>) -> PResult<EnumDecl> {
        self.expect(Tok::Enum)?;
        let (name, name_span) = self.ident("an enum name")?;
        self.expect(Tok::LBrace)?;
        let mut variants = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) {
                self.bump();
                break;
            }
            let (vname, vspan) = self.ident("a variant name")?;
            let mut fields = Vec::new();
            if self.at(&Tok::LParen) {
                self.bump();
                while !self.at(&Tok::RParen) {
                    let (fname, fspan) = self.ident("a field name")?;
                    self.expect(Tok::Colon)?;
                    let ty = self.type_expr()?;
                    fields.push(Field { name: fname, span: fspan, ty });
                    if self.at(&Tok::Comma) {
                        self.bump();
                    } else if !self.at(&Tok::RParen) {
                        return self.unexpected("`,` or `)`");
                    }
                }
                self.expect(Tok::RParen)?;
            }
            variants.push(VariantDecl { name: vname, span: vspan, fields });
            self.end_of_line("a variant")?;
        }
        Ok(EnumDecl { notes, name, name_span, variants })
    }

    // ---------- statements ----------

    fn block(&mut self) -> PResult<Block> {
        let start = self.expect(Tok::LBrace)?;
        let mut stmts = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) {
                break;
            }
            if self.at(&Tok::Eof) {
                return self.unexpected("`}`");
            }
            stmts.push(self.statement()?);
            self.end_of_line("a statement")?;
        }
        let end = self.expect(Tok::RBrace)?;
        let tail = match stmts.last() {
            Some(Stmt::Expr(_)) => match stmts.pop() {
                Some(Stmt::Expr(e)) => Some(Box::new(e)),
                _ => None,
            },
            _ => None,
        };
        Ok(Block { stmts, tail, span: start.to(end) })
    }

    fn statement(&mut self) -> PResult<Stmt> {
        match self.peek() {
            Tok::Let | Tok::Var => {
                let mutable = self.bump().tok == Tok::Var;
                let (name, name_span) = match self.peek().clone() {
                    Tok::Underscore => (None, self.bump().span),
                    Tok::Ident(n) => (Some(n), self.bump().span),
                    _ => return self.unexpected("a name"),
                };
                let ty = if self.at(&Tok::Colon) {
                    self.bump();
                    Some(self.type_expr()?)
                } else {
                    None
                };
                self.expect(Tok::Assign)?;
                let value = self.expr(true)?;
                Ok(Stmt::Let { name, name_span, ty, value, mutable })
            }
            Tok::Set => {
                self.bump();
                let (name, name_span) = self.ident("the name of a `var`")?;
                if !self.at(&Tok::Assign) {
                    return self.unexpected("`=`");
                }
                self.bump();
                let value = self.expr(true)?;
                Ok(Stmt::Set { name, name_span, value })
            }
            Tok::For => {
                self.bump();
                let (var, var_span) = self.ident("a loop variable name")?;
                self.expect(Tok::In)?;
                let iterable = self.expr(false)?;
                let body = self.block()?;
                Ok(Stmt::For { var, var_span, iterable, body })
            }
            Tok::While => {
                self.bump();
                let cond = self.expr(false)?;
                let body = self.block()?;
                Ok(Stmt::While { cond, body })
            }
            Tok::Ident(_) if *self.peek_at(1) == Tok::Assign => {
                let span = self.span();
                self.error(Diagnostic::new("E0110", span, "assignment needs `set`").help("declare the variable with `var` and change it with `set name = value`"))
            }
            _ => Ok(Stmt::Expr(self.expr(true)?)),
        }
    }

    // ---------- expressions ----------

    /// `allow_record`: false in `if`/`while`/`for`/`match` headers, where `{` starts the body.
    fn expr(&mut self, allow_record: bool) -> PResult<Expr> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            let span = self.span();
            self.depth -= 1;
            return self.error(Diagnostic::new("E0107", span, "expression is nested too deeply").help("split it into smaller functions or `let` bindings"));
        }
        let result = self.logical(allow_record);
        self.depth -= 1;
        result
    }

    fn logical(&mut self, r: bool) -> PResult<Expr> {
        let mut left = self.comparison(r)?;
        let op = match self.peek() {
            Tok::And => BinaryOp::And,
            Tok::Or => BinaryOp::Or,
            _ => return Ok(left),
        };
        let op_tok = if op == BinaryOp::And { Tok::And } else { Tok::Or };
        let other = if op == BinaryOp::And { Tok::Or } else { Tok::And };
        while *self.peek() == op_tok {
            let op_span = self.bump().span;
            let right = self.comparison(r)?;
            let span = left.span.to(right.span);
            left = self.mk(ExprKind::Binary { op, op_span, left: Box::new(left), right: Box::new(right) }, span);
        }
        if *self.peek() == other {
            let span = self.span();
            return self.error(
                Diagnostic::new("E0105", span, "`and` and `or` cannot be mixed without parentheses")
                    .help("add parentheses to show the intended grouping, e.g. `(a and b) or c`"),
            );
        }
        Ok(left)
    }

    fn compare_op(&self) -> Option<BinaryOp> {
        Some(match self.peek() {
            Tok::EqEq => BinaryOp::Eq,
            Tok::NotEq => BinaryOp::NotEq,
            Tok::Lt => BinaryOp::Lt,
            Tok::LtEq => BinaryOp::LtEq,
            Tok::Gt => BinaryOp::Gt,
            Tok::GtEq => BinaryOp::GtEq,
            _ => return None,
        })
    }

    fn comparison(&mut self, r: bool) -> PResult<Expr> {
        let starts_with_not = self.at(&Tok::Not);
        let left = self.additive(r)?;
        let Some(op) = self.compare_op() else { return Ok(left) };
        if starts_with_not {
            let span = left.span;
            return self.error(
                Diagnostic::new("E0105", span, "`not` before a comparison is ambiguous")
                    .help("write `not (a == b)` or `(not a) == b`"),
            );
        }
        let op_span = self.bump().span;
        let right = self.additive(r)?;
        if self.compare_op().is_some() {
            let span = self.span();
            return self.error(
                Diagnostic::new("E0104", span, "comparisons cannot be chained")
                    .help("combine separate comparisons with `and`, e.g. `a < b and b < c`"),
            );
        }
        let span = left.span.to(right.span);
        Ok(self.mk(ExprKind::Binary { op, op_span, left: Box::new(left), right: Box::new(right) }, span))
    }

    fn additive(&mut self, r: bool) -> PResult<Expr> {
        let mut left = self.multiplicative(r)?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinaryOp::Add,
                Tok::Minus => BinaryOp::Sub,
                _ => return Ok(left),
            };
            let op_span = self.bump().span;
            let right = self.multiplicative(r)?;
            let span = left.span.to(right.span);
            left = self.mk(ExprKind::Binary { op, op_span, left: Box::new(left), right: Box::new(right) }, span);
        }
    }

    fn multiplicative(&mut self, r: bool) -> PResult<Expr> {
        let mut left = self.unary(r)?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinaryOp::Mul,
                Tok::Slash => BinaryOp::Div,
                Tok::Percent => BinaryOp::Rem,
                _ => return Ok(left),
            };
            let op_span = self.bump().span;
            let right = self.unary(r)?;
            let span = left.span.to(right.span);
            left = self.mk(ExprKind::Binary { op, op_span, left: Box::new(left), right: Box::new(right) }, span);
        }
    }

    fn unary(&mut self, r: bool) -> PResult<Expr> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            let span = self.span();
            self.depth -= 1;
            return self.error(Diagnostic::new("E0107", span, "expression is nested too deeply"));
        }
        let result = self.unary_inner(r);
        self.depth -= 1;
        result
    }

    fn unary_inner(&mut self, r: bool) -> PResult<Expr> {
        match self.peek().clone() {
            Tok::Minus => {
                let start = self.bump().span;
                if let Tok::Int(v) = *self.peek() {
                    let end = self.bump().span;
                    let value = if v == 1u64 << 63 { i64::MIN } else { -(v as i64) };
                    let e = self.mk(ExprKind::Int(value), start.to(end));
                    return self.postfix_ops(e);
                }
                let operand = self.unary(r)?;
                let span = start.to(operand.span);
                Ok(self.mk(ExprKind::Unary { op: UnaryOp::Neg, operand: Box::new(operand) }, span))
            }
            Tok::Not => {
                let start = self.bump().span;
                let operand = self.unary(r)?;
                let span = start.to(operand.span);
                Ok(self.mk(ExprKind::Unary { op: UnaryOp::Not, operand: Box::new(operand) }, span))
            }
            _ => {
                let e = self.primary(r)?;
                self.postfix_ops(e)
            }
        }
    }

    fn postfix_ops(&mut self, mut e: Expr) -> PResult<Expr> {
        loop {
            match self.peek() {
                Tok::LBracket => {
                    self.bump();
                    let index = self.expr(true)?;
                    let end = self.expect(Tok::RBracket)?;
                    let span = e.span.to(end);
                    e = self.mk(ExprKind::Index { target: Box::new(e), index: Box::new(index) }, span);
                }
                Tok::Dot => {
                    self.bump();
                    let (name, name_span) = self.ident("a field name")?;
                    if self.at(&Tok::LParen) {
                        let span = self.span();
                        return self.error(
                            Diagnostic::new("E0110", span, "Tessera has no method calls")
                                .help(format!("call the function with the value as first argument, e.g. `{}(value, ...)`", name)),
                        );
                    }
                    let span = e.span.to(name_span);
                    e = self.mk(ExprKind::Field { target: Box::new(e), name, name_span }, span);
                }
                _ => return Ok(e),
            }
        }
    }

    fn args(&mut self, close: Tok) -> PResult<(Vec<Expr>, Span)> {
        let mut args = Vec::new();
        while *self.peek() != close {
            args.push(self.expr(true)?);
            if self.at(&Tok::Comma) {
                self.bump();
            } else if *self.peek() != close {
                let expected = format!("`,` or `{}`", close.symbol());
                return self.unexpected(&expected);
            }
        }
        let end = self.expect(close)?;
        Ok((args, end))
    }

    fn primary(&mut self, r: bool) -> PResult<Expr> {
        let start = self.span();
        match self.peek().clone() {
            Tok::Int(v) => {
                self.bump();
                if v > i64::MAX as u64 {
                    return self.error(Diagnostic::new("E0106", start, "number does not fit in `Int`").help("the largest `Int` is 9223372036854775807"));
                }
                Ok(self.mk(ExprKind::Int(v as i64), start))
            }
            Tok::Text(s) => {
                self.bump();
                Ok(self.mk(ExprKind::Text(s), start))
            }
            Tok::True | Tok::False => {
                let value = self.bump().tok == Tok::True;
                Ok(self.mk(ExprKind::Bool(value), start))
            }
            Tok::LParen => {
                self.bump();
                if self.at(&Tok::RParen) {
                    let end = self.bump().span;
                    return Ok(self.mk(ExprKind::Unit, start.to(end)));
                }
                let mut inner = self.expr(true)?;
                let end = self.expect(Tok::RParen)?;
                inner.span = start.to(end);
                Ok(inner)
            }
            Tok::LBracket => {
                self.bump();
                let (items, end) = self.args(Tok::RBracket)?;
                Ok(self.mk(ExprKind::List(items), start.to(end)))
            }
            Tok::LBrace => {
                if !r {
                    return self.error(Diagnostic::new("E0110", start, "a block cannot start here").help("wrap the block in parentheses"));
                }
                let block = self.block()?;
                let span = block.span;
                Ok(self.mk(ExprKind::Block(block), span))
            }
            Tok::If => self.if_expr(),
            Tok::Match => self.match_expr(),
            Tok::Return => {
                self.bump();
                let value = match self.peek() {
                    Tok::Newline | Tok::RBrace | Tok::RParen | Tok::RBracket | Tok::Comma | Tok::Eof => None,
                    _ => Some(Box::new(self.expr(r)?)),
                };
                let span = value.as_ref().map_or(start, |v| start.to(v.span));
                Ok(self.mk(ExprKind::Return(value), span))
            }
            Tok::Break => {
                self.bump();
                Ok(self.mk(ExprKind::Break, start))
            }
            Tok::Continue => {
                self.bump();
                Ok(self.mk(ExprKind::Continue, start))
            }
            Tok::Ident(name) => {
                if is_type_name(&name) {
                    self.bump();
                    if self.at(&Tok::Dot) {
                        self.bump();
                        let (variant, variant_span) = self.ident("a variant name")?;
                        let mut args = Vec::new();
                        let mut end = variant_span;
                        if self.at(&Tok::LParen) {
                            self.bump();
                            let (a, e) = self.args(Tok::RParen)?;
                            args = a;
                            end = e;
                        }
                        return Ok(self.mk(ExprKind::Variant { type_name: name, variant, variant_span, args }, start.to(end)));
                    }
                    if self.at(&Tok::LBrace) && r {
                        return self.record_literal(name, start);
                    }
                    if self.at(&Tok::LBrace) {
                        return self.error(
                            Diagnostic::new("E0110", start, format!("a record literal cannot appear here without parentheses"))
                                .help(format!("in `if`, `while`, `for` and `match` headers `{{` starts the body; write `({} {{ ... }})`", name)),
                        );
                    }
                    return self.error(
                        Diagnostic::new("E0110", start, format!("type name `{}` cannot be used as a value", name))
                            .help(format!("build a record with `{} {{ field: value }}` or an enum value with `{}.Variant`", name, name)),
                    );
                }
                self.bump();
                if self.at(&Tok::LParen) {
                    self.bump();
                    let (args, end) = self.args(Tok::RParen)?;
                    return Ok(self.mk(ExprKind::Call { name, name_span: start, args }, start.to(end)));
                }
                Ok(self.mk(ExprKind::Name(name), start))
            }
            _ => self.unexpected("an expression"),
        }
    }

    fn record_literal(&mut self, type_name: String, start: Span) -> PResult<Expr> {
        self.expect(Tok::LBrace)?;
        let mut fields = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) {
                break;
            }
            let (name, span) = self.ident("a field name")?;
            self.expect(Tok::Colon)?;
            let value = self.expr(true)?;
            fields.push(FieldInit { name, span, value });
            self.skip_newlines();
            if self.at(&Tok::Comma) {
                self.bump();
            } else if !self.at(&Tok::RBrace) {
                return self.unexpected("`,` or `}`");
            }
        }
        let end = self.expect(Tok::RBrace)?;
        Ok(self.mk(ExprKind::Record { type_name, type_span: start, fields }, start.to(end)))
    }

    fn if_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(Tok::If)?;
        let cond = self.expr(false)?;
        let then_block = self.block()?;
        let mut end = then_block.span;
        let else_branch = if self.at(&Tok::Else) {
            self.bump();
            let e = if self.at(&Tok::If) {
                self.if_expr()?
            } else {
                let block = self.block()?;
                let span = block.span;
                self.mk(ExprKind::Block(block), span)
            };
            end = e.span;
            Some(Box::new(e))
        } else {
            if self.at(&Tok::Newline) && *self.peek_at(1) == Tok::Else {
                let span = self.toks[self.pos + 1].span;
                return self.error(Diagnostic::new("E0111", span, "`else` must be on the same line as the closing `}`").help("write `} else {`"));
            }
            None
        };
        Ok(self.mk(ExprKind::If { cond: Box::new(cond), then_block, else_branch }, start.to(end)))
    }

    fn match_expr(&mut self) -> PResult<Expr> {
        let start = self.expect(Tok::Match)?;
        let scrutinee = self.expr(false)?;
        self.expect(Tok::LBrace)?;
        let mut arms = Vec::new();
        loop {
            self.skip_newlines();
            if self.at(&Tok::RBrace) {
                break;
            }
            let pattern = self.pattern()?;
            self.expect(Tok::FatArrow)?;
            let body = self.expr(true)?;
            arms.push(Arm { pattern, body });
            self.end_of_line("a match arm")?;
        }
        let end = self.expect(Tok::RBrace)?;
        Ok(self.mk(ExprKind::Match { scrutinee: Box::new(scrutinee), arms }, start.to(end)))
    }

    fn pattern(&mut self) -> PResult<Pattern> {
        let start = self.span();
        let kind = match self.peek().clone() {
            Tok::Underscore => {
                self.bump();
                PatternKind::Wildcard
            }
            Tok::Int(v) => {
                self.bump();
                if v > i64::MAX as u64 {
                    return self.error(Diagnostic::new("E0106", start, "number does not fit in `Int`"));
                }
                PatternKind::Int(v as i64)
            }
            Tok::Minus => {
                self.bump();
                match *self.peek() {
                    Tok::Int(v) => {
                        self.bump();
                        PatternKind::Int(if v == 1u64 << 63 { i64::MIN } else { -(v as i64) })
                    }
                    _ => return self.unexpected("a number"),
                }
            }
            Tok::Text(s) => {
                self.bump();
                PatternKind::Text(s)
            }
            Tok::True => {
                self.bump();
                PatternKind::Bool(true)
            }
            Tok::False => {
                self.bump();
                PatternKind::Bool(false)
            }
            Tok::Ident(type_name) if is_type_name(&type_name) => {
                self.bump();
                self.expect(Tok::Dot)?;
                let (variant, _) = self.ident("a variant name")?;
                let mut bindings = Vec::new();
                if self.at(&Tok::LParen) {
                    self.bump();
                    while !self.at(&Tok::RParen) {
                        match self.peek().clone() {
                            Tok::Underscore => bindings.push((None, self.bump().span)),
                            Tok::Ident(n) => bindings.push((Some(n), self.bump().span)),
                            _ => return self.unexpected("a binding name or `_`"),
                        }
                        if self.at(&Tok::Comma) {
                            self.bump();
                        } else if !self.at(&Tok::RParen) {
                            return self.unexpected("`,` or `)`");
                        }
                    }
                    self.bump();
                }
                PatternKind::Variant { type_name, variant, bindings }
            }
            _ => return self.unexpected("a pattern (`_`, a literal, or `Type.Variant`)"),
        };
        Ok(Pattern { kind, span: start.to(self.prev_span()) })
    }
}
