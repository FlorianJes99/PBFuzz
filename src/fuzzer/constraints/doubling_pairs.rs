use std::fmt::Display;

use crate::fuzzer::{
    constraints::constraint::Constraint,
    domain::{Comparison, Literal},
};

pub struct DoublingPairs {
    literals: Vec<Literal>,
    comparison: Comparison,
    degree: i64,
}

impl DoublingPairs {
    pub fn new(literals: Vec<Literal>, comparison: Comparison, degree: i64) -> Self {
        Self {
            literals,
            comparison,
            degree,
        }
    }
}

impl Constraint for DoublingPairs {
    fn literals(&self) -> &[Literal] {
        &self.literals
    }

    fn comparison(&self) -> &Comparison {
        &self.comparison
    }

    fn degree(&self) -> i64 {
        self.degree
    }

    fn kind(&self) -> super::constraint::ConstraintKind {
        super::constraint::ConstraintKind::DoublingPairs
    }
}

impl Display for DoublingPairs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}
