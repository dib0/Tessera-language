//! Name resolution, type checking, and effect checking.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast::*;
use crate::diag::{Diagnostic, Span};
use crate::types::{Type, TypeDef, EFFECTS};

pub const BUILTINS: &[&str] = &[
    "print",
    "print_error",
    "to_text",
    "length",
    "append",
    "concat",
    "slice",
    "replace_at",
    "range",
    "code_points",
    "from_code_point",
    "from_code_points",
    "join",
    "read_file",
    "write_file",
    "args",
    "exit",
    "fail",
];

pub struct Checked {
    pub expr_types: Vec<Type>,
    pub type_defs: BTreeMap<String, TypeDef>,
}

#[derive(Clone)]
struct FnSig {
    params: Vec<Type>,
    ret: Type,
    effects: Vec<String>,
}

struct Local {
    name: String,
    ty: Type,
    mutable: bool,
    was_set: bool,
    span: Span,
}

struct Checker<'a> {
    diags: &'a mut Vec<Diagnostic>,
    expr_types: Vec<Type>,
    type_defs: BTreeMap<String, TypeDef>,
    fns: HashMap<String, FnSig>,
    scopes: Vec<Vec<Local>>,
    fn_name: String,
    ret: Type,
    declared_effects: Vec<String>,
    used_effects: HashSet<String>,
    loop_depth: usize,
}

pub fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') && !name.ends_with('_') && !name.contains("__")
}

pub fn is_pascal_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_uppercase() => {}
        _ => return false,
    }
    name.chars().all(|c| c.is_ascii_alphanumeric())
}

fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if c == '_' && (out.is_empty() || out.ends_with('_')) {
        } else {
            out.push(c);
        }
    }
    out.trim_end_matches('_').to_string()
}

