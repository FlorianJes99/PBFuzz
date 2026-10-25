use crate::{
    fuzzer::{
        constraints::errors::ConstraintViolation,
        grammar::{MinimumValidation, PBGrammar},
    },
    orchestrator::parsing::parser::ParsedStatus,
};

use thiserror::Error;

use crate::{
    fuzzer::domain::MinObjectiveValue, orchestrator::evaluation::proof_checking::ProofError,
};

pub fn wrap_objective(min_objective: MinObjectiveValue, solver_name: String) -> ObjectiveInfo {
    ObjectiveInfo {
        solver_name,
        min_objective,
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    Constraint(#[from] ConstraintViolation),
    #[error(transparent)]
    Objective(#[from] MinimumValidation),
}

impl ValidationError {
    pub fn short_str(&self) -> &'static str {
        match self {
            ValidationError::Constraint(_) => "invalid_constraint",
            ValidationError::Objective(_) => "invalid_objective",
        }
    }
}

#[derive(Debug, Error)]
#[error("{solver_name} - minimum: {min_objective}")]
pub struct ObjectiveInfo {
    solver_name: String,
    min_objective: MinObjectiveValue,
}

impl ObjectiveInfo {
    pub fn get_obj(&self) -> MinObjectiveValue {
        self.min_objective
    }
}

pub enum StatusFlat {
    Sat,
    Optimum,
    Unsat,
    TimeLimit,
    Unknown,
}

impl StatusFlat {
    pub fn from_parsed_status(status: &ParsedStatus) -> Self {
        match status {
            ParsedStatus::Sat { .. } => StatusFlat::Sat,
            ParsedStatus::Optimum { .. } => StatusFlat::Optimum,
            ParsedStatus::Unsat { .. } => StatusFlat::Unsat,
            ParsedStatus::TimeLimit => StatusFlat::TimeLimit,
            ParsedStatus::Unknown { .. } => StatusFlat::Unknown,
        }
    }

    pub fn get_status_as_str(&self) -> &'static str {
        match self {
            StatusFlat::Sat => "sat",
            StatusFlat::Optimum => "optimum",
            StatusFlat::Unsat => "unsat",
            StatusFlat::TimeLimit => "timeout",
            StatusFlat::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Error)]
pub enum BugReason {
    #[error(
        "Different objectives: Claimed {}, but better objective found was: {} ",
        claimed,
        better
    )]
    DifferentObjectives {
        claimed: ObjectiveInfo,
        better: MinObjectiveValue,
    },
    #[error("Solver {} claimed UNSAT but SAT solution was found!", solver_name)]
    WrongUnsat { solver_name: String },
    #[error(
        "Solver {} claimed UNKNOWN but other solvers found a solution",
        solver_name
    )]
    UnknownWhileDecidable { solver_name: String },
    #[error(
        "Solver {} returned SAT result but an OPTIMUM was expected",
        solver_name
    )]
    SatButExpectedOptimum { solver_name: String },
    #[error(transparent)]
    ProofCheckerFailed(#[from] ProofError),
    #[error(transparent)]
    InvalidModel(#[from] ValidationError),
}

impl BugReason {
    pub fn classify_bug(&self) -> &'static str {
        match self {
            BugReason::DifferentObjectives { .. } => "different_objectives",
            BugReason::WrongUnsat { .. }
            | BugReason::UnknownWhileDecidable { .. }
            | BugReason::SatButExpectedOptimum { .. } => "different_status",
            BugReason::ProofCheckerFailed(e) => e.short_str(),
            BugReason::InvalidModel(e) => e.short_str(),
        }
    }
}

pub fn verify_solution(status: &ParsedStatus, grammar: &PBGrammar) -> Result<(), ValidationError> {
    match status {
        ParsedStatus::Sat {
            conflicts: _,
            solution,
        } => grammar.verify_solution(solution).map_err(|e| e.into()),
        ParsedStatus::Optimum {
            conflicts: _,
            solution,
            minimum,
        } => grammar
            .verify_solution(solution)
            .map_err(|e| e.into())
            .and_then(|_| {
                grammar
                    .verify_min_constraint(solution, *minimum)
                    .map_err(|e| e.into())
            }),
        ParsedStatus::Unsat { conflicts: _ } => Ok(()),
        ParsedStatus::TimeLimit => Ok(()),
        ParsedStatus::Unknown { status: _ } => Ok(()),
    }
}
