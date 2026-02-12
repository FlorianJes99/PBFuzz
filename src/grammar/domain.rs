/// Basic building blocks for generating pseudo boolean constraints
use std::fmt::Display;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, PartialOrd, Ord)]
pub struct AtomIdent(u32);

impl AtomIdent {
    pub fn new(var_id: u32) -> Self {
        assert!(var_id > 0, "Identifier of an Atom needs to be > 0");
        Self(var_id)
    }
    pub fn literal_id(&self) -> u32 {
        self.0
    }

    /// consumes the id and creates a new atom
    pub fn to_atom(self, coeff: i32) -> Atom {
        Atom {
            atom_id: self,
            coeff,
        }
    }
}

impl Display for AtomIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "x{}", self.literal_id())
    }
}
///Atom is the most basic building block
///
///It consists of an Id (u32), which is used to display the variable name
///and an coefficient (i32).
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Atom {
    pub atom_id: AtomIdent,
    pub coeff: i32,
}

impl Atom {
    pub fn new(var_id: u32, coeff: i32) -> Self {
        Self {
            atom_id: AtomIdent::new(var_id),
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

#[test]
fn display_atom() {
    let atom = Atom::new(1, 3);
    assert_eq!(atom.to_string(), "3 x1".to_string());

    let negative_coeff = Atom::new(1, -3);
    assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
}
