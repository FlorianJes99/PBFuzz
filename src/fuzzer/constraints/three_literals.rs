use crate::fuzzer::constraints::generic::ArbitraryLengthConstraint;
use std::{cell::OnceCell, fmt::Display};

use crate::fuzzer::{
    constraints::constraint::{Constraint, ConstraintKind, CostraintWithUnsatAssignments},
    domain::{Comparison, Literal},
};

/// A constraint with exactly three literals
///
/// For this type it is fairly ease to compute all possible unsat assignments.
#[derive(Debug)]
pub struct ThreeLiteralConstraint {
    literals: [Literal; 3],
    comparison: Comparison,
    degree: i64,
    /// lazy init this when first needed, can be expensive to compute
    amount_unsat: OnceCell<u128>,
}

impl Constraint for ThreeLiteralConstraint {
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
        ConstraintKind::ThreeLiteral
    }
}

impl CostraintWithUnsatAssignments for ThreeLiteralConstraint {
    fn amount_unsat_assignments(&self) -> u128 {
        *self
            .amount_unsat
            .get_or_init(|| self.unsat_count_three_literals().into())
    }
}

impl Display for ThreeLiteralConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}

impl TryFrom<ArbitraryLengthConstraint> for ThreeLiteralConstraint {
    type Error = String;

    fn try_from(value: ArbitraryLengthConstraint) -> Result<Self, Self::Error> {
        value
            .literals()
            .to_owned()
            .try_into()
            .map_err(|_| "Constraint has not exactly three literals".to_string())
            .map(|[a1, a2, a3]: [Literal; 3]| {
                ThreeLiteralConstraint::new([a1, a2, a3], *value.comparison(), value.degree())
            })
    }
}

impl ThreeLiteralConstraint {
    const ASSIGNMENTS: [(i32, i32, i32); 8] = [
        (0, 0, 0),
        (1, 0, 0),
        (0, 1, 0),
        (0, 0, 1),
        (1, 1, 0),
        (1, 0, 1),
        (0, 1, 1),
        (1, 1, 1),
    ];

    pub fn new(literals: [Literal; 3], comparison: Comparison, degree: i64) -> Self {
        Self {
            literals,
            comparison,
            degree,
            amount_unsat: OnceCell::new(),
        }
    }

    /// Checks how many assignments are unsat for a constraint with 3 Literals
    pub fn unsat_count_three_literals(&self) -> u32 {
        let [a1, a2, a3] = &self.literals;
        Self::ASSIGNMENTS
            .iter()
            .map(|(b1, b2, b3)| b1 * a1.coeff + b2 * a2.coeff + b3 * a3.coeff)
            .filter(|&sum| !self.satisfied(sum.into()))
            .count()
            .try_into()
            .expect("Should be able to convert at max 8 assignments to a u32 number")
    }
}
