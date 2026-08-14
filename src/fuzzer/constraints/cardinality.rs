use std::cell::OnceCell;
use std::fmt::Display;

use log::error;

use crate::fuzzer::constraints::constraint::Constraint;
use crate::fuzzer::constraints::constraint::ConstraintKind;
use crate::fuzzer::constraints::constraint::CostraintWithUnsatAssignments;
use crate::fuzzer::constraints::errors::InvalidConstraintConstruction;
use crate::fuzzer::domain::Comparison;

use crate::fuzzer::domain::Literal;

/// Cardinality constraints are a special type of constraint, where every coefficient
/// of an literal needs to be 1 or -1
pub struct CardinalityConstraint {
    literals: Vec<Literal>,
    comparison: Comparison,
    degree: i64,
    // Lazy init this when first needed, can be expensive to compute
    amount_unsat: OnceCell<u128>,
}
impl CardinalityConstraint {
    pub fn new(
        literals: Vec<Literal>,
        comparison: Comparison,
        degree: i64,
    ) -> Result<Self, InvalidConstraintConstruction> {
        if literals.is_empty() {
            return Err(InvalidConstraintConstruction::create_empty_literal(
                ConstraintKind::Cardinality,
            ));
        }
        if literals
            .iter()
            .find(|a| !(a.coeff == 1 || a.coeff == -1))
            .is_some()
        {
            return Err(InvalidConstraintConstruction(
                "All coefficients of a CardinalityConstraint need to be 1 or -1".to_string(),
                ConstraintKind::Cardinality,
            ));
        }
        let c = Self {
            literals,
            comparison,
            degree,
            amount_unsat: OnceCell::new(),
        };
        if let Err(e) = c.is_solvable() {
            error!("{e}");
            return Err(InvalidConstraintConstruction(
                "A constraint on itself needs to be solvable".to_string(),
                ConstraintKind::Cardinality,
            ));
        }
        Ok(c)
    }

    fn lowest_rhs_assignment(&self) -> i32 {
        self.literals()
            .iter()
            .map(|a| a.coeff)
            .filter(|coeff| *coeff < 0)
            .sum()
    }
}

impl Constraint for CardinalityConstraint {
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
        ConstraintKind::Cardinality
    }
}

impl CostraintWithUnsatAssignments for CardinalityConstraint {
    //TODO: this is not correct anymore, because coeffs can be -1 as well now
    fn amount_unsat_assignments(&self) -> u128 {
        *self.amount_unsat.get_or_init(|| {
            let max_combinations_to_set_true = self.degree() - self.lowest_rhs_assignment() as i64;
            let amount_vars = self.literals().len() as u64;

            (0..max_combinations_to_set_true as u64)
                .fold(0_u128, |acc, i| acc + binomial(amount_vars, i))
        })
    }
}

impl Display for CardinalityConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}

fn binomial(n: u64, k: u64) -> u128 {
    let k = k.min(n - k);
    (0..k).fold(1u128, |acc, i| acc * (n - i) as u128 / (i + 1) as u128)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choose_zero() {
        assert_eq!(binomial(5, 0), 1);
    }

    #[test]
    fn test_choose_self() {
        assert_eq!(binomial(5, 5), 1);
    }

    #[test]
    fn test_choose_one() {
        assert_eq!(binomial(5, 1), 5);
    }

    #[test]
    fn test_choose_two() {
        assert_eq!(binomial(5, 2), 10);
    }

    #[test]
    fn test_symmetry() {
        // C(n, k) == C(n, n-k)
        assert_eq!(binomial(10, 3), binomial(10, 7));
    }

    #[test]
    fn test_pascals_identity() {
        // C(n, k) == C(n-1, k-1) + C(n-1, k)
        assert_eq!(binomial(10, 4), binomial(9, 3) + binomial(9, 4));
    }

    #[test]
    fn test_known_value() {
        // 52 choose 5 = number of poker hands
        assert_eq!(binomial(52, 5), 2_598_960);
    }
}
