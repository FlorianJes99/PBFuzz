/// Basic building blocks for generating pseudo boolean constraints
///
/// A constraint is build up from some amount of atoms an comparison operator and a degree (RHS)
use std::fmt::Display;

use itertools::Itertools;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct AtomId {
    pub literal_id: u32,
}

impl AtomId {
    fn new(var_id: u32) -> Self {
        Self { literal_id: var_id }
    }
}

impl Display for AtomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "x{}", self.literal_id)
    }
}
///Atom is the most basic building block
///
///It consists of an Id (u32), which is used to display the variable name
///and an coefficient (i32).
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Atom {
    pub atom_id: AtomId,
    pub coeff: i32,
}

impl Atom {
    pub fn new(var_id: u32, coeff: i32) -> Self {
        assert!(
            coeff != 0,
            "Zero coefficient does not make any sense for an atom"
        );
        Self {
            atom_id: AtomId::new(var_id),
            coeff,
        }
    }
}

impl Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.coeff, self.atom_id)
    }
}

/// Comparison enum models the different (In)equalities.
///
/// PBS/PBO normally supports only >= and = operators.
#[derive(Debug)]
pub enum Comparison {
    //TODO:  Equal,
    GrEqual,
}

impl Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            // Comparison::Equal => "=",
            Comparison::GrEqual => ">=",
        };
        write!(f, "{s}")
    }
}

pub trait Constraint {
    fn atoms(&self) -> &[Atom];
    fn comparison(&self) -> &Comparison;
    fn degree(&self) -> i64;

    fn satisfied(&self, value: i32) -> bool {
        match self.comparison() {
            Comparison::GrEqual => value as i64 >= self.degree(),
        }
    }

    fn is_solvable(&self) -> bool {
        match self.comparison() {
            Comparison::GrEqual => {
                self.atoms()
                    .iter()
                    .filter(|atom| atom.coeff > 0)
                    .map(|atom| atom.coeff as i64)
                    .sum::<i64>()
                    >= self.degree()
            }
        }
    }
}

/// Constraint is a combination of atoms, comparison operator and a degree
///
/// The degree should always be in bound of the atmos max and min assignment.
/// Otherwise a Constraint itself makes the problem instantly unsat.
#[allow(dead_code)]
#[derive(Debug)]
pub struct ArbitraryLengthConstraint {
    atoms: Vec<Atom>,
    comparison: Comparison,
    degree: i64,
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
}

#[allow(dead_code)]
impl ArbitraryLengthConstraint {
    /// Construct a new constraints object
    /// Atoms vector needs to have at least one item, otherwise function will panic
    /// Degree needs to be inside the min/max bounds of atoms assignment
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
        let atoms: String = self.atoms().iter().map(|x| x.to_string()).join(" ");

        write!(f, "{} {} {};", atoms, self.comparison(), self.degree())
    }
}

impl Display for ThreeAtomConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let atoms: String = self.atoms().iter().map(|x| x.to_string()).join(" ");

        write!(f, "{} {} {};", atoms, self.comparison(), self.degree())
    }
}

#[derive(Debug)]
pub struct ThreeAtomConstraint {
    atoms: [Atom; 3],
    comparison: Comparison,
    degree: i64,
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
        }
    }

    /// Checks how many assignments are unsat for a constraint
    /// !!!IMPORTANT!!! this method is only designed for constraints with 3 Methods.
    /// Calling this method on a constraint with different amount of methods will cause a panick
    pub fn unsat_count_three_atoms(&self) -> u32 {
        let n = self.atoms.len();
        assert_eq!(
            n, 3,
            "This method works only for exactly three atoms per constraint"
        );

        let [a1, a2, a3] = &self.atoms[..] else {
            unreachable!();
        };
        let (a1, a2, a3) = (a1.coeff, a2.coeff, a3.coeff);
        Self::ASSIGNMENTS
            .iter()
            .map(|(b1, b2, b3)| b1 * a1 + b2 * a2 + b3 * a3)
            .filter(|&sum| !self.satisfied(sum))
            .count()
            .try_into()
            .expect("Should be able to convert at max 8 assignments to a u32 number")
    }
}

#[test]
fn display_atom() {
    let atom = Atom::new(1, 3);
    assert_eq!(atom.to_string(), "3 x1".to_string());

    let negative_coeff = Atom::new(1, -3);
    assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
}

#[test]
fn amount_unsat() {
    let atoms = [Atom::new(1, 5), Atom::new(3, -1), Atom::new(5, 3)];
    let c = ThreeAtomConstraint::new(atoms, Comparison::GrEqual, 4);
    assert_eq!(c.unsat_count_three_atoms(), 4);

    let atoms = [Atom::new(1, 5), Atom::new(3, -1), Atom::new(5, 3)];
    let c = ThreeAtomConstraint::new(atoms, Comparison::GrEqual, 0);
    assert_eq!(c.unsat_count_three_atoms(), 1)
}

#[test]
fn display_constraint() {
    let atoms: Vec<Atom> = (1..=3).map(|i| Atom::new(i as u32, i * 10)).collect();

    let comp = Comparison::GrEqual;
    let res = 15;

    let constr = ArbitraryLengthConstraint::new(atoms, comp, res);

    assert_eq!(constr.to_string(), "10 x1 20 x2 30 x3 >= 15;")
}
