//! Syntax tree produced by the parser.

use crate::diag::Span;

pub type ExprId = usize;

#[derive(Debug)]
pub struct Program {
    pub items: Vec<Item>,
    /// Number of expression ids handed out; used to size side tables.
    pub expr_count: usize,
}

#[derive(Debug)]
pub enum Item {
    Function(Function),
    Record(RecordDecl),
    Enum(EnumDecl),
}

#[derive(Debug, Clone)]
pub struct Note {
    pub text: String,
    pub span: Span,
}

#[derive(Debug)]
pub struct Function {
    pub notes: Vec<Note>,
    pub name: String,
    pub name_span: Span,
    pub params: Vec<Param>,
    pub ret: TypeExpr,
    pub effects: Vec<(String, Span)>,
    pub body: Expr,
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    pub span: Span,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone)]
pub struct TypeExpr {
    pub name: String,
    pub args: Vec<TypeExpr>,
    pub span: Span,
}

#[derive(Debug)]
pub struct Field {
    pub name: String,
    pub span: Span,
    pub ty: TypeExpr,
}

#[derive(Debug)]
pub struct RecordDecl {
    pub notes: Vec<Note>,
    pub name: String,
    pub name_span: Span,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub notes: Vec<Note>,
    pub name: String,
    pub name_span: Span,
    pub variants: Vec<VariantDecl>,
}

#[derive(Debug)]
pub struct VariantDecl {
    pub name: String,
    pub span: Span,
    pub fields: Vec<Field>,
}

#[derive(Debug)]
pub struct Expr {
    pub id: ExprId,
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

impl BinaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::NotEq => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::LtEq => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::GtEq => ">=",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
        }
    }
}

#[derive(Debug)]
pub enum ExprKind {
    Int(i64),
    Bool(bool),
    Text(String),
    Unit,
    Name(String),
    List(Vec<Expr>),
    Call { name: String, name_span: Span, args: Vec<Expr> },
    Index { target: Box<Expr>, index: Box<Expr> },
    Field { target: Box<Expr>, name: String, name_span: Span },
    Unary { op: UnaryOp, operand: Box<Expr> },
    Binary { op: BinaryOp, op_span: Span, left: Box<Expr>, right: Box<Expr> },
    If { cond: Box<Expr>, then_block: Block, else_branch: Option<Box<Expr>> },
    Block(Block),
    Match { scrutinee: Box<Expr>, arms: Vec<Arm> },
    Record { type_name: String, type_span: Span, fields: Vec<FieldInit> },
    Variant { type_name: String, variant: String, variant_span: Span, args: Vec<Expr> },
    Return(Option<Box<Expr>>),
    Break,
    Continue,
}

#[derive(Debug)]
pub struct FieldInit {
    pub name: String,
    pub span: Span,
    pub value: Expr,
}

#[derive(Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub tail: Option<Box<Expr>>,
    pub span: Span,
}

#[derive(Debug)]
pub enum Stmt {
    Let { name: Option<String>, name_span: Span, ty: Option<TypeExpr>, value: Expr, mutable: bool },
    Set { name: String, name_span: Span, value: Expr },
    For { var: String, var_span: Span, iterable: Expr, body: Block },
    While { cond: Expr, body: Block },
    Expr(Expr),
}

#[derive(Debug)]
pub struct Arm {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Debug)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum PatternKind {
    Wildcard,
    Int(i64),
    Text(String),
    Bool(bool),
    Variant { type_name: String, variant: String, bindings: Vec<(Option<String>, Span)> },
}
