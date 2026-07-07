/// Basic building blocks for generating pseudo boolean constraints
use std::{
    collections::HashSet,
    fmt::{Display, Write},
    num::ParseIntError,
    ops::Deref,
};

use rand::RngExt;
use thiserror::Error;

/// Wrapper type for the identifier of an literal.
///
/// Values are expected to be > 0.
/// This type gets displayed as xN, where N is the actual id number.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, PartialOrd, Ord)]
pub struct LiteralIdent(u32);

#[derive(Debug, PartialOrd, PartialEq, Eq, Clone)]
pub struct MinObjectiveValue(i32);

impl Display for MinObjectiveValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct SolutionModel(HashSet<LiteralIdent>);

impl<T> From<T> for SolutionModel
where
    T: IntoIterator<Item = LiteralIdent>,
{
    fn from(value: T) -> Self {
        Self(value.into_iter().collect())
    }
}

impl Deref for SolutionModel {
    type Target = HashSet<LiteralIdent>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<i32> for MinObjectiveValue {
    fn from(value: i32) -> Self {
        MinObjectiveValue(value)
    }
}

impl LiteralIdent {
    pub fn new(var_id: u32) -> Self {
        assert!(var_id > 0, "Identifier of an Literal needs to be > 0");
        Self(var_id)
    }
    pub fn literal_id(&self) -> u32 {
        self.0
    }

    /// Consumes the id and creates a new literal
    pub fn to_literal(self, coeff: i32) -> Literal {
        Literal {
            literal_id: self,
            coeff,
        }
    }
}

impl From<u32> for LiteralIdent {
    fn from(value: u32) -> Self {
        LiteralIdent::new(value)
    }
}

impl std::str::FromStr for LiteralIdent {
    type Err = ParseLiteralError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with("x") {
            return Err(ParseLiteralError::InvalidCharacter { literal: s.into() });
        }
        let num_str = &s[1..];
        num_str
            .parse::<u32>()
            .map(LiteralIdent::new)
            .map_err(|err| ParseLiteralError::ParseInt {
                literal: s.into(),
                source: err,
            })
    }
}

#[derive(Debug, Error)]
pub enum ParseLiteralError {
    #[error("Failed to parse literal ({literal}) due to: {source}")]
    ParseInt {
        literal: String,
        #[source]
        source: ParseIntError,
    },
    #[error("Expected literal to start with x, but was {literal}")]
    InvalidCharacter { literal: String },
}

impl Display for LiteralIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "x{}", self.literal_id())
    }
}

/// Literal is the most basic building block
///
/// It consists of an Id (u32), which is used to display the variable name
/// and a coefficient (i32).
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Literal {
    pub literal_id: LiteralIdent,
    pub coeff: i32,
}

impl Literal {
    pub fn new(var_id: u32, coeff: i32) -> Self {
        Self {
            literal_id: LiteralIdent::new(var_id),
            coeff,
        }
    }
}

impl Display for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.coeff, self.literal_id)
    }
}

/// Wrapper type to represent the minimizing function
///
/// This is just a vector of literals with distinct ids.
/// It is used when creating Pseudo-Boolean Optimization problems.
pub struct MinFunction(Vec<Literal>);

impl MinFunction {
    pub fn new(literals: Vec<Literal>) -> Self {
        MinFunction(literals)
    }

    pub fn get_literals(&self) -> &[Literal] {
        &self.0
    }
}

impl Display for MinFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("min: ")?;
        for (i, literal) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}", literal)?;
        }
        f.write_char(';')
    }
}

/// Comparison enum models the different (In)equalities.
///
/// PBS/PBO normally supports only >= and = operators.
#[derive(Debug)]
pub enum Comparison {
    Equal,
    GrEqual,
}

impl Comparison {
    pub fn sample(rand: &mut impl RngExt) -> Self {
        match rand.random_bool(0.5) {
            true => Self::Equal,
            false => Self::GrEqual,
        }
    }
}

impl Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Comparison::GrEqual => ">=",
            Comparison::Equal => "=",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn convert_literal() {
        let id = LiteralIdent::new(2);
        let literal = id.to_literal(15);
        let expected = Literal::new(2, 15);
        assert_eq!(
            expected, literal,
            "Expected equality: {} and {}",
            expected, literal
        )
    }

    #[test]
    fn display_literal() {
        let literal = Literal::new(1, 3);
        assert_eq!(literal.to_string(), "3 x1".to_string());

        let negative_coeff = Literal::new(1, -3);
        assert_eq!(negative_coeff.to_string(), "-3 x1".to_string());
    }

    #[test]
    fn display_min_function() {
        let min_function = MinFunction::new(vec![
            Literal::new(1, 3),
            Literal::new(2, 8),
            Literal::new(5, -3),
        ]);

        assert_eq!("min: 3 x1 8 x2 -3 x5;", min_function.to_string())
    }
}
