use std::fmt::Display;

use thiserror::Error;

use crate::{
    fuzzer::{
        constraint::ConstraintViolation, domain::MinObjectiveValue, grammar::MinObjectiveInvalid,
    },
    orchestrator::{
        evaluation::proof_checking::ProofError,
        folders::{BUG_FOLDER, SAT_FOLDER, TIMEOUT_FOLDER, UNKNOWN_FOLDER, UNSAT_FOLDER},
        parsing::parser::ParsedStatus,
        solving::solver::SolverExecutionRecord,
    },
};

pub struct VerificationData {
    pub solver_data: SolverExecutionRecord,
    pub parser_data: ParsedStatus,
}

pub enum VerificationOutcome {
    Sat,
    Unsat,
    Timelimit,
    Bug(BugReason),
    Unknown,
}

impl Display for VerificationOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationOutcome::Bug(bug_reason) => writeln!(f, "Bug: {}", bug_reason),
            _ => writeln!(f, "Status: {}", self.get_status_as_string()),
        }
    }
}

impl VerificationOutcome {
    pub fn get_status_as_string(&self) -> &'static str {
        match self {
            VerificationOutcome::Sat => "SAT",
            VerificationOutcome::Unsat => "UNSAT",
            VerificationOutcome::Timelimit => "TIMELIMIT",
            VerificationOutcome::Bug(_) => "BUG",
            VerificationOutcome::Unknown => "UNKNOWN",
        }
    }

    pub fn get_folder_name(&self) -> &'static str {
        match self {
            VerificationOutcome::Sat => SAT_FOLDER.name,
            VerificationOutcome::Unsat => UNSAT_FOLDER.name,
            VerificationOutcome::Timelimit => TIMEOUT_FOLDER.name,
            VerificationOutcome::Bug(_) => BUG_FOLDER.name,
            VerificationOutcome::Unknown => UNKNOWN_FOLDER.name,
        }
    }

    pub fn get_file_name_prefix(&self) -> &'static str {
        self.get_folder_name()
    }
}

pub fn wrap_violation(violation: ConstraintViolation, solver_name: String) -> BugReason {
    BugReason::Solution(InvalidSolution {
        solver_name,
        violation: violation.into(),
    })
}

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
    Objective(#[from] MinObjectiveInvalid),
}

#[derive(Debug, Error)]
#[error("{solver_name} - minimum: {min_objective}")]
pub struct ObjectiveInfo {
    solver_name: String,
    min_objective: MinObjectiveValue,
}

#[derive(Debug, Error)]
#[error("{solver_name}: {violation}")]
pub struct InvalidSolution {
    solver_name: String,
    #[source]
    violation: ValidationError,
}

#[derive(Debug, Error)]
pub enum BugReason {
    #[error("Invalid solution: {0}")]
    Solution(InvalidSolution),
    #[error("Different objectives in solver: {0} and {1}")]
    DifferentObjectives(ObjectiveInfo, ObjectiveInfo),
    #[error("Output status differs: {0} != {1}")]
    OutputStatusDiffers(String, String),
    #[error(transparent)]
    ProofCheckerFailed(ProofError),
}
