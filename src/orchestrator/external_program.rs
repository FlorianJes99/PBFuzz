use std::{
    fmt::Display,
    io,
    ops::Not,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread::{self},
    time::Duration,
};

use log::info;
use serde::Deserialize;

use crate::{
    fuzzer::{
        domain::{MinObjectiveValue, SolutionModel},
        grammar::{PBGrammar, PBKind},
    },
    orchestrator::{
        default_parser::DefaultParser,
        folders::{
            BUG_FOLDER, ERROR_FOLDER, SAT_FOLDER, TIMEOUT_FOLDER, UNKNOWN_FOLDER, UNSAT_FOLDER,
        },
        modes::proof_checking_mode::check_proof,
        parser::{Parser, ParserKind},
        scip_parser::ScipParser,
    },
};

#[derive(Deserialize)]
pub struct ExternalProgramSer {
    binary: PathBuf,
    args: Vec<String>,
    name: String,
    parser_kind: ParserKind,
    proof_arg: Option<String>,
}

impl ExternalProgramSer {
    pub fn to_external_program(self, timelimit: Duration) -> ExternalProgram {
        ExternalProgram {
            parser: self.create_parser(),
            binary: self.binary,
            args: self.args,
            name: self.name,
            proof_arg: self.proof_arg,
            timelimit,
        }
    }
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let s = std::fs::read_to_string(path)?;
        toml::from_str(&s).map_err(std::io::Error::other)
    }

    fn create_parser(&self) -> Box<dyn Parser> {
        match self.parser_kind {
            ParserKind::Default => Box::new(DefaultParser::new(self.name.clone())),
            ParserKind::Scip => Box::new(ScipParser::new(self.name.clone())),
        }
    }
}

pub struct ExternalProgram {
    binary: PathBuf,
    args: Vec<String>,
    name: String,
    parser: Box<dyn Parser>,
    proof_arg: Option<String>,
    timelimit: Duration,
}

impl ExternalProgram {
    pub fn run_with_proof(
        &self,
        file_path_pb: &Path,
        file_path_proof: &Path,
    ) -> Result<ExternalProgramResult, ExternalProgramError> {
        let mut command = Command::new(self.binary.as_os_str());
        // TODO: remove later, when profiling analysis is done
        let prof_file_name = file_path_pb
            .file_stem()
            .expect("PB File should exist")
            .display();
        add_profiling_to_command(&mut command, &prof_file_name);

        let proof_arg = self.proof_arg.as_ref().ok_or_else(|| {
            ExternalProgramError::new(
                self.get_name().to_string(),
                "No argument provided to generate a proof".to_string(),
                ErrorKind::MissingProofLoggingArgument,
            )
        })?;
        command.arg(format!("{proof_arg}{}", file_path_proof.display()));

        for arg in &self.args {
            command.arg(arg);
        }

        command.arg(file_path_pb);
        self.run_with_timeout(&mut command, self.timelimit)
            .map_err(|e| self.external_execution_error(e.to_string()))
    }

    pub fn run(&self, file_path: &Path) -> Result<ExternalProgramResult, ExternalProgramError> {
        let mut command = Command::new(self.binary.as_os_str());
        for arg in &self.args {
            command.arg(arg);
        }
        command.arg(file_path);
        self.run_with_timeout(&mut command, self.timelimit)
            .map_err(|e| self.external_execution_error(e.to_string()))
    }

    pub fn supports_proofs(&self) -> bool {
        self.proof_arg.is_some()
    }

    pub fn parse(
        &self,
        ext_result: ExternalProgramResult,
        pb_kind: &PBKind,
    ) -> Result<SolverStatus, ExternalProgramError> {
        match ext_result {
            ExternalProgramResult::Terminated(output) => match pb_kind {
                PBKind::PBS => self.parser.parse_pbs(&output),
                PBKind::PBO => self.parser.parse_pbo(&output),
            },
            ExternalProgramResult::Timeout => Ok(SolverStatus::TimeLimit),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    fn external_execution_error(&self, message: impl Into<String>) -> ExternalProgramError {
        ExternalProgramError::new(
            self.name.clone(),
            message.into(),
            ErrorKind::ProgramExecutionFailed,
        )
    }
    fn run_with_timeout(
        &self,
        cmd: &mut Command,
        timeout: Duration,
    ) -> io::Result<ExternalProgramResult> {
        let child = cmd.stdout(Stdio::piped()).spawn()?;
        let (tx, rx) = mpsc::channel();
        let pid = child.id();

        thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });

        match rx.recv_timeout(timeout) {
            Ok(result) => result.map(ExternalProgramResult::Terminated),
            Err(_) => {
                info!("Terminating solver {}...", self.get_name());
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                Ok(ExternalProgramResult::Timeout)
            }
        }
    }
}

pub enum ExternalProgramResult {
    Terminated(Output),
    Timeout,
}

fn add_profiling_to_command(command: &mut Command, prof_file_name: &impl Display) {
    if PathBuf::from("profiles/").exists().not() {
        panic!("Folder `Profiles` needs to exist!")
    }
    let prof_file = format!("profiles/{prof_file_name}.profraw");
    command.env("LLVM_PROFILE_FILE", prof_file);
}