fn to_pascal_case(name: &str) -> String {
    name.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut cs = p.chars();
            match cs.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

pub fn check(program: &Program, diags: &mut Vec<Diagnostic>) -> Checked {
    let mut c = Checker {
        diags,
        expr_types: vec![Type::Unknown; program.expr_count],
        type_defs: BTreeMap::new(),
        fns: HashMap::new(),
        scopes: Vec::new(),
        fn_name: String::new(),
        ret: Type::Unit,
        declared_effects: Vec::new(),
        used_effects: HashSet::new(),
        loop_depth: 0,
    };
    c.run(program);
    Checked { expr_types: c.expr_types, type_defs: c.type_defs }
}

impl<'a> Checker<'a> {
    fn err(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    fn check_snake(&mut self, name: &str, span: Span, what: &str) {
        if !is_snake_case(name) {
            let suggestion = to_snake_case(name);
            self.err(
                Diagnostic::new("E0207", span, format!("{} `{}` must be written in snake_case", what, name))
                    .help(format!("rename it to `{}`", if suggestion.is_empty() { "a_name".into() } else { suggestion })),
            );
        }
    }

    fn check_pascal(&mut self, name: &str, span: Span, what: &str) {
        if !is_pascal_case(name) {
            self.err(
                Diagnostic::new("E0207", span, format!("{} `{}` must be written in PascalCase", what, name))
                    .help(format!("rename it to `{}`", to_pascal_case(name))),
            );
        }
    }

    // ---------- declarations ----------

    fn run(&mut self, program: &Program) {
        // 1. Collect type names.
        let mut type_spans: HashMap<String, Span> = HashMap::new();
        for item in &program.items {
            let (name, span) = match item {
                Item::Record(r) => (&r.name, r.name_span),
                Item::Enum(e) => (&e.name, e.name_span),
                Item::Function(_) => continue,
            };
            self.check_pascal(name, span, "type");
            if ["Int", "Bool", "Text", "Unit", "List", "Never"].contains(&name.as_str()) {
                self.err(Diagnostic::new("E0201", span, format!("`{}` is a built-in type and cannot be redefined", name)));
                continue;
            }
            if let Some(prev) = type_spans.get(name) {
                let line = prev.line;
                self.err(Diagnostic::new("E0201", span, format!("type `{}` is already defined on line {}", name, line)));
                continue;
            }
            type_spans.insert(name.clone(), span);
            // Placeholder so field types can refer to any declared type.
            self.type_defs.insert(name.clone(), TypeDef::Record { fields: vec![] });
        }

        // 2. Resolve type bodies.
        for item in &program.items {
            match item {
                Item::Record(r) => {
                    if type_spans.get(&r.name) != Some(&r.name_span) {
                        continue;
                    }
                    let fields = self.resolve_fields(&r.fields);
                    self.type_defs.insert(r.name.clone(), TypeDef::Record { fields });
                }
                Item::Enum(e) => {
                    if type_spans.get(&e.name) != Some(&e.name_span) {
                        continue;
                    }
                    let mut variants: Vec<(String, Vec<(String, Type)>)> = Vec::new();
                    for v in &e.variants {
                        self.check_pascal(&v.name, v.span, "variant");
                        if variants.iter().any(|(n, _)| *n == v.name) {
                            self.err(Diagnostic::new("E0201", v.span, format!("variant `{}` is defined twice", v.name)));
                            continue;
                        }
                        let fields = self.resolve_fields(&v.fields);
                        variants.push((v.name.clone(), fields));
                    }
                    if variants.is_empty() {
                        self.err(Diagnostic::new("E0201", e.name_span, format!("enum `{}` has no variants", e.name)));
                    }
                    self.type_defs.insert(e.name.clone(), TypeDef::Enum { variants });
                }
                Item::Function(_) => {}
            }
        }

        // 3. Function signatures.
        let mut fn_spans: HashMap<String, Span> = HashMap::new();
        for item in &program.items {
            let Item::Function(f) = item else { continue };
            self.check_snake(&f.name, f.name_span, "function name");
            if BUILTINS.contains(&f.name.as_str()) {
                self.err(Diagnostic::new("E0211", f.name_span, format!("`{}` is a built-in function and cannot be redefined", f.name)).help("choose a different name"));
                continue;
            }
            if let Some(prev) = fn_spans.get(&f.name) {
                let line = prev.line;
                self.err(
                    Diagnostic::new("E0201", f.name_span, format!("function `{}` is already defined on line {}", f.name, line))
                        .help("Tessera has no overloading; give the function a different name"),
                );
                continue;
            }
            fn_spans.insert(f.name.clone(), f.name_span);
            let params = f.params.iter().map(|p| self.resolve(&p.ty)).collect();
            let ret = self.resolve(&f.ret);
            let mut effects: Vec<String> = Vec::new();
            for (effect, span) in &f.effects {
                if !EFFECTS.contains(&effect.as_str()) {
                    self.err(
                        Diagnostic::new("E0303", *span, format!("unknown effect `{}`", effect))
                            .help(format!("known effects are: {}", EFFECTS.join(", "))),
                    );
                } else if effects.contains(effect) {
                    self.err(Diagnostic::new("E0304", *span, format!("effect `{}` is listed twice", effect)));
                } else {
                    effects.push(effect.clone());
                }
            }
            self.fns.insert(f.name.clone(), FnSig { params, ret, effects });
        }

        // 4. main
        match program.items.iter().find_map(|i| match i {
            Item::Function(f) if f.name == "main" => Some(f),
            _ => None,
        }) {
            None => {
                let span = Span { file: 0, line: 1, col: 1, end_line: 1, end_col: 2 };
                self.err(Diagnostic::new("E0212", span, "the program has no `main` function").help("add `fn main() -> Unit uses io { ... }`"));
            }
            Some(f) => {
                if !f.params.is_empty() || f.ret.name != "Unit" {
                    self.err(
                        Diagnostic::new("E0212", f.name_span, "`main` must take no parameters and return `Unit`")
                            .help("use the `args()` built-in (effect `env`) to read command-line arguments"),
                    );
                }
            }
        }

        // 5. Bodies.
        for item in &program.items {
            if let Item::Function(f) = item {
                if fn_spans.get(&f.name) == Some(&f.name_span) {
                    self.function(f);
                }
            }
        }
    }

    fn resolve_fields(&mut self, fields: &[Field]) -> Vec<(String, Type)> {
        let mut out: Vec<(String, Type)> = Vec::new();
        for f in fields {
            self.check_snake(&f.name, f.span, "field");
            if out.iter().any(|(n, _)| *n == f.name) {
                self.err(Diagnostic::new("E0201", f.span, format!("field `{}` is defined twice", f.name)));
                continue;
            }
            let ty = self.resolve(&f.ty);
            out.push((f.name.clone(), ty));
        }
        out
    }

    fn resolve(&mut self, te: &TypeExpr) -> Type {
        let simple = |t: Type| t;
        let ty = match te.name.as_str() {
            "Int" => simple(Type::Int),
            "Bool" => simple(Type::Bool),
            "Text" => simple(Type::Text),
            "Unit" => simple(Type::Unit),
            "List" => {
                if te.args.len() != 1 {
                    self.err(Diagnostic::new("E0208", te.span, "`List` needs exactly one element type").help("write e.g. `List[Int]`"));
                    return Type::Unknown;
                }
                let inner = self.resolve(&te.args[0]);
                return Type::List(Box::new(inner));
            }
            name if self.type_defs.contains_key(name) => Type::Named(name.to_string()),
            name => {
                let hint = match name {
                    "i64" | "int" | "integer" => Some("Int"),
                    "bool" | "boolean" => Some("Bool"),
                    "str" | "string" | "String" | "text" => Some("Text"),
                    "unit" | "void" => Some("Unit"),
                    "list" | "Vec" | "vec" | "Array" => Some("List[...]"),
                    _ => None,
                };
                let mut d = Diagnostic::new("E0208", te.span, format!("unknown type `{}`", name));
                if let Some(h) = hint {
                    d = d.help(format!("did you mean `{}`?", h));
                }
                self.err(d);
                return Type::Unknown;
            }
        };
        if !te.args.is_empty() {
            self.err(Diagnostic::new("E0208", te.span, format!("type `{}` takes no type arguments", te.name)));
        }
        ty
    }

    fn function(&mut self, f: &Function) {
        let sig = self.fns[&f.name].clone();
        self.fn_name = f.name.clone();
        self.ret = sig.ret.clone();
        self.declared_effects = sig.effects.clone();
        self.used_effects.clear();
        self.loop_depth = 0;
        self.scopes = vec![Vec::new()];
        for (p, ty) in f.params.iter().zip(sig.params.iter()) {
            self.declare(&p.name, p.span, ty.clone(), false, "parameter");
        }
        let body_ty = self.check_expr(&f.body, Some(&sig.ret));
        if !body_ty.fits(&sig.ret) {
            let span = match &f.body.kind {
                ExprKind::Block(b) => b.tail.as_ref().map_or(b.span, |t| t.span),
                _ => f.body.span,
            };
            self.err(
                Diagnostic::new("E0501", span, format!("function `{}` must return `{}`, but its body produces `{}`", f.name, sig.ret, body_ty))
                    .label(format!("this is `{}`", body_ty)),
            );
        }
        self.pop_scope();
        if f.name != "main" {
            for (effect, span) in &f.effects {
                if sig.effects.contains(effect) && !self.used_effects.contains(effect) {
                    self.err(
                        Diagnostic::new("E0302", *span, format!("function `{}` declares effect `{}` but never uses it", f.name, effect))
                            .help(format!("remove `{}` from the `uses` list so the signature does not overstate what the function can do", effect)),
                    );
                }
            }
        }
    }

    // ---------- scopes ----------

    fn lookup(&self, name: &str) -> Option<&Local> {
        self.scopes.iter().rev().flat_map(|s| s.iter().rev()).find(|l| l.name == name)
    }

    fn lookup_mut(&mut self, name: &str) -> Option<&mut Local> {
        self.scopes.iter_mut().rev().flat_map(|s| s.iter_mut().rev()).find(|l| l.name == name)
    }

    fn declare(&mut self, name: &str, span: Span, ty: Type, mutable: bool, what: &str) {
        self.check_snake(name, span, what);
        if let Some(prev) = self.lookup(name) {
            let line = prev.span.line;
            self.err(
                Diagnostic::new("E0204", span, format!("`{}` is already defined on line {}", name, line))
                    .help("Tessera does not allow shadowing; choose a different name"),
            );
            return;
        }
        self.scopes.last_mut().unwrap().push(Local { name: name.to_string(), ty, mutable, was_set: false, span });
    }

    fn pop_scope(&mut self) {
        let scope = self.scopes.pop().unwrap_or_default();
        for local in scope {
            if local.mutable && !local.was_set {
                self.err(
                    Diagnostic::new("E0206", local.span, format!("`{}` is declared with `var` but never changed", local.name))
                        .help("use `let` for values that do not change"),
                );
            }
        }
    }

    // ---------- expressions ----------

    fn record(&mut self, e: &Expr, ty: Type) -> Type {
        self.expr_types[e.id] = ty.clone();
        ty
    }

    fn expect_expr(&mut self, e: &Expr, expected: &Type) -> Type {
        let t = self.check_expr(e, Some(expected));
        if !t.fits(expected) {
            self.err(Diagnostic::new("E0501", e.span, format!("expected `{}`, found `{}`", expected, t)).label(format!("this is `{}`", t)));
        }
        t
    }

    fn check_expr(&mut self, e: &Expr, expected: Option<&Type>) -> Type {
        let ty = self.check_expr_inner(e, expected);
        self.record(e, ty)
    }

    fn check_expr_inner(&mut self, e: &Expr, expected: Option<&Type>) -> Type {
        match &e.kind {
            ExprKind::Int(_) => Type::Int,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Text(_) => Type::Text,
            ExprKind::Unit => Type::Unit,
            ExprKind::Name(name) => match self.lookup(name) {
                Some(local) => local.ty.clone(),
                None => {
                    let mut d = Diagnostic::new("E0202", e.span, format!("unknown name `{}`", name));
                    if self.fns.contains_key(name) || BUILTINS.contains(&name.as_str()) {
                        d = d.help(format!("`{}` is a function; call it with `{}(...)`", name, name));
                    }
                    self.err(d);
                    Type::Unknown
                }
            },
            ExprKind::List(items) => {
                let elem_expected = match expected {
                    Some(Type::List(t)) => Some((**t).clone()),
                    _ => None,
                };
                if items.is_empty() {
                    return match elem_expected {
                        Some(t) => Type::List(Box::new(t)),
                        None => {
                            self.err(
                                Diagnostic::new("E0505", e.span, "the element type of this empty list is unknown")
                                    .help("add a type annotation, e.g. `let names: List[Text] = []`"),
                            );
                            Type::Unknown
                        }
                    };
                }
                let mut elem = match &elem_expected {
                    Some(t) => self.check_expr(&items[0], Some(t)),
                    None => self.check_expr(&items[0], None),
                };
                if let Some(t) = &elem_expected {
                    if elem.fits(t) {
                        elem = t.clone();
                    }
                }
                for item in &items[1..] {
                    let t = self.check_expr(item, Some(&elem));
                    match elem.join(&t) {
                        Some(j) => elem = j,
                        None => self.err(
                            Diagnostic::new("E0501", item.span, format!("list elements must all have the same type: expected `{}`, found `{}`", elem, t))
                                .label(format!("this is `{}`", t)),
                        ),
                    }
                }
                Type::List(Box::new(elem))
            }
            ExprKind::Call { name, name_span, args } => self.check_call(name, *name_span, args, e.span, expected),
            ExprKind::Index { target, index } => {
                let t = self.check_expr(target, None);
                self.expect_expr(index, &Type::Int);
                match t {
                    Type::List(elem) => *elem,
                    Type::Unknown => Type::Unknown,
                    Type::Text => {
                        self.err(
                            Diagnostic::new("E0507", target.span, "`Text` cannot be indexed")
                                .help("use `code_points(text)` to get a `List[Int]` of characters"),
                        );
                        Type::Unknown
                    }
                    other => {
                        self.err(Diagnostic::new("E0507", target.span, format!("only lists can be indexed, found `{}`", other)));
                        Type::Unknown
                    }
                }
            }
            ExprKind::Field { target, name, name_span } => {
                let t = self.check_expr(target, None);
                match &t {
                    Type::Unknown => Type::Unknown,
                    Type::Named(type_name) => match self.type_defs.get(type_name) {
                        Some(TypeDef::Record { fields }) => match fields.iter().find(|(n, _)| n == name) {
                            Some((_, ft)) => ft.clone(),
                            None => {
                                let names: Vec<String> = fields.iter().map(|(n, _)| format!("`{}`", n)).collect();
                                self.err(
                                    Diagnostic::new("E0209", *name_span, format!("type `{}` has no field `{}`", type_name, name))
                                        .help(format!("available fields: {}", names.join(", "))),
                                );
                                Type::Unknown
                            }
                        },
                        _ => {
                            self.err(
                                Diagnostic::new("E0512", *name_span, format!("`{}` is an enum; it has no fields", type_name))
                                    .help("use `match` to read the values inside an enum"),
                            );
                            Type::Unknown
                        }
                    },
                    other => {
                        self.err(Diagnostic::new("E0512", *name_span, format!("`{}` has no fields", other)));
                        Type::Unknown
                    }
                }
            }
            ExprKind::Unary { op, operand } => {
                let want = if *op == UnaryOp::Neg { Type::Int } else { Type::Bool };
                let t = self.check_expr(operand, Some(&want));
                if !t.fits(&want) {
                    let sym = if *op == UnaryOp::Neg { "-" } else { "not" };
                    self.err(Diagnostic::new("E0503", operand.span, format!("`{}` needs `{}`, found `{}`", sym, want, t)));
                }
                want
            }
            ExprKind::Binary { op, left, right, .. } => self.check_binary(*op, left, right, e.span),
            ExprKind::If { cond, then_block, else_branch } => {
                self.check_condition(cond, "if");
                match else_branch {
                    None => {
                        let t = self.check_block(then_block, Some(&Type::Unit));
                        if !t.is_unit_like() {
                            let span = then_block.tail.as_ref().map_or(then_block.span, |t| t.span);
                            self.err(
                                Diagnostic::new("E0506", span, format!("this `if` has no `else`, so its block cannot produce a value (found `{}`)", t))
                                    .help("add an `else` branch, or use `let _ = ...` to discard the value"),
                            );
                        }
                        Type::Unit
                    }
                    Some(else_expr) => {
                        let then_t = self.check_block(then_block, expected);
                        let hint = expected.cloned().or_else(|| if then_t == Type::Never { None } else { Some(then_t.clone()) });
                        let else_t = self.check_expr(else_expr, hint.as_ref());
                        match then_t.join(&else_t) {
                            Some(t) => t,
                            None => {
                                self.err(
                                    Diagnostic::new("E0506", else_expr.span, format!("`if` and `else` have different types: `{}` and `{}`", then_t, else_t))
                                        .label(format!("this is `{}`", else_t)),
                                );
                                Type::Unknown
                            }
                        }
                    }
                }
            }
            ExprKind::Block(block) => self.check_block(block, expected),
            ExprKind::Match { scrutinee, arms } => self.check_match(scrutinee, arms, e.span, expected),
            ExprKind::Record { type_name, type_span, fields } => self.check_record_literal(type_name, *type_span, fields, e.span),
            ExprKind::Variant { type_name, variant, variant_span, args } => {
                let Some(def) = self.type_defs.get(type_name).cloned() else {
                    self.err(Diagnostic::new("E0208", e.span, format!("unknown type `{}`", type_name)));
                    for a in args {
                        self.check_expr(a, None);
                    }
                    return Type::Unknown;
                };
                let TypeDef::Enum { variants } = def else {
                    self.err(
                        Diagnostic::new("E0209", e.span, format!("`{}` is a record, not an enum", type_name))
                            .help(format!("build it with `{} {{ field: value }}`", type_name)),
                    );
                    return Type::Unknown;
                };
                let Some((_, fields)) = variants.iter().find(|(n, _)| n == variant) else {
                    let names: Vec<String> = variants.iter().map(|(n, _)| format!("`{}`", n)).collect();
                    self.err(
                        Diagnostic::new("E0209", *variant_span, format!("enum `{}` has no variant `{}`", type_name, variant))
                            .help(format!("available variants: {}", names.join(", "))),
                    );
                    for a in args {
                        self.check_expr(a, None);
                    }
                    return Type::Unknown;
                };
                if args.len() != fields.len() {
                    self.err(Diagnostic::new(
                        "E0502",
                        e.span,
                        format!("`{}.{}` takes {} value(s), but {} were given", type_name, variant, fields.len(), args.len()),
                    ));
                    for a in args {
                        self.check_expr(a, None);
                    }
                } else {
                    for (a, (_, ft)) in args.iter().zip(fields.iter()) {
                        self.expect_expr(a, ft);
                    }
                }
                Type::Named(type_name.clone())
            }
            ExprKind::Return(value) => {
                let ret = self.ret.clone();
                match value {
                    Some(v) => {
                        let t = self.check_expr(v, Some(&ret));
                        if !t.fits(&ret) {
                            self.err(Diagnostic::new("E0501", v.span, format!("function `{}` returns `{}`, but this is `{}`", self.fn_name, ret, t)));
                        }
                    }
                    None => {
                        if !Type::Unit.fits(&ret) {
                            self.err(Diagnostic::new("E0501", e.span, format!("function `{}` returns `{}`; `return` needs a value", self.fn_name, ret)));
                        }
                    }
                }
                Type::Never
            }
            ExprKind::Break | ExprKind::Continue => {
                if self.loop_depth == 0 {
                    let word = if matches!(e.kind, ExprKind::Break) { "break" } else { "continue" };
                    self.err(Diagnostic::new("E0210", e.span, format!("`{}` can only be used inside a loop", word)));
                }
                Type::Never
            }
        }
    }

    fn check_condition(&mut self, cond: &Expr, what: &str) {
        let t = self.check_expr(cond, Some(&Type::Bool));
        if !t.fits(&Type::Bool) {
            self.err(
                Diagnostic::new("E0504", cond.span, format!("the condition of `{}` must be `Bool`, found `{}`", what, t))
                    .help("Tessera has no truthiness; write a comparison such as `count != 0`"),
            );
        }
    }

    fn check_block(&mut self, block: &Block, expected: Option<&Type>) -> Type {
        self.scopes.push(Vec::new());
        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }
        let t = match &block.tail {
            Some(tail) => {
                let t = self.check_expr(tail, expected);
                if expected == Some(&Type::Unit) && !t.is_unit_like() {
                    // Reported by the caller with better context.
                }
                t
            }
            None => Type::Unit,
        };
        self.pop_scope();
        t
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, name_span, ty, value, mutable } => {
                let declared = ty.as_ref().map(|t| self.resolve(t));
                let t = match &declared {
                    Some(d) => {
                        self.expect_expr(value, d);
                        d.clone()
                    }
                    None => self.check_expr(value, None),
                };
                if t == Type::Unit && declared.is_none() {
                    self.err(Diagnostic::new("E0508", value.span, "this expression produces no value (`Unit`)").help("call it as a statement instead of binding it"));
                }
                match name {
                    Some(n) => self.declare(n, *name_span, t, *mutable, "variable"),
                    None => {
                        if *mutable {
                            self.err(Diagnostic::new("E0110", *name_span, "`var _` is not allowed").help("use `let _ = ...` to discard a value"));
                        }
                    }
                }
            }
            Stmt::Set { name, name_span, value } => {
                let target = self.lookup(name).map(|l| (l.ty.clone(), l.mutable, l.span));
                match target {
                    None => {
                        self.err(Diagnostic::new("E0202", *name_span, format!("unknown name `{}`", name)));
                        self.check_expr(value, None);
                    }
                    Some((ty, mutable, span)) => {
                        if !mutable {
                            self.err(
                                Diagnostic::new("E0205", *name_span, format!("`{}` cannot be changed", name))
                                    .help(format!("declare it with `var` instead of `let` (line {}); parameters can never be changed", span.line)),
                            );
                        } else if let Some(l) = self.lookup_mut(name) {
                            l.was_set = true;
                        }
                        self.expect_expr(value, &ty);
                    }
                }
            }
            Stmt::For { var, var_span, iterable, body } => {
                let t = self.check_expr(iterable, None);
                let elem = match t {
                    Type::List(elem) => *elem,
                    Type::Unknown => Type::Unknown,
                    other => {
                        let mut d = Diagnostic::new("E0507", iterable.span, format!("`for` needs a list, found `{}`", other));
                        if other == Type::Text {
                            d = d.help("use `for code in code_points(text)` to loop over characters");
                        } else if other == Type::Int {
                            d = d.help("use `range(0, n)` to loop over numbers");
                        }
                        self.err(d);
                        Type::Unknown
                    }
                };
                self.scopes.push(Vec::new());
                self.declare(var, *var_span, elem, false, "loop variable");
                self.loop_depth += 1;
                self.check_loop_body(body, "for");
                self.loop_depth -= 1;
                self.pop_scope();
            }
            Stmt::While { cond, body } => {
                self.check_condition(cond, "while");
                self.loop_depth += 1;
                self.check_loop_body(body, "while");
                self.loop_depth -= 1;
            }
            Stmt::Expr(e) => {
                let t = self.check_expr(e, None);
                if !t.is_unit_like() {
                    self.err(
                        Diagnostic::new("E0508", e.span, format!("the value of this expression (`{}`) is not used", t))
                            .help("use the value, or write `let _ = ...` to discard it explicitly"),
                    );
                }
            }
        }
    }

    fn check_loop_body(&mut self, body: &Block, what: &str) {
        let t = self.check_block(body, Some(&Type::Unit));
        if !t.is_unit_like() {
            let span = body.tail.as_ref().map_or(body.span, |t| t.span);
            self.err(
                Diagnostic::new("E0508", span, format!("the body of `{}` must not produce a value (found `{}`)", what, t))
                    .help("write `let _ = ...` to discard the value"),
            );
        }
    }

    fn check_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr, span: Span) -> Type {
        match op {
            BinaryOp::Add => {
                let lt = self.check_expr(left, None);
                let rt = self.check_expr(right, Some(&lt));
                match (&lt, &rt) {
                    (Type::Int, Type::Int) => Type::Int,
                    (Type::Text, Type::Text) => Type::Text,
                    (Type::Unknown, t) | (t, Type::Unknown) if matches!(t, Type::Int | Type::Text | Type::Unknown) => t.clone(),
                    _ => {
                        let mut d = Diagnostic::new("E0503", span, format!("`+` cannot combine `{}` and `{}`", lt, rt));
                        if (lt == Type::Text && rt == Type::Int) || (lt == Type::Int && rt == Type::Text) {
                            d = d.help("convert the number with `to_text(...)`; Tessera has no implicit conversions");
                        } else if matches!(lt, Type::List(_)) {
                            d = d.help("use `concat(a, b)` to join lists or `append(list, item)` to add one item");
                        }
                        self.err(d);
                        Type::Unknown
                    }
                }
            }
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                for side in [left, right] {
                    let t = self.check_expr(side, Some(&Type::Int));
                    if !t.fits(&Type::Int) {
                        self.err(Diagnostic::new("E0503", side.span, format!("`{}` needs `Int`, found `{}`", op.symbol(), t)));
                    }
                }
                Type::Int
            }
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => {
                for side in [left, right] {
                    let t = self.check_expr(side, Some(&Type::Int));
                    if !t.fits(&Type::Int) {
                        self.err(Diagnostic::new("E0514", side.span, format!("`{}` compares numbers only, found `{}`", op.symbol(), t)));
                    }
                }
                Type::Bool
            }
            BinaryOp::Eq | BinaryOp::NotEq => {
                let lt = self.check_expr(left, None);
                let rt = self.check_expr(right, Some(&lt));
                if lt.join(&rt).is_none() {
                    self.err(Diagnostic::new("E0514", span, format!("cannot compare `{}` with `{}`", lt, rt)));
                }
                Type::Bool
            }
            BinaryOp::And | BinaryOp::Or => {
                for side in [left, right] {
                    let t = self.check_expr(side, Some(&Type::Bool));
                    if !t.fits(&Type::Bool) {
                        self.err(Diagnostic::new("E0503", side.span, format!("`{}` needs `Bool`, found `{}`", op.symbol(), t)));
                    }
                }
                Type::Bool
            }
        }
    }

    fn check_record_literal(&mut self, type_name: &str, type_span: Span, inits: &[FieldInit], span: Span) -> Type {
        let fields = match self.type_defs.get(type_name).cloned() {
            Some(TypeDef::Record { fields }) => fields,
            Some(TypeDef::Enum { .. }) => {
                self.err(
                    Diagnostic::new("E0511", type_span, format!("`{}` is an enum, not a record", type_name))
                        .help(format!("build an enum value with `{}.Variant(...)`", type_name)),
                );
                for i in inits {
                    self.check_expr(&i.value, None);
                }
                return Type::Unknown;
            }
            None => {
                self.err(Diagnostic::new("E0208", type_span, format!("unknown type `{}`", type_name)));
                for i in inits {
                    self.check_expr(&i.value, None);
                }
                return Type::Unknown;
            }
        };
        let mut seen: Vec<&str> = Vec::new();
        for init in inits {
            match fields.iter().find(|(n, _)| *n == init.name) {
                Some((_, ft)) => {
                    if seen.contains(&init.name.as_str()) {
                        self.err(Diagnostic::new("E0511", init.span, format!("field `{}` is given twice", init.name)));
                    }
                    seen.push(&init.name);
                    self.expect_expr(&init.value, ft);
                }
                None => {
                    self.err(Diagnostic::new("E0209", init.span, format!("type `{}` has no field `{}`", type_name, init.name)));
                    self.check_expr(&init.value, None);
                }
            }
        }
        let missing: Vec<String> = fields.iter().filter(|(n, _)| !seen.contains(&n.as_str())).map(|(n, _)| format!("`{}`", n)).collect();
        if !missing.is_empty() {
            self.err(
                Diagnostic::new("E0511", span, format!("missing field(s) {} in `{}`", missing.join(", "), type_name))
                    .help("every field must be given a value"),
            );
        }
        Type::Named(type_name.to_string())
    }

    fn check_match(&mut self, scrutinee: &Expr, arms: &[Arm], span: Span, expected: Option<&Type>) -> Type {
        let st = self.check_expr(scrutinee, None);
        let enum_variants: Option<Vec<(String, Vec<(String, Type)>)>> = match &st {
            Type::Named(n) => match self.type_defs.get(n) {
                Some(TypeDef::Enum { variants }) => Some(variants.clone()),
                _ => None,
            },
            _ => None,
        };
        match &st {
            Type::Int | Type::Text | Type::Bool | Type::Unknown => {}
            Type::Named(_) if enum_variants.is_some() => {}
            other => {
                self.err(Diagnostic::new("E0513", scrutinee.span, format!("cannot `match` on `{}`", other)).help("`match` works on enums, `Int`, `Text` and `Bool`"));
            }
        }
        let mut result: Option<Type> = None;
        let mut covered: Vec<String> = Vec::new();
        let mut seen_true = false;
        let mut seen_false = false;
        let mut has_wildcard = false;
        let mut seen_literals: Vec<String> = Vec::new();
        for arm in arms {
            if has_wildcard {
                self.err(Diagnostic::new("E0510", arm.pattern.span, "this arm can never match: an earlier `_` arm matches everything").help("remove this arm or move it above the `_` arm"));
            }
            self.scopes.push(Vec::new());
            match &arm.pattern.kind {
                PatternKind::Wildcard => has_wildcard = true,
                PatternKind::Int(v) => {
                    self.pattern_type(&st, &Type::Int, arm.pattern.span);
                    self.duplicate_literal(&mut seen_literals, v.to_string(), arm.pattern.span);
                }
                PatternKind::Text(s) => {
                    self.pattern_type(&st, &Type::Text, arm.pattern.span);
                    self.duplicate_literal(&mut seen_literals, format!("{:?}", s), arm.pattern.span);
                }
                PatternKind::Bool(b) => {
                    self.pattern_type(&st, &Type::Bool, arm.pattern.span);
                    if (*b && seen_true) || (!*b && seen_false) {
                        self.err(Diagnostic::new("E0510", arm.pattern.span, "this value is already matched by an earlier arm"));
                    }
                    if *b {
                        seen_true = true
                    } else {
                        seen_false = true
                    }
                }
                PatternKind::Variant { type_name, variant, bindings } => {
                    let fields = match (&st, &enum_variants) {
                        (Type::Named(n), Some(variants)) if n == type_name => match variants.iter().find(|(v, _)| v == variant) {
                            Some((_, fields)) => Some(fields.clone()),
                            None => {
                                let names: Vec<String> = variants.iter().map(|(n, _)| format!("`{}`", n)).collect();
                                self.err(
                                    Diagnostic::new("E0209", arm.pattern.span, format!("enum `{}` has no variant `{}`", type_name, variant))
                                        .help(format!("available variants: {}", names.join(", "))),
                                );
                                None
                            }
                        },
                        (Type::Unknown, _) => None,
                        _ => {
                            self.err(Diagnostic::new("E0513", arm.pattern.span, format!("pattern of type `{}` cannot match a value of type `{}`", type_name, st)));
                            None
                        }
                    };
                    if let Some(fields) = fields {
                        if covered.contains(variant) {
                            self.err(Diagnostic::new("E0510", arm.pattern.span, format!("`{}.{}` is already matched by an earlier arm", type_name, variant)));
                        }
                        covered.push(variant.clone());
                        if bindings.len() != fields.len() {
                            self.err(
                                Diagnostic::new(
                                    "E0513",
                                    arm.pattern.span,
                                    format!("`{}.{}` has {} field(s), but the pattern binds {}", type_name, variant, fields.len(), bindings.len()),
                                )
                                .help(format!(
                                    "write `{}.{}({})`",
                                    type_name,
                                    variant,
                                    fields.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
                                )),
                            );
                        }
                        for ((binding, bspan), (_, fty)) in bindings.iter().zip(fields.iter()) {
                            if let Some(b) = binding {
                                self.declare(b, *bspan, fty.clone(), false, "binding");
                            }
                        }
                    } else {
                        for (binding, bspan) in bindings {
                            if let Some(b) = binding {
                                self.declare(b, *bspan, Type::Unknown, false, "binding");
                            }
                        }
                    }
                }
            }
            let hint = expected.cloned().or_else(|| result.clone().filter(|t| *t != Type::Never));
            let t = self.check_expr(&arm.body, hint.as_ref());
            self.pop_scope();
            result = match &result {
                None => Some(t),
                Some(prev) => match prev.join(&t) {
                    Some(j) => Some(j),
                    None => {
                        self.err(
                            Diagnostic::new("E0506", arm.body.span, format!("match arms have different types: `{}` and `{}`", prev, t))
                                .label(format!("this is `{}`", t)),
                        );
                        Some(Type::Unknown)
                    }
                },
            };
        }
        if !has_wildcard {
            match &st {
                Type::Unknown => {}
                Type::Bool => {
                    if !(seen_true && seen_false) {
                        self.err(Diagnostic::new("E0509", span, "`match` does not cover all values").help("add the missing `true`/`false` arm or a `_` arm"));
                    }
                }
                Type::Named(n) => {
                    if let Some(variants) = &enum_variants {
                        let missing: Vec<String> = variants.iter().filter(|(v, _)| !covered.contains(v)).map(|(v, _)| format!("`{}.{}`", n, v)).collect();
                        if !missing.is_empty() {
                            self.err(
                                Diagnostic::new("E0509", span, format!("`match` does not cover {}", missing.join(", ")))
                                    .help("add an arm for each missing variant (preferred) or a `_` arm"),
                            );
                        }
                    }
                }
                _ => {
                    self.err(Diagnostic::new("E0509", span, format!("`match` on `{}` needs a `_` arm", st)).help("add `_ => ...` as the last arm"));
                }
            }
        }
        if arms.is_empty() {
            return Type::Never;
        }
        result.unwrap_or(Type::Unknown)
    }

    fn pattern_type(&mut self, scrutinee: &Type, pattern: &Type, span: Span) {
        if !pattern.fits(scrutinee) {
            self.err(Diagnostic::new("E0513", span, format!("pattern of type `{}` cannot match a value of type `{}`", pattern, scrutinee)));
        }
    }

    fn duplicate_literal(&mut self, seen: &mut Vec<String>, key: String, span: Span) {
        if seen.contains(&key) {
            self.err(Diagnostic::new("E0510", span, "this value is already matched by an earlier arm"));
        }
        seen.push(key);
    }

    // ---------- calls ----------

    fn use_effect(&mut self, effect: &str, callee: &str, span: Span) {
        if self.declared_effects.iter().any(|e| e == effect) {
            self.used_effects.insert(effect.to_string());
        } else {
            self.err(
                Diagnostic::new("E0301", span, format!("`{}` uses effect `{}`, but `{}` does not declare it", callee, effect, self.fn_name))
                    .label(format!("this call needs `{}`", effect))
                    .help(format!("add `uses {}` to the signature of `{}`, or move the call to a function that already uses `{}`", effect, self.fn_name, effect)),
            );
        }
    }

    fn arity(&mut self, name: &str, args: &[Expr], n: usize, span: Span) -> bool {
        if args.len() == n {
            return true;
        }
        self.err(Diagnostic::new("E0502", span, format!("`{}` takes {} argument(s), but {} were given", name, n, args.len())));
        for a in args {
            self.check_expr(a, None);
        }
        false
    }

    /// Check an argument that must be a list; returns the element type.
    fn list_arg(&mut self, arg: &Expr, expected: Option<&Type>, name: &str) -> Type {
        let t = self.check_expr(arg, expected);
        match t {
            Type::List(elem) => *elem,
            Type::Unknown | Type::Never => Type::Unknown,
            other => {
                self.err(Diagnostic::new("E0501", arg.span, format!("`{}` needs a list, found `{}`", name, other)));
                Type::Unknown
            }
        }
    }

    fn check_call(&mut self, name: &str, name_span: Span, args: &[Expr], span: Span, expected: Option<&Type>) -> Type {
        let list_expected = match expected {
            Some(t @ Type::List(_)) => Some(t.clone()),
            _ => None,
        };
        let list_of = |t: Type| Type::List(Box::new(t));
        match name {
            "print" | "print_error" => {
                self.use_effect("io", name, span);
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &Type::Text);
                }
                Type::Unit
            }
            "to_text" => {
                if self.arity(name, args, 1, span) {
                    let t = self.check_expr(&args[0], None);
                    if !matches!(t, Type::Int | Type::Bool | Type::Unknown | Type::Never) {
                        self.err(Diagnostic::new("E0501", args[0].span, format!("`to_text` accepts `Int` or `Bool`, found `{}`", t)));
                    }
                }
                Type::Text
            }
            "length" => {
                if self.arity(name, args, 1, span) {
                    let t = self.check_expr(&args[0], None);
                    if !matches!(t, Type::List(_) | Type::Text | Type::Unknown | Type::Never) {
                        self.err(Diagnostic::new("E0501", args[0].span, format!("`length` accepts a list or `Text`, found `{}`", t)));
                    }
                }
                Type::Int
            }
            "append" => {
                if !self.arity(name, args, 2, span) {
                    return Type::Unknown;
                }
                let elem = self.list_arg(&args[0], list_expected.as_ref(), name);
                self.expect_expr(&args[1], &elem);
                list_of(elem)
            }
            "concat" => {
                if !self.arity(name, args, 2, span) {
                    return Type::Unknown;
                }
                let elem = self.list_arg(&args[0], list_expected.as_ref(), name);
                self.expect_expr(&args[1], &list_of(elem.clone()));
                list_of(elem)
            }
            "slice" => {
                if !self.arity(name, args, 3, span) {
                    return Type::Unknown;
                }
                let elem = self.list_arg(&args[0], list_expected.as_ref(), name);
                self.expect_expr(&args[1], &Type::Int);
                self.expect_expr(&args[2], &Type::Int);
                list_of(elem)
            }
            "replace_at" => {
                if !self.arity(name, args, 3, span) {
                    return Type::Unknown;
                }
                let elem = self.list_arg(&args[0], list_expected.as_ref(), name);
                self.expect_expr(&args[1], &Type::Int);
                self.expect_expr(&args[2], &elem);
                list_of(elem)
            }
            "range" => {
                if self.arity(name, args, 2, span) {
                    self.expect_expr(&args[0], &Type::Int);
                    self.expect_expr(&args[1], &Type::Int);
                }
                list_of(Type::Int)
            }
            "code_points" => {
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &Type::Text);
                }
                list_of(Type::Int)
            }
            "from_code_point" => {
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &Type::Int);
                }
                Type::Text
            }
            "from_code_points" => {
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &list_of(Type::Int));
                }
                Type::Text
            }
            "join" => {
                if self.arity(name, args, 2, span) {
                    self.expect_expr(&args[0], &list_of(Type::Text));
                    self.expect_expr(&args[1], &Type::Text);
                }
                Type::Text
            }
            "read_file" => {
                self.use_effect("fs", name, span);
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &Type::Text);
                }
                Type::Text
            }
            "write_file" => {
                self.use_effect("fs", name, span);
                if self.arity(name, args, 2, span) {
                    self.expect_expr(&args[0], &Type::Text);
                    self.expect_expr(&args[1], &Type::Text);
                }
                Type::Unit
            }
            "args" => {
                self.use_effect("env", name, span);
                self.arity(name, args, 0, span);
                list_of(Type::Text)
            }
            "exit" => {
                self.use_effect("env", name, span);
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &Type::Int);
                }
                Type::Never
            }
            "fail" => {
                if self.arity(name, args, 1, span) {
                    self.expect_expr(&args[0], &Type::Text);
                }
                Type::Never
            }
            _ => {
                let Some(sig) = self.fns.get(name).cloned() else {
                    let mut d = Diagnostic::new("E0203", name_span, format!("unknown function `{}`", name));
                    if self.lookup(name).is_some() {
                        d = d.help(format!("`{}` is a value, not a function", name));
                    }
                    self.err(d);
                    for a in args {
                        self.check_expr(a, None);
                    }
                    return Type::Unknown;
                };
                for effect in &sig.effects {
                    self.use_effect(effect, name, span);
                }
                if args.len() != sig.params.len() {
                    self.err(Diagnostic::new("E0502", span, format!("`{}` takes {} argument(s), but {} were given", name, sig.params.len(), args.len())));
                    for a in args {
                        self.check_expr(a, None);
                    }
                } else {
                    for (a, p) in args.iter().zip(sig.params.iter()) {
                        self.expect_expr(a, p);
                    }
                }
                sig.ret
            }
        }
    }
}
