use std::{collections::HashSet, fmt::Display, path::Path, process::Output};

use crate::{
    fuzzer::{
        domain::{AtomIdent, MinObjectiveValue},
        grammar::PBGrammar,
    },
    orchestrator::folders::{BUG_FOLDER, ERROR_FOLDER, SAT_FOLDER, UNKNOWN_FOLDER, UNSAT_FOLDER},
};

/// Shared behavior in order to run an external program and extract the output printed to console
pub trait SolverRunner : Send + Sync {
    /// This method should execute a program and return the console output
    ///
    /// file_path: the PBS or PBO file, we want to solve.
    /// returns: the raw console output or some error while executing the program
    fn run(&self, file_path: &Path) -> Result<Output, ExternalProgramError>;
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
pub fn run_pbs<P>(program: &P, file_path: &Path) -> Result<SolverResultPBS, ExternalProgramError>
where
    P: SolverRunner + PBSProcessor,
{
    println!("Solving generated instance with {}", program.get_name());
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
pub fn run_pbo<P>(program: &P, file_path: &Path) -> Result<SolverResultPBO, ExternalProgramError>
where
    P: SolverRunner + PBOProcessor,
{
    println!("Solving generated instance with {}", program.get_name());
    program
        .run(file_path)
        .and_then(|output| program.extract_result_from_output(output))
}

pub struct PBSValues {
    true_vars: HashSet<AtomIdent>,
}

impl SatSolutionVerifyer for PBSValues {
    fn verify_model(&self, grammar: &PBGrammar) -> bool {
        grammar.verify_solution(&self.true_vars)
    }

    fn has_same_min_objective(&self, _: &Self) -> bool {
        // pbs does not track min objective
        true
    }
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

impl SatSolutionVerifyer for PBOValues {
    fn verify_model(&self, grammar: &PBGrammar) -> bool {
        self.pbs_values.verify_model(grammar)
            && grammar.verify_min_constraint(&self.pbs_values.true_vars, &self.min_objective_value)
    }

    fn has_same_min_objective(&self, other: &Self) -> bool {
        self.min_objective_value == other.min_objective_value
    }
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

pub trait SatSolutionVerifyer {
    fn verify_model(&self, grammar: &PBGrammar) -> bool;
    fn has_same_min_objective(&self, other: &Self) -> bool;
}

pub type SolverResultPBS = SolverStatus<PBSValues>;
pub type SolverResultPBO = SolverStatus<PBOValues>;

#[derive(Debug, Clone)]
pub enum SolverStatus<M> {
    Sat { conflicts: u32, solution: M },
    Unsat { conflicts: u32 },
    Unknown,
}

impl<M: SatSolutionVerifyer> SolverStatus<M> {
    pub fn compare_status(&self, other: &SolverStatus<M>, grammar: &PBGrammar) -> IterationOutcome {
        match (self, other) {
            (
                SolverStatus::Sat {
                    conflicts: c1,
                    solution: s1,
                },
                SolverStatus::Sat {
                    // Currently we don't track conflicts for scip
                    conflicts: _,
                    solution: s2,
                },
            ) => {
                let v1 = s1.verify_model(grammar);
                if !v1 {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution,
                    };
                }
                let v2 = s2.verify_model(grammar);
                if !v2 {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution,
                    };
                }
                if s1.has_same_min_objective(s2) {
                    return IterationOutcome::Sat { conflicts: *c1 };
                } else {
                    return IterationOutcome::Bug {
                        reason: BugReason::DifferentObjectives,
                    };
                }
            }
            (SolverStatus::Unsat { conflicts: c1 }, SolverStatus::Unsat { conflicts: _ }) => {
                IterationOutcome::Unsat { conflicts: *c1 }
            }
            (SolverStatus::Unknown, SolverStatus::Unknown) => IterationOutcome::Unknown,
            _ => IterationOutcome::Bug {
                reason: BugReason::OutputStatusDiffers,
            },
        }
    }
}

pub enum IterationOutcome {
    Sat { conflicts: u32 },
    Unsat { conflicts: u32 },
    Unknown,
    Bug { reason: BugReason },
    Error { error: ExternalProgramError },
}

impl IterationOutcome {
    pub fn get_folder_name(&self) -> &'static str {
        match self {
            IterationOutcome::Sat { .. } => SAT_FOLDER.name,
            IterationOutcome::Unsat { .. } => UNSAT_FOLDER.name,
            IterationOutcome::Unknown => UNKNOWN_FOLDER.name,
            IterationOutcome::Bug { .. } => BUG_FOLDER.name,
            IterationOutcome::Error { .. } => ERROR_FOLDER.name,
        }
    }
    pub fn get_file_name_prefix(&self) -> &'static str {
        self.get_status_as_string()
    }

    pub fn get_status_as_string(&self) -> &'static str {
        match self {
            IterationOutcome::Sat { .. } => "sat",
            IterationOutcome::Unsat { .. } => "unsat",
            IterationOutcome::Unknown => "unknown",
            IterationOutcome::Bug { .. } => "bug",
            IterationOutcome::Error { .. } => "error",
        }
    }

    pub fn get_conflicts(&self) -> u32 {
        match self {
            IterationOutcome::Sat { conflicts } => *conflicts,
            IterationOutcome::Unsat { conflicts } => *conflicts,
            IterationOutcome::Unknown => 0,
            IterationOutcome::Bug { .. } => 0,
            IterationOutcome::Error { .. } => 0,
        }
    }
}

pub enum BugReason {
    InvalidSolution,
    DifferentObjectives,
    OutputStatusDiffers,
}

impl Display for BugReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BugReason::InvalidSolution => "Invalid solution",
                BugReason::DifferentObjectives => "Different objectives founds",
                BugReason::OutputStatusDiffers => "Output status differs",
            }
        )
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
