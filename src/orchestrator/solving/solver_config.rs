use rand::prelude::IndexedRandom;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use thiserror::Error;

use rand::RngExt;
use serde::Deserialize;

use crate::orchestrator::parsing::parser::ParserKind;

/// This holds all the necessary configuration in order to use an external solver for our use cases
///
/// A solver is expected to solve PBS/PBO problems, when providing the correct binary and mandatory args.
/// Furthermore you can provide additional arguments in order to fuzz different solver configurations.
/// If an argument for solving proofs is provided, then the solver is expected to generate proofs about its solutions.
///
/// If solver does not output his results in normal PB format, a specific parser kind can be added to enable proper parsing of the
/// output. Currently only SCIP is supported here
#[derive(Deserialize)]
pub struct SolverConfig {
    pub name: String,
    pub binary: PathBuf,
    pub args: Vec<String>,
    #[serde(default)]
    pub optional_args: OptionalArgs,
    pub proof_arg: Option<String>,
    pub parser_kind: ParserKind,
}

impl SolverConfig {
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        toml::from_str(&s).map_err(std::io::Error::other)
    }
}

#[derive(Deserialize, Default)]
pub struct OptionalArgs {
    args: Vec<OptionalArg>,
    /// if arg name and value are seperated by spaces, they need to be provided as separate arguments to the `Command` struct
    /// default to false
    #[serde(default)]
    space_separated: bool,
}

impl OptionalArgs {
    pub fn sample(&self, amount: usize, rng: &mut impl RngExt) -> Vec<String> {
        let amount = std::cmp::min(amount, self.args.len());
        self.args
            .sample(rng, amount)
            .flat_map(|arg| arg.collect_into_string_args(self.space_separated, rng))
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.args.is_empty()
    }
}

#[derive(Deserialize)]
pub struct OptionalArg {
    arg_name: String,
    arg_type: Option<ArgumentType>,
}

impl OptionalArg {
    fn collect_into_string_args(
        &self,
        space_separated: bool,
        rng: &mut impl RngExt,
    ) -> Vec<String> {
        match &self.arg_type {
            Some(t) => {
                let arg_type_as_string = t.sample_to_string(rng);
                if space_separated {
                    vec![self.arg_name.clone(), arg_type_as_string]
                } else {
                    vec![format!("{}{}", self.arg_name, arg_type_as_string)]
                }
            }
            None => vec![self.arg_name.to_string()],
        }
    }
}

#[derive(Deserialize)]
#[serde(try_from = "ArgumentTypeRaw")]
pub enum ArgumentType {
    IntRange(RangeInclusive<i32>),
    FloatRange(RangeInclusive<f32>),
    Values(Vec<String>),
}

impl TryFrom<ArgumentTypeRaw> for ArgumentType {
    type Error = SolverDeserializationError;

    fn try_from(value: ArgumentTypeRaw) -> Result<Self, Self::Error> {
        match value {
            ArgumentTypeRaw::Values(items) => Ok(ArgumentType::Values(items)),
            ArgumentTypeRaw::IntRange(i1, i2) => {
                if i1 > i2 {
                    Err(SolverDeserializationError::InvalidIntRange(i1, i2))
                } else {
                    Ok(ArgumentType::IntRange(i1..=i2))
                }
            }
            ArgumentTypeRaw::FloatRange(f1, f2) => {
                if f1 > f2 {
                    Err(SolverDeserializationError::InvalidFloatRange(f1, f2))
                } else {
                    Ok(ArgumentType::FloatRange(f1..=f2))
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum SolverDeserializationError {
    #[error("Invalid Range specified: {0} needs to be <= {1}")]
    InvalidIntRange(i32, i32),
    #[error("Invalid Range specified: {0} needs to be <= {1}")]
    InvalidFloatRange(f32, f32),
}

#[derive(Deserialize)]
enum ArgumentTypeRaw {
    IntRange(i32, i32),
    FloatRange(f32, f32),
    Values(Vec<String>),
}

impl ArgumentType {
    fn sample_to_string(&self, rng: &mut impl RngExt) -> String {
        match self {
            ArgumentType::IntRange(r) => rng.random_range(r.clone()).to_string(),
            ArgumentType::FloatRange(r) => rng.random_range(r.clone()).to_string(),
            ArgumentType::Values(items) => items.choose(rng).cloned().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_collect_arg_into_string_not_separated() {
        let mut rng = StdRng::seed_from_u64(42);
        let arg = OptionalArg {
            arg_name: "--myArg=".into(),
            arg_type: Some(ArgumentType::IntRange(1..=3)),
        };

        let args = arg.collect_into_string_args(false, &mut rng);
        assert_eq!(1, args.len());
        let (arg_name, arg_type) = args[0].split_once("=").unwrap();
        assert_eq!("--myArg", arg_name);
        assert!(
            arg_type.parse::<i32>().is_ok(),
            "should be able to parse int argument value: {}",
            arg_type
        );
    }

    #[test]
    fn test_collect_arg_into_string_separated() {
        let mut rng = StdRng::seed_from_u64(42);
        let arg = OptionalArg {
            arg_name: "--myArg".into(),
            arg_type: Some(ArgumentType::FloatRange(1.0..=3.0)),
        };

        let args = arg.collect_into_string_args(true, &mut rng);
        assert_eq!(args.len(), 2, "Expected Vec of size 2, but got: {:?}", args);
        let [arg_name, arg_type]: &[String; 2] = args.as_slice().try_into().unwrap();
        assert_eq!("--myArg", arg_name);
        assert!(
            arg_type.parse::<f32>().is_ok(),
            "should be able to parse float argument value: {}",
            arg_type
        );
    }
}
