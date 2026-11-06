//! Semantic types shared by the checker and the code generator.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Type {
    Int,
    Bool,
    Text,
    Unit,
    /// The type of `return`, `break`, `fail(...)`: never produces a value.
    Never,
    /// Produced after an error; compatible with everything to avoid cascades.
    Unknown,
    List(Box<Type>),
    Named(String),
}

impl Type {
    pub fn is_unit_like(&self) -> bool {
        matches!(self, Type::Unit | Type::Never | Type::Unknown)
    }

    pub fn contains_unknown(&self) -> bool {
        match self {
            Type::Unknown => true,
            Type::List(t) => t.contains_unknown(),
            _ => false,
        }
    }

    /// Can a value of type `self` be used where `expected` is required?
    pub fn fits(&self, expected: &Type) -> bool {
        match (self, expected) {
            (Type::Never, _) | (Type::Unknown, _) | (_, Type::Unknown) => true,
            (Type::List(a), Type::List(b)) => a.fits(b),
            (a, b) => a == b,
        }
    }

    /// The common type of two branches, if any.
    pub fn join(&self, other: &Type) -> Option<Type> {
        match (self, other) {
            (Type::Never, t) | (t, Type::Never) => Some(t.clone()),
            (Type::Unknown, _) | (_, Type::Unknown) => Some(Type::Unknown),
            (Type::List(a), Type::List(b)) => a.join(b).map(|t| Type::List(Box::new(t))),
            (a, b) if a == b => Some(a.clone()),
            _ => None,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "Int"),
            Type::Bool => write!(f, "Bool"),
            Type::Text => write!(f, "Text"),
            Type::Unit => write!(f, "Unit"),
            Type::Never => write!(f, "Never"),
            Type::Unknown => write!(f, "?"),
            Type::List(t) => write!(f, "List[{}]", t),
            Type::Named(n) => write!(f, "{}", n),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TypeDef {
    Record { fields: Vec<(String, Type)> },
    Enum { variants: Vec<(String, Vec<(String, Type)>)> },
}

pub const EFFECTS: &[&str] = &["io", "fs", "env", "process"];
