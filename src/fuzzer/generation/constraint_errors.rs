use std::num::TryFromIntError;
use thiserror::Error;

use crate::fuzzer::constraints::errors::InvalidConstraintConstruction;

/// Sum type for all Errors happenings during Constraint Generation
///
/// This can be either Errors wrapped and propagated upwards from underlying calls or new error types
#[derive(Debug, Error)]
pub enum ConstraintGenerationError {
    #[error("Construction of constraint failed")]
    Invalid(#[from] InvalidConstraintConstruction),
    #[error(transparent)]
    ConvertionFromInt(#[from] TryFromIntError),
    #[error("Constraint length ({0}) is larger than amount of distinct literal ids ({1})")]
    InvalidSampling(u32, u32),
}
