use std::{collections::HashSet, fmt::Display, path::PathBuf, process::Output};

use crate::fuzzer::domain::{AtomIdent, MinObjectiveValue};

/// Shared behavior in order to run an external program and extract the output printed to console
pub trait SolverRunner {
    /// This method should execute a program and return the console output
    ///
    /// file_path: the PBS or PBO file, we want to solve.
    /// returns: the raw console output or some error while executing the program
    fn run(&self, file_path: &PathBuf) -> Result<Output, ExternalProgramError>;
    /// Simply the name of the binary/program we are running
    fn get_name(&self) -> String;
}

/// Parsing Output and extracting stuff relevant for PBS analysis
pub trait PBSProcessor {
    /// This method parses the output and extracts all information needed for PBS validation
    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBS, ExternalProgramError>;
}

/// Glue code for conveniently running program and extracting output
///
/// Provide implementations for running an external program and extracting a result from
/// the output
pub fn run_pbs<P>(program: &P, file_path: &PathBuf) -> Result<SolverResultPBS, ExternalProgramError>
where
    P: SolverRunner + PBSProcessor,
{
    program
        .run(file_path)
        .and_then(|output| program.extract_result_from_output(output))
}

/// Parsing Output and extracting stuff relevant for BPO analysis
pub trait PBOProcessor {
    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBO, ExternalProgramError>;
}
///
/// Glue code for conveniently running program and extracting output
///
/// Provide implementations for running an external program and extracting a result from
/// the output
pub fn run_pbo<P>(program: &P, file_path: &PathBuf) -> Result<SolverResultPBO, ExternalProgramError>
where
    P: SolverRunner + PBOProcessor,
{
    program
        .run(file_path)
        .and_then(|output| program.extract_result_from_output(output))
}

pub struct PBSValues {
    true_vars: HashSet<AtomIdent>,
}

impl PBSValues {
    pub fn new(true_vars: HashSet<AtomIdent>) -> Self {
        Self { true_vars }
    }

    pub fn get_true_model(&self) -> &HashSet<AtomIdent> {
        &self.true_vars
    }
}

pub struct PBOValues {
    pbs_values: PBSValues,
    min_objective_value: MinObjectiveValue,
}

impl PBOValues {
    pub fn new(pbs_values: PBSValues, min_objective_value: MinObjectiveValue) -> Self {
        Self {
            pbs_values,
            min_objective_value,
        }
    }

    pub fn get_true_model(&self) -> &HashSet<AtomIdent> {
        &self.pbs_values.true_vars
    }

    pub fn get_min_objective_values(&self) -> &MinObjectiveValue {
        &self.min_objective_value
    }
}

pub type SolverResultPBS = SolverResult<PBSValues>;
pub type SolverResultPBO = SolverResult<PBOValues>;

pub struct SolverResult<M> {
    status: SolverStatus<M>,
    conflicts: u32,
}

impl<M> SolverResult<M> {
    pub fn get_conflicts(&self) -> u32 {
        self.conflicts
    }

    pub fn get_status(&self) -> &SolverStatus<M> {
        &self.status
    }
}

impl SolverResultPBO {
    pub fn new(status: SolverStatus<PBOValues>, conflicts: u32) -> Self {
        Self { status, conflicts }
    }
}

impl SolverResultPBS {
    pub fn new(status: SolverStatus<PBSValues>, conflicts: u32) -> Self {
        Self { status, conflicts }
    }
}

#[derive(Debug, Clone)]
pub enum SolverStatus<M> {
    Sat(M),
    Unsat,
    Unknown,
}

impl<M> SolverStatus<M> {
    pub fn has_same_status(&self, other: &SolverStatus<M>) -> bool {
        match (self, other) {
            // Different solvers can possibly find different sat models.
            // Ignore when just comparing status
            (SolverStatus::Sat(_), SolverStatus::Sat(_)) => true,
            (SolverStatus::Unsat, SolverStatus::Unsat) => true,
            (SolverStatus::Unknown, SolverStatus::Unknown) => true,
            _ => false,
        }
    }

    pub fn get_status_string(&self) -> String {
        match self {
            SolverStatus::Sat(_) => "SAT",
            SolverStatus::Unsat => "UNSAT",
            SolverStatus::Unknown => "UNKNOWN",
        }
        .to_string()
    }

    pub fn is_sat(&self) -> bool {
        match self {
            SolverStatus::Sat(_) => true,
            SolverStatus::Unsat => false,
            SolverStatus::Unknown => false,
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
