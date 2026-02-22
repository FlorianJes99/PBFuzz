use std::{collections::HashSet, fmt::Display, path::PathBuf};

use crate::fuzzer::domain::AtomIdent;

pub trait Solver {
    fn execute_solver(&self, file_path: &PathBuf) -> Result<SolverResult, ExternalProgramError>;
    fn get_name(&self) -> String;
}
pub struct SolverResult {
    status: SolverStatus,
    conflicts: u32,
}

impl SolverResult {
    pub fn new(status: SolverStatus, conflicts: u32) -> Self {
        Self { status, conflicts }
    }

    pub fn get_status(&self) -> &SolverStatus {
        &self.status
    }

    pub fn get_conflicts(&self) -> u32 {
        self.conflicts
    }
}

#[derive(Debug, Clone)]
pub enum SolverStatus {
    Sat { found_model: HashSet<AtomIdent> },
    Unsat,
    Unknown,
}

impl SolverStatus {
    pub fn has_same_status(&self, other: &SolverStatus) -> bool {
        match (self, other) {
            // Different solvers can possibly find different sat models.
            // Ignore when just comparing status
            (SolverStatus::Sat { found_model: _ }, SolverStatus::Sat { found_model: _ }) => true,
            (SolverStatus::Unsat, SolverStatus::Unsat) => true,
            (SolverStatus::Unknown, SolverStatus::Unknown) => true,
            _ => false,
        }
    }

    pub fn get_status_string(&self) -> String {
        match self {
            SolverStatus::Sat { found_model: _ } => "SAT",
            SolverStatus::Unsat => "UNSAT",
            SolverStatus::Unknown => "UNKNOWN",
        }
        .to_string()
    }

    pub fn is_sat(&self) -> bool {
        match self {
            SolverStatus::Sat { found_model: _ } => true,
            SolverStatus::Unsat =>false ,
            SolverStatus::Unknown =>false,
        }

    }
}

#[derive(Debug)]
pub struct ExternalProgramError {
    program_name: String,
    error_message: String,
    error_kind: ErrorKind,
}

impl ExternalProgramError {
    pub fn new(program_name: String, error_message: String, error_kind: ErrorKind) -> Self {
        Self {
            program_name,
            error_message,
            error_kind,
        }
    }
}

impl Display for ExternalProgramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Error while executing program: {}", self.program_name)?;
        writeln!(f, "Error reason: {}", self.error_kind)?;
        writeln!(f, "Error message: {}", self.error_message)
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    ProgramExecutionFailed,
    OutputParsingError,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorKind::ProgramExecutionFailed => "Program Execution Error",
                ErrorKind::OutputParsingError => "Output Parsing Error",
            }
        )
    }
}