#[derive(Debug, Clone)]
pub enum SolverSolution {
    PBS(SolutionModel),
    PBO(SolutionModel, MinObjectiveValue),
}

impl SolverSolution {
    fn is_valid(&self, grammar: &PBGrammar) -> bool {
        match self {
            SolverSolution::PBS(solution_model) => grammar.verify_solution(solution_model),
            SolverSolution::PBO(solution_model, min_objective_value) => {
                grammar.verify_solution(solution_model)
                    && grammar.verify_min_constraint(solution_model, min_objective_value)
            }
        }
    }
    fn has_same_min_objective(&self, other: &SolverSolution) -> bool {
        match (self, other) {
            (SolverSolution::PBS(_), SolverSolution::PBS(_)) => true,
            (SolverSolution::PBO(_, o1), SolverSolution::PBO(_, o2)) => o1 == o2,
            _ => unreachable!(
                "PBS should not be compared to PBO values: {:#?}, {:#?}",
                self, other
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SolverStatus {
    Sat {
        conflicts: u32,
        solution: SolverSolution,
    },
    Unsat {
        conflicts: u32,
    },
    TimeLimit,
    Unknown,
}

impl SolverStatus {
    pub fn verify_with_proof_checking(
        &self,
        grammar: &PBGrammar,
        formula: &Path,
        derivation: &Path,
    ) -> IterationOutcome {
        match self {
            SolverStatus::Sat {
                conflicts,
                solution,
            } => {
                if !solution.is_valid(grammar) {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution,
                    };
                }
                if !check_proof(formula, derivation) {
                    return IterationOutcome::Bug {
                        reason: BugReason::ProofCheckerFailed,
                    };
                }
                IterationOutcome::Sat {
                    conflicts: *conflicts,
                }
            }
            SolverStatus::Unsat { conflicts } => IterationOutcome::Unsat {
                conflicts: *conflicts,
            },
            SolverStatus::TimeLimit => IterationOutcome::TimeLimt,
            SolverStatus::Unknown => IterationOutcome::Unknown,
        }
    }
    pub fn verify_and_compare_status(
        &self,
        other: &SolverStatus,
        grammar: &PBGrammar,
    ) -> IterationOutcome {
        match (self, other) {
            (
                SolverStatus::Sat {
                    conflicts: c1,
                    solution: s1,
                },
                SolverStatus::Sat {
                    // TODO: Currently we don't track conflicts for scip
                    conflicts: _,
                    solution: s2,
                },
            ) => {
                let v1 = s1.is_valid(grammar);
                if !v1 {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution,
                    };
                }
                let v2 = s2.is_valid(grammar);
                if !v2 {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution,
                    };
                }
                if s1.has_same_min_objective(s2) {
                    IterationOutcome::Sat { conflicts: *c1 }
                } else {
                    IterationOutcome::Bug {
                        reason: BugReason::DifferentObjectives,
                    }
                }
            }
            (SolverStatus::Unsat { conflicts: c1 }, SolverStatus::Unsat { conflicts: _ }) => {
                IterationOutcome::Unsat { conflicts: *c1 }
            }
            (SolverStatus::Unknown, SolverStatus::Unknown) => IterationOutcome::Unknown,
            (SolverStatus::TimeLimit, _) => IterationOutcome::TimeLimt,
            (_, SolverStatus::TimeLimit) => IterationOutcome::TimeLimt,
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
    TimeLimt,
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
            IterationOutcome::TimeLimt => TIMEOUT_FOLDER.name,
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
            IterationOutcome::TimeLimt => "timeout",
        }
    }

    pub fn get_conflicts(&self) -> u32 {
        match self {
            IterationOutcome::Sat { conflicts } => *conflicts,
            IterationOutcome::Unsat { conflicts } => *conflicts,
            IterationOutcome::Unknown => 0,
            IterationOutcome::Bug { .. } => 0,
            IterationOutcome::Error { .. } => 0,
            IterationOutcome::TimeLimt => 0,
        }
    }
}

pub enum BugReason {
    InvalidSolution,
    DifferentObjectives,
    OutputStatusDiffers,
    ProofCheckerFailed,
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
                BugReason::ProofCheckerFailed => "Proof Checker disagreed with solver solution",
            }
        )
    }
}

#[derive(Debug)]
pub enum ErrorKind {
    ProgramExecutionFailed,
    OutputParsingError,
    MissingProofLoggingArgument,
}

#[derive(Debug)]
pub struct ExternalProgramError {
    program_name: String,
    error_message: String,
    error_kind: ErrorKind,
}

impl std::error::Error for ExternalProgramError {}

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

impl Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ErrorKind::ProgramExecutionFailed => "Program Execution Error",
                ErrorKind::OutputParsingError => "Output Parsing Error",
                ErrorKind::MissingProofLoggingArgument =>
                    "Proof logging not supplied in program configuration",
            }
        )
    }
}
