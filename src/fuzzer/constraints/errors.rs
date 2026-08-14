use thiserror::Error;

use crate::fuzzer::{constraints::constraint::ConstraintKind, domain::Comparison};

#[derive(Debug, Error)]
#[error("Constraint violated: evaluated {evaluated}, expected {comp} {expected}")]
pub struct ConstraintViolation {
    pub evaluated: i64,
    pub expected: i64,
    pub comp: Comparison,
}

#[derive(Debug, Error)]
#[error("Can not construct constraint of kind '{1}' out of line: {0}")]
pub struct InvalidConstraintConstruction(pub String, pub ConstraintKind);

impl InvalidConstraintConstruction {
    pub fn create_empty_literal(kind: ConstraintKind) -> Self {
        InvalidConstraintConstruction("Constraint has no literals".into(), kind)
    }
}
