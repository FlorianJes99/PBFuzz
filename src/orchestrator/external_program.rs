use std::{
    fmt::Display,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde::{Deserialize, Serialize};

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
        parser::{Parser, ParserKind},
        scip_parser::ScipParser,
    },
};

#[derive(Serialize, Deserialize)]
pub struct ExternalProgramSer {
    binary: PathBuf,
    args: Vec<String>,
    name: String,
    parser_kind: ParserKind,
}

impl ExternalProgramSer {
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
}

impl From<ExternalProgramSer> for ExternalProgram {
    fn from(value: ExternalProgramSer) -> Self {
        ExternalProgram {
            parser: value.create_parser(),
            binary: value.binary,
            args: value.args,
            name: value.name,
        }
    }
}

impl ExternalProgram {
    pub fn run(&self, file_path: &Path) -> Result<Output, ExternalProgramError> {
        let mut command = Command::new(self.binary.as_os_str());
        for arg in &self.args {
            command.arg(arg);
        }
        command.arg(file_path).output().map_err(|e| {
            ExternalProgramError::new(
                self.get_name().to_string(),
                e.to_string(),
                ErrorKind::ProgramExecutionFailed,
            )
        })
    }

    pub fn parse(
        &self,
        output: Output,
        pb_kind: &PBKind,
    ) -> Result<SolverStatus, ExternalProgramError> {
        match pb_kind {
            PBKind::PBS => self.parser.parse_pbs(&output),
            PBKind::PBO => self.parser.parse_pbo(&output),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub enum SolverSolution {
    PBS(SolutionModel),
    PBO(SolutionModel, MinObjectiveValue),
}

impl SolverSolution {
    fn get_model(&self) -> &SolutionModel {
        match self {
            SolverSolution::PBS(solution_model) => solution_model,
            SolverSolution::PBO(solution_model, _) => solution_model,
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
    pub fn verify(&self, grammar: &PBGrammar) -> bool {
        if let SolverStatus::Sat {
            conflicts: _,
            solution: s,
        } = self
        {
            match s {
                SolverSolution::PBS(solution_model) => grammar.verify_solution(solution_model),
                SolverSolution::PBO(solution_model, min_objective_value) => {
                    grammar.verify_solution(solution_model)
                        && grammar.verify_min_constraint(solution_model, min_objective_value)
                }
            }
        } else {
            true
        }
    }
    pub fn compare_status(&self, other: &SolverStatus, grammar: &PBGrammar) -> IterationOutcome {
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
                let v1 = grammar.verify_solution(s1.get_model());
                if !v1 {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution,
                    };
                }
                let v2 = grammar.verify_solution(s2.get_model());
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
pub enum ErrorKind {
    ProgramExecutionFailed,
    OutputParsingError,
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
            }
        )
    }
}
