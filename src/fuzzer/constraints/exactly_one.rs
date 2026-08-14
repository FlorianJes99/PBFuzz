use crate::fuzzer::constraints::constraint::Constraint;
use crate::fuzzer::constraints::constraint::ConstraintKind;
use std::fmt::Display;

use crate::fuzzer::constraints::errors::InvalidConstraintConstruction;

use crate::fuzzer::domain::Comparison;

use crate::fuzzer::domain::Literal;

pub struct ExactlyOneConstraint {
    literals: Vec<Literal>,
    comparison: Comparison,
    degree: i64,
}

impl ExactlyOneConstraint {
    pub fn new(
        literals: Vec<Literal>,
        comparison: Comparison,
        degree: i64,
    ) -> Result<Self, InvalidConstraintConstruction> {
        if literals.is_empty() {
            return Err(InvalidConstraintConstruction::create_empty_literal(
                ConstraintKind::ExactlyOne,
            ));
        }
        match comparison {
            Comparison::Equal => Ok(Self {
                literals,
                comparison,
                degree,
            }),
            Comparison::GrEqual => Err(InvalidConstraintConstruction(
                format!("Invalid comparison operator ({comparison}) for this constraint!"),
                ConstraintKind::ExactlyOne,
            )),
        }
    }
}

impl Constraint for ExactlyOneConstraint {
    fn literals(&self) -> &[Literal] {
        &self.literals
    }

    fn comparison(&self) -> &Comparison {
        &self.comparison
    }

    fn degree(&self) -> i64 {
        self.degree
    }

    fn kind(&self) -> ConstraintKind {
        ConstraintKind::ExactlyOne
    }
}

impl Display for ExactlyOneConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}
