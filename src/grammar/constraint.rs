use crate::grammar::domain::Atom;
use crate::grammar::domain::Comparison;
use std::cell::OnceCell;
use std::fmt::Display;

/// Constraint trait for different types of constraints in a PBS/PBO grammar.
///
/// atoms: a reference to a array slice of atoms.
/// comparison: the comparison operator used in this constraint.
/// degree: rhs value, for which we are solving the constraint.
///
/// Constraint is a combination of atoms, comparison operator and a degree
///
/// The degree should always be in bound of the atoms max and min assignment.
/// Otherwise, a Constraint itself makes the problem instantly unsat.
pub trait Constraint: Display {
    fn atoms(&self) -> &[Atom];
    fn comparison(&self) -> &Comparison;
    fn degree(&self) -> i64;
    fn kind(&self) -> ConstraintKind;

    /// checks if a value satisfies the degree of the constraint
    /// this depends on the comparison operator of the constraint.
    fn satisfied(&self, value: i64) -> bool {
        match self.comparison() {
            Comparison::GrEqual => value as i64 >= self.degree(),
        }
    }

    /// Checks if the constructed constraint is solvable.
    /// A solvable constraint needs to have at least one assignment, which fulfill the degree.
    fn is_solvable(&self) -> bool {
        self.satisfied(
            self.atoms()
                .iter()
                .filter(|atom| atom.coeff > 0)
                .map(|atom| atom.coeff as i64)
                .sum::<i64>(),
        )
    }

    fn fmt_constraint(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, atom) in self.atoms().iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            atom.fmt(f)?;
        }
        write!(f, " {} {};", self.comparison(), self.degree())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Hash)]
pub enum ConstraintKind {
    ArbitraryLength,
    ThreeAtom,
    Cardinality,
}

impl Display for ConstraintKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstraintKind::ArbitraryLength => write!(f, "Arbitrary Length"),
            ConstraintKind::ThreeAtom => write!(f, "Three Atom"),
            ConstraintKind::Cardinality => write!(f, "Cardinality"),
        }
    }
}

pub trait CostraintWithUnsatAssignments: Constraint {
    fn amount_unsat_assignments(&self) -> u128;
}

#[derive(Debug)]
pub struct ArbitraryLengthConstraint {
    atoms: Vec<Atom>,
    comparison: Comparison,
    degree: i64,
}

#[derive(Debug)]
pub struct ThreeAtomConstraint {
    atoms: [Atom; 3],
    comparison: Comparison,
    degree: i64,
    // lazy init this when first needed, can be expensive to compute
    amount_unsat: OnceCell<u128>,
}

/// Cardinality constraints are a special type of constraint, where every coefficient of an atom needs to be 1
pub struct CardinalityConstraint {
    atoms: Vec<Atom>,
    comparison: Comparison,
    degree: i64,
    // lazy init this when first needed, can be expensive to compute
    amount_unsat: OnceCell<u128>,
}

