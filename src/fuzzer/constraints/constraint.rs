use crate::fuzzer::constraints::errors::ConstraintViolation;
use crate::fuzzer::domain::Comparison;
use crate::fuzzer::domain::Literal;
use crate::fuzzer::domain::SolutionModel;
use std::fmt::Display;

/// Constraint trait for different types of constraints in a PBS/PBO grammar.
///
/// Literals: a reference to a array slice of literals.
/// Comparison: the comparison operator used in this constraint.
/// Degree: rhs value, for which we are solving the constraint.
///
/// Constraint is a combination of literals, comparison operator and a degree
///
/// The degree should always be in bound of the literals max and min assignment.
/// Otherwise, a Constraint itself makes the problem instantly unsat.
pub trait Constraint: Display + Send {
    fn literals(&self) -> &[Literal];
    fn comparison(&self) -> &Comparison;
    fn degree(&self) -> i64;
    fn kind(&self) -> ConstraintKind;

    /// Checks if a value satisfies the degree of the constraint
    /// this depends on the comparison operator of the constraint.
    fn satisfied(&self, value: i64) -> bool {
        match self.comparison() {
            Comparison::GrEqual => value >= self.degree(),
            Comparison::Equal => value == self.degree(),
        }
    }

    /// Checks if the constraint is satisfied for a given set of true variables in a grammar
    ///
    /// This can be used in order to verify the results of a solver
    fn verify(&self, true_vars: &SolutionModel) -> Result<(), ConstraintViolation> {
        let eval = self
            .literals()
            .iter()
            .filter(|a| true_vars.contains(&a.literal_id))
            .map(|a| a.coeff as i64)
            .sum::<i64>();
        if self.satisfied(eval) {
            Ok(())
        } else {
            Err(ConstraintViolation {
                evaluated: eval,
                expected: self.degree(),
                comp: *self.comparison(),
            })
        }
    }

    /// Checks if the constructed constraint is solvable.
    /// A solvable constraint needs to have at least one assignment, which fulfill the degree.
    fn is_solvable(&self) -> Result<(), ConstraintViolation> {
        let lhs = self
            .literals()
            .iter()
            .filter(|literal| literal.coeff > 0)
            .map(|literal| Into::<i64>::into(literal.coeff))
            .sum::<i64>();

        if self.satisfied(lhs) {
            Ok(())
        } else {
            Err(ConstraintViolation {
                evaluated: lhs,
                expected: self.degree(),
                comp: *self.comparison(),
            })
        }
    }

    /// Can be used in every implementation of the trait in order to get a formatted version of the
    /// constraint
    fn fmt_constraint(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, literal) in self.literals().iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            literal.fmt(f)?;
        }
        write!(f, " {} {};", self.comparison(), self.degree())
    }
}

/// Extension of the Constraint type
///
/// It is used for Constraint Kinds, where we can compute the amount of unsat assignments
/// in reasonable time amount (e.g. Three Literal or Cardinality)
pub trait CostraintWithUnsatAssignments: Constraint {
    fn amount_unsat_assignments(&self) -> u128;
}

/// Currently, these types of constraints are supported.
///  Mainly used for type safe generation of the different types of actual constraints
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstraintKind {
    ArbitraryLength,
    ThreeLiteral,
    Cardinality,
    ExactlyOne,
    DoublingPairs,
}

impl Display for ConstraintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintKind::ArbitraryLength => write!(f, "Arbitrary Length"),
            ConstraintKind::ThreeLiteral => write!(f, "Three Literal"),
            ConstraintKind::Cardinality => write!(f, "Cardinality"),
            ConstraintKind::ExactlyOne => write!(f, "Exactly One"),
            ConstraintKind::DoublingPairs => write!(f, "Doubling Pairs"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::fuzzer::constraints::cardinality::CardinalityConstraint;
    use crate::fuzzer::constraints::generic::ArbitraryLengthConstraint;
    use crate::fuzzer::constraints::three_literals::ThreeLiteralConstraint;
    use crate::fuzzer::domain::LiteralIdent;
    use std::collections::HashSet;
    #[test]
    fn amount_unsat() {
        let literals = [Literal::new(1, 5), Literal::new(3, -1), Literal::new(5, 3)];
        let c = ThreeLiteralConstraint::new(literals, Comparison::GrEqual, 4);
        assert_eq!(c.unsat_count_three_literals(), 4);

        let literals = [Literal::new(1, 5), Literal::new(3, -1), Literal::new(5, 3)];
        let c = ThreeLiteralConstraint::new(literals, Comparison::GrEqual, 0);
        assert_eq!(c.unsat_count_three_literals(), 1);

        let literals = vec![Literal::new(1, 1), Literal::new(2, 1), Literal::new(3, 1)];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, 2).expect("Test method");
        assert_eq!(c.amount_unsat_assignments(), 4);

        let literals = vec![Literal::new(1, 1), Literal::new(2, 1), Literal::new(3, 1)];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, 1).expect("Test method");
        assert_eq!(c.amount_unsat_assignments(), 1)
    }

    #[test]
    fn amount_unsat_with_negative_coefficeints() {
        let literals = vec![
            Literal::new(1, -1),
            Literal::new(2, -1),
            Literal::new(3, -1),
        ];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, 0).expect("Test method");
        assert_eq!(c.amount_unsat_assignments(), 7);

        let literals = vec![
            Literal::new(1, -1),
            Literal::new(2, -1),
            Literal::new(3, -1),
        ];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, -2).expect("Test method");
        assert_eq!(c.amount_unsat_assignments(), 1);

        let literals = vec![
            Literal::new(1, -1),
            Literal::new(2, -1),
            Literal::new(3, -1),
        ];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, -3).expect("Test method");
        assert_eq!(c.amount_unsat_assignments(), 0)
    }

    #[test]
    fn display_constraint() {
        let literals: Vec<Literal> = (1..=3).map(|i| Literal::new(i as u32, i * 10)).collect();

        let comp = Comparison::GrEqual;
        let res = 15;

        let constr = ArbitraryLengthConstraint::new(literals, comp, res);

        assert_eq!(constr.to_string(), "10 x1 20 x2 30 x3 >= 15;")
    }

    #[test]
    fn verify_constraint() {
        let literals = [Literal::new(1, 5), Literal::new(3, -1), Literal::new(5, 3)];
        let c = ThreeLiteralConstraint::new(literals, Comparison::GrEqual, 0);
        let true_vars = HashSet::from([LiteralIdent::new(1)]).into();
        assert!(c.verify(&true_vars).is_ok());
        let true_vars = HashSet::from([LiteralIdent::new(1), LiteralIdent::new(3)]).into();
        assert!(c.verify(&true_vars).is_ok());
        let true_vars = HashSet::from([LiteralIdent::new(3)]).into();
        assert!(c.verify(&true_vars).is_err());

        let literals = vec![
            Literal::new(1, -1),
            Literal::new(2, -1),
            Literal::new(3, -1),
        ];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, 0).expect("Test method");
        let true_vars = HashSet::new().into();
        assert!(c.verify(&true_vars).is_ok());

        let true_vars = HashSet::from([LiteralIdent::new(1)]).into();
        assert!(c.verify(&true_vars).is_err());

        let true_vars = HashSet::from([LiteralIdent::new(1), LiteralIdent::new(3)]).into();
        assert!(c.verify(&true_vars).is_err());
    }
}
