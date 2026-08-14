use crate::fuzzer::constraints::constraint::Constraint;
use crate::fuzzer::constraints::constraint::ConstraintKind;
use crate::fuzzer::constraints::three_literals::ThreeLiteralConstraint;
use std::fmt::Display;

use crate::fuzzer::domain::Comparison;

use crate::fuzzer::domain::Literal;

/// A constraint with no specific characteristics.
///
/// Length can be chosen arbitrary as well as coefficient range.
#[derive(Debug)]
pub struct ArbitraryLengthConstraint {
    literals: Vec<Literal>,
    comparison: Comparison,
    degree: i64,
}

impl Constraint for ArbitraryLengthConstraint {
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
        ConstraintKind::ArbitraryLength
    }
}

impl ArbitraryLengthConstraint {
    /// Construct a new constraints object with arbitrary length.
    ///
    /// Arbitrary length is relevant for the amount of literals a single constraint holds.
    /// Degree needs to be inside the min/max bounds of literals assignment.
    pub fn new(literals: Vec<Literal>, comparison: Comparison, degree: i64) -> Self {
        assert!(
            !literals.is_empty(),
            "Constraint needs at least one literal to be valid"
        );
        let constraint = Self {
            literals,
            comparison,
            degree,
        };

        #[cfg(debug_assertions)]
        if let Err(e) = constraint.is_solvable() {
            panic!("Constraint is not solvable: {e}");
        }
        constraint
    }
}

impl Display for ArbitraryLengthConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}

impl From<ThreeLiteralConstraint> for ArbitraryLengthConstraint {
    fn from(value: ThreeLiteralConstraint) -> Self {
        ArbitraryLengthConstraint::new(
            value.literals().to_vec(),
            *value.comparison(),
            value.degree(),
        )
    }
}
