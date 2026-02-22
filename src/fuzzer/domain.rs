/// Basic building blocks for generating pseudo boolean constraints
use std::fmt::{Display, Write};

/// Wrapper type for the identifier of an atom.
///
/// Values are expected to be > 0.
/// This type gets displayed as xN, where N is the actual id number.
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

    /// Consumes the id and creates a new atom
    pub fn to_atom(self, coeff: i32) -> Atom {
        Atom {
            atom_id: self,
            coeff,
        }
    }
}

impl From<u32> for AtomIdent {
    fn from(value: u32) -> Self {
        AtomIdent::new(value)
    }
}

impl std::str::FromStr for AtomIdent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("x") {
            return Err(format!(
                "Identifier does not start with character 'x', instead {}",
                s
            ));
        }
        let num_str = &s[1..];
        num_str
            .parse::<u32>()
            .map(|n| AtomIdent::new(n))
            .map_err(|e| format!("Failed to parse number: {}", e))
    }
}

impl Display for AtomIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "x{}", self.literal_id())
    }
}

/// Atom is the most basic building block
///
/// It consists of an Id (u32), which is used to display the variable name
/// and a coefficient (i32).
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

/// Wrapper type to represent the minimizing function
///
/// This is just a vector of atoms with distinct ids.
/// It is used when creating Pseudo-Boolean Optimization problems.
pub struct MinFunction(Vec<Atom>);

impl MinFunction {
    pub fn new(atoms: Vec<Atom>) -> Self {
        MinFunction(atoms)
    }
}

impl Display for MinFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("min: ")?;
        for (i, atom) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}", atom)?;
        }
        f.write_char(';')
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn convert_atom() {
        let id = AtomIdent::new(2);
        let atom = id.to_atom(15);
        let expected = Atom::new(2, 15);
        assert_eq!(
            expected, atom,
            "Expected equality: {} and {}",
            expected, atom
        )
    }

    #[test]
    fn display_atom() {
        let atom = Atom::new(1, 3);
        assert_eq!(atom.to_string(), "3 x1".to_string());

        let negative_coeff = Atom::new(1, -3);
        assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
    }

    #[test]
    fn display_min_function() {
        let min_function =
            MinFunction::new(vec![Atom::new(1, 3), Atom::new(2, 8), Atom::new(5, -3)]);

        assert_eq!("min: 3 x1 8 x2 -3 x5;", min_function.to_string())
    }
}