impl Constraint for ArbitraryLengthConstraint {
    fn atoms(&self) -> &[Atom] {
        &self.atoms
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
    /// Arbitrary length is relevant for the amount of atoms a single constraint holds.
    /// Degree needs to be inside the min/max bounds of atoms assignment.
    pub fn new(atoms: Vec<Atom>, comparison: Comparison, degree: i64) -> Self {
        assert!(
            !atoms.is_empty(),
            "Constraint needs at least one atom to be valid"
        );
        let constraint = Self {
            atoms,
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

impl From<ThreeAtomConstraint> for ArbitraryLengthConstraint {
    fn from(value: ThreeAtomConstraint) -> Self {
        ArbitraryLengthConstraint::new(value.atoms.to_vec(), value.comparison, value.degree)
    }
}

impl Constraint for ThreeAtomConstraint {
    fn atoms(&self) -> &[Atom] {
        &self.atoms
    }

    fn comparison(&self) -> &Comparison {
        &self.comparison
    }

    fn degree(&self) -> i64 {
        self.degree
    }

    fn kind(&self) -> ConstraintKind {
        ConstraintKind::ThreeAtom
    }
}

impl CostraintWithUnsatAssignments for ThreeAtomConstraint {
    fn amount_unsat_assignments(&self) -> u128 {
        *self.amount_unsat.get_or_init(|| {
            self.unsat_count_three_atoms()
                .try_into()
                .expect("Should able to widen u32 into u128")
        })
    }
}

impl Display for ThreeAtomConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}

impl TryFrom<ArbitraryLengthConstraint> for ThreeAtomConstraint {
    type Error = String;

    fn try_from(value: ArbitraryLengthConstraint) -> Result<Self, Self::Error> {
        let ArbitraryLengthConstraint {
            atoms,
            comparison,
            degree,
        } = value;
        atoms
            .try_into()
            .map_err(|_| "Constraint has not exactly three atoms".to_string())
            .map(|[a1, a2, a3]: [Atom; 3]| {
                ThreeAtomConstraint::new([a1, a2, a3], comparison, degree)
            })
    }
}

impl ThreeAtomConstraint {
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

    pub fn new(atoms: [Atom; 3], comparison: Comparison, degree: i64) -> Self {
        Self {
            atoms,
            comparison,
            degree,
            amount_unsat: OnceCell::new(),
        }
    }

    pub fn atoms_mut(&mut self) -> &mut [Atom] {
        &mut self.atoms
    }

    /// Checks how many assignments are unsat for a constraint with 3 Atoms
    fn unsat_count_three_atoms(&self) -> u32 {
        let [a1, a2, a3] = &self.atoms;
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
    pub fn new(atoms: Vec<Atom>, comparison: Comparison, degree: i64) -> Result<Self, String> {
        if atoms.iter().find(|a| a.coeff != 1).is_some() {
            return Err(
                "All coefficients of a CardinalityConstraint need to be 1".to_string(),
            );
        }
        let c = Self {
            atoms,
            comparison,
            degree,
            amount_unsat: OnceCell::new(),
        };
        if !c.is_solvable() {
            return Err("A constraint on itself needs to be solvable".to_string());
        }
        Ok(c)
    }
}

impl Constraint for CardinalityConstraint {
    fn atoms(&self) -> &[Atom] {
        &self.atoms
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
    fn amount_unsat_assignments(&self) -> u128 {
        *self.amount_unsat.get_or_init(|| {
            let amount_vars = self.atoms().len() as u64;
            let degree = self.degree() as u64;
            if degree == 0 {
                return 0;
            }

            (0..degree).fold(0_u128, |acc, i| acc + binomial(amount_vars, i))
        })
    }
}

fn binomial(n: u64, k: u64) -> u128 {
    let k = k.min(n - k);
    let mut r = 1u128;
    for i in 0..k {
        r = r * (n - i) as u128 / (i + 1) as u128;
    }
    r
}

impl Display for CardinalityConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_constraint(f)
    }
}

#[test]
fn amount_unsat() {
    let atoms = [Atom::new(1, 5), Atom::new(3, -1), Atom::new(5, 3)];
    let c = ThreeAtomConstraint::new(atoms, Comparison::GrEqual, 4);
    assert_eq!(c.unsat_count_three_atoms(), 4);

    let atoms = [Atom::new(1, 5), Atom::new(3, -1), Atom::new(5, 3)];
    let c = ThreeAtomConstraint::new(atoms, Comparison::GrEqual, 0);
    assert_eq!(c.unsat_count_three_atoms(), 1);

    let atoms = vec![Atom::new(1, 1), Atom::new(2, 1), Atom::new(3, 1)];
    let c = CardinalityConstraint::new(atoms, Comparison::GrEqual, 2).expect("Test method");
    assert_eq!(c.amount_unsat_assignments(), 4);

    let atoms = vec![Atom::new(1, 1), Atom::new(2, 1), Atom::new(3, 1)];
    let c = CardinalityConstraint::new(atoms, Comparison::GrEqual, 1).expect("Test method");
    assert_eq!(c.amount_unsat_assignments(), 1)
}

#[test]
fn display_constraint() {
    let atoms: Vec<Atom> = (1..=3).map(|i| Atom::new(i as u32, i * 10)).collect();

    let comp = Comparison::GrEqual;
    let res = 15;

    let constr = ArbitraryLengthConstraint::new(atoms, comp, res);

    assert_eq!(constr.to_string(), "10 x1 20 x2 30 x3 >= 15;")
}
