use crate::fuzzer::domain::Comparison;
use crate::fuzzer::domain::Literal;
use crate::fuzzer::domain::SolutionModel;
use std::cell::OnceCell;
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
        }
    }

    /// Checks if the constraint is satisfied for a given set of true variables in a grammar
    ///
    /// This can be used in order to verify the results of a solver
    fn verify(&self, true_vars: &SolutionModel) -> bool {
        self.satisfied(
            self.literals()
                .iter()
                .filter(|a| true_vars.contains(&a.literal_id))
                .map(|a| a.coeff as i64)
                .sum::<i64>(),
        )
    }

    /// Checks if the constructed constraint is solvable.
    /// A solvable constraint needs to have at least one assignment, which fulfill the degree.
    fn is_solvable(&self) -> bool {
        self.satisfied(
            self.literals()
                .iter()
                .filter(|literal| literal.coeff > 0)
                .map(|literal| literal.coeff as i64)
                .sum::<i64>(),
        )
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
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConstraintKind {
    ArbitraryLength,
    ThreeLiteral,
    Cardinality,
}

impl Display for ConstraintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintKind::ArbitraryLength => write!(f, "Arbitrary Length"),
            ConstraintKind::ThreeLiteral => write!(f, "Three Literal"),
            ConstraintKind::Cardinality => write!(f, "Cardinality"),
        }
    }
}

/// A constraint with no specific characteristics.
///
/// Length can be chosen arbitrary as well as coefficient range.
#[derive(Debug)]
pub struct ArbitraryLengthConstraint {
    literals: Vec<Literal>,
    comparison: Comparison,
    degree: i64,
}

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

/// Cardinality constraints are a special type of constraint, where every coefficient
/// of an literal needs to be 1 or -1
pub struct CardinalityConstraint {
    literals: Vec<Literal>,
    comparison: Comparison,
    degree: i64,
    // Lazy init this when first needed, can be expensive to compute
    amount_unsat: OnceCell<u128>,
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

        debug_assert!(
            constraint.is_solvable(),
            "A constraint on each own should be solvable"
        );
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
        ArbitraryLengthConstraint::new(value.literals.to_vec(), value.comparison, value.degree)
    }
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
        let ArbitraryLengthConstraint {
            literals,
            comparison,
            degree,
        } = value;
        literals
            .try_into()
            .map_err(|_| "Constraint has not exactly three literals".to_string())
            .map(|[a1, a2, a3]: [Literal; 3]| {
                ThreeLiteralConstraint::new([a1, a2, a3], comparison, degree)
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
    fn unsat_count_three_literals(&self) -> u32 {
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

impl CardinalityConstraint {
    pub fn new(
        literals: Vec<Literal>,
        comparison: Comparison,
        degree: i64,
    ) -> Result<Self, String> {
        if literals
            .iter()
            .find(|a| !(a.coeff == 1 || a.coeff == -1))
            .is_some()
        {
            return Err(
                "All coefficients of a CardinalityConstraint need to be 1 or -1".to_string(),
            );
        }
        let c = Self {
            literals,
            comparison,
            degree,
            amount_unsat: OnceCell::new(),
        };
        if !c.is_solvable() {
            return Err("A constraint on itself needs to be solvable".to_string());
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

fn binomial(n: u64, k: u64) -> u128 {
    let k = k.min(n - k);
    (0..k).fold(1u128, |acc, i| acc * (n - i) as u128 / (i + 1) as u128)
}

impl Display for CardinalityConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}

#[cfg(test)]
mod test {
    use super::*;
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
        assert!(c.verify(&true_vars));
        let true_vars = HashSet::from([LiteralIdent::new(1), LiteralIdent::new(3)]).into();
        assert!(c.verify(&true_vars));
        let true_vars = HashSet::from([LiteralIdent::new(3)]).into();
        assert!(!c.verify(&true_vars));

        let literals = vec![
            Literal::new(1, -1),
            Literal::new(2, -1),
            Literal::new(3, -1),
        ];
        let c = CardinalityConstraint::new(literals, Comparison::GrEqual, 0).expect("Test method");
        let true_vars = HashSet::new().into();
        assert!(c.verify(&true_vars));

        let true_vars = HashSet::from([LiteralIdent::new(1)]).into();
        assert!(!c.verify(&true_vars));

        let true_vars = HashSet::from([LiteralIdent::new(1), LiteralIdent::new(3)]).into();
        assert!(!c.verify(&true_vars));
    }
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
