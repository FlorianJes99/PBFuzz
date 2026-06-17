use std::{
    fmt::Display,
    io,
    ops::{Not, RangeInclusive},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread::{self},
    time::Duration,
};

use log::info;
use rand::{RngExt, seq::IndexedRandom};
use serde::Deserialize;
use thiserror::Error;

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
        parser::{OutputParsingError, Parser, ParserKind},
        scip_parser::ScipParser,
    },
};

pub trait Solver {
    fn get_name(&self) -> &str;

    fn external_execution_error(&self, source: std::io::Error) -> ExternalProgramError {
        ExternalProgramError::ExecutionFailed {
            program: self.get_name().into(),
            source,
        }
    }

    fn parser(&self) -> &dyn Parser;

    fn parse(
        &self,
        ext_result: SolverOutcome,
        pb_kind: &PBKind,
    ) -> Result<SolverStatus, ExternalProgramError> {
        match ext_result {
            SolverOutcome::Terminated(output) => match pb_kind {
                PBKind::PBS => self.parser().parse_pbs(&output),
                PBKind::PBO => self.parser().parse_pbo(&output),
            }
            .map_err(Into::into),
            SolverOutcome::Timeout => Ok(SolverStatus::TimeLimit),
        }
    }

    fn run_with_timeout(&self, cmd: &mut Command, timeout: Duration) -> io::Result<SolverOutcome> {
        let child = cmd.stdout(Stdio::piped()).spawn()?;
        let (tx, rx) = mpsc::channel();
        let pid = child.id();

        thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });

        match rx.recv_timeout(timeout) {
            Ok(result) => result.map(SolverOutcome::Terminated),
            Err(_) => {
                info!("Terminating solver {}...", self.get_name());
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                Ok(SolverOutcome::Timeout)
            }
        }
    }
}

#[derive(Deserialize)]
pub struct ExternalProgramSer {
    binary: PathBuf,
    args: Vec<String>,
    optional_args: Vec<OptionalArg>,
    name: String,
    parser_kind: ParserKind,
    proof_arg: Option<String>,
}

#[derive(Deserialize)]
struct OptionalArg {
    arg_name: String,
    arg_type: Option<ArgumentType>,
    /// if arg name and value are seperated by spaces, they need to be provided as separate arguments to the `Command` struct
    /// default to false
    #[serde(default)]
    space_separated: bool,
}

impl OptionalArg {
    fn collect_into_string_args(&self, rng: &mut impl RngExt) -> Vec<String> {
        let arg_type_as_string = self
            .arg_type
            .as_ref()
            .map(|t| t.sample_to_string(rng))
            .unwrap_or_default();
        if self.space_separated {
            vec![self.arg_name.clone(), arg_type_as_string]
        } else {
            vec![format!("{}{}", self.arg_name, arg_type_as_string)]
        }
    }
}

#[derive(Deserialize)]
#[serde(try_from = "ArgumentTypeRaw")]
enum ArgumentType {
    Bool,
    IntRange(RangeInclusive<i32>),
    FloatRange(RangeInclusive<f32>),
    Values(Vec<String>),
}

impl TryFrom<ArgumentTypeRaw> for ArgumentType {
    type Error = ExternalProgramError;

    fn try_from(value: ArgumentTypeRaw) -> Result<Self, Self::Error> {
        match value {
            ArgumentTypeRaw::Bool => Ok(ArgumentType::Bool),
            ArgumentTypeRaw::Values(items) => Ok(ArgumentType::Values(items)),
            ArgumentTypeRaw::IntRange(i1, i2) => {
                if i1 > i2 {
                    Err(ExternalProgramError::InvalidIntRange(i1, i2))
                } else {
                    Ok(ArgumentType::IntRange(i1..=i2))
                }
            }
            ArgumentTypeRaw::FloatRange(f1, f2) => {
                if f1 > f2 {
                    Err(ExternalProgramError::InvalidFloatRange(f1, f2))
                } else {
                    Ok(ArgumentType::FloatRange(f1..=f2))
                }
            }
        }
    }
}

#[derive(Deserialize)]
enum ArgumentTypeRaw {
    Bool,
    IntRange(i32, i32),
    FloatRange(f32, f32),
    Values(Vec<String>),
}

impl ArgumentType {
    fn sample_to_string(&self, rng: &mut impl RngExt) -> String {
        match self {
            ArgumentType::Bool => (rng.random_bool(0.5) as u8).to_string(),
            ArgumentType::IntRange(r) => rng.random_range(r.clone()).to_string(),
            ArgumentType::FloatRange(r) => rng.random_range(r.clone()).to_string(),
            ArgumentType::Values(items) => items.choose(rng).cloned().unwrap_or_default(),
        }
    }
}

impl ExternalProgramSer {
    pub fn to_external_program(self, timelimit: Duration) -> ExternalProgram {
        ExternalProgram {
            parser: self.create_parser(),
            binary: self.binary,
            args: self.args,
            optional_args: self.optional_args,
            name: self.name,
            timelimit,
        }
    }

    pub fn to_external_program_with_proof(
        self,
        timelimit: Duration,
    ) -> Result<ExternalProgramWithProof, ExternalProgramError> {
        match self.proof_arg.as_ref() {
            Some(proof_arg) => Ok(ExternalProgramWithProof {
                proof_arg: proof_arg.to_string(),
                program: self.to_external_program(timelimit),
            }),
            None => Err(ExternalProgramError::MissingProofLoggingArgument { program: self.name }),
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
    /// path to the binary of the solver to execute
    binary: PathBuf,
    /// essential args needed in order to run the solver
    args: Vec<String>,
    /// optional arguments, which are used to implement option fuzzing
    optional_args: Vec<OptionalArg>,
    /// name of the solver, used as an identifier in info/error messages
    name: String,
    /// parser used to read output of the solver
    parser: Box<dyn Parser>,
    /// terminate running process of the solver after this duration
    timelimit: Duration,
}

pub struct ExternalProgramWithProof {
    program: ExternalProgram,
    proof_arg: String,
}

impl Solver for ExternalProgramWithProof {
    fn get_name(&self) -> &str {
        &self.program.name
    }

    fn parser(&self) -> &dyn Parser {
        &*self.program.parser
    }
}

impl ExternalProgramWithProof {
    pub fn run_with_proof(
        &self,
        file_path_pb: &Path,
        file_path_proof: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverOutcome, ExternalProgramError> {
        let mut command = Command::new(self.program.binary.as_os_str());
        // TODO: remove later, when profiling analysis is done
        let prof_file_name = file_path_pb
            .file_stem()
            .expect("PB File should exist")
            .display();
        add_profiling_to_command(&mut command, &prof_file_name);

        command.arg(format!("{}{}", self.proof_arg, file_path_proof.display()));

        for arg in &self.program.args {
            command.arg(arg);
        }

        if with_option_fuzzing {
            for arg in &self.program.sample_optional_args() {
                // TODO: what to do if arg look like this `--myarg 10`
                // Command structure wants every space seperated input as seperarate arg
                // roundingsat conects arg name and arg with =
                command.arg(arg);
            }
        }

        command.arg(file_path_pb);
        self.run_with_timeout(&mut command, self.program.timelimit)
            .map_err(|e| self.external_execution_error(e))
    }
}

impl Solver for ExternalProgram {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn parser(&self) -> &dyn Parser {
        &*self.parser
    }
}

impl ExternalProgram {
    pub fn run(
        &self,
        file_path: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverOutcome, ExternalProgramError> {
        let mut command = Command::new(self.binary.as_os_str());
        for arg in &self.args {
            command.arg(arg);
        }

        if with_option_fuzzing {
            for arg in &self.sample_optional_args() {
                // TODO: what to do if arg look like this `--myarg 10`
                // Command structure wants every space seperated input as seperarate arg
                // roundingsat conects arg name and arg with =
                command.arg(arg);
            }
        }
        command.arg(file_path);
        self.run_with_timeout(&mut command, self.timelimit)
            .map_err(|e| self.external_execution_error(e))
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    fn sample_optional_args(&self) -> Vec<String> {
        let rng = &mut rand::rng();
        let amount = 3.min(self.optional_args.len());
        self.optional_args
            .sample(rng, amount)
            .flat_map(|arg| arg.collect_into_string_args(rng))
            .collect()
    }
}

pub enum SolverOutcome {
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
        solver_name: &str,
    ) -> IterationOutcome {
        match self {
            SolverStatus::Sat {
                conflicts,
                solution,
            } => {
                if !solution.is_valid(grammar) {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution {
                            solver_name: solver_name.into(),
                        },
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
        //TODO: we probably want to save the name somewhere inside self and other
        self_name: &str,
        other_name: &str,
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
                        reason: BugReason::InvalidSolution {
                            solver_name: self_name.into(),
                        },
                    };
                }
                let v2 = s2.is_valid(grammar);
                if !v2 {
                    return IterationOutcome::Bug {
                        reason: BugReason::InvalidSolution {
                            solver_name: other_name.into(),
                        },
                    };
                }
                match (s1, s2) {
                    (SolverSolution::PBS(_), SolverSolution::PBS(_)) => {
                        IterationOutcome::Sat { conflicts: *c1 }
                    }
                    (SolverSolution::PBO(_, min1), SolverSolution::PBO(_, min2)) => {
                        if min1 == min2 {
                            IterationOutcome::Sat { conflicts: *c1 }
                        } else {
                            IterationOutcome::Bug {
                                reason: BugReason::DifferentObjectives(min1.clone(), min2.clone()),
                            }
                        }
                    }
                    _ => unreachable!("cant compare pbs with pbo"),
                }
            }
            (SolverStatus::Unsat { conflicts: c1 }, SolverStatus::Unsat { conflicts: _ }) => {
                IterationOutcome::Unsat { conflicts: *c1 }
            }
            (SolverStatus::Unknown, SolverStatus::Unknown) => IterationOutcome::Unknown,
            (SolverStatus::TimeLimit, _) => IterationOutcome::TimeLimt,
            (_, SolverStatus::TimeLimit) => IterationOutcome::TimeLimt,
            _ => IterationOutcome::Bug {
                reason: BugReason::OutputStatusDiffers(
                    self.get_status_string().into(),
                    other.get_status_string().into(),
                ),
            },
        }
    }

    fn get_status_string(&self) -> &'static str {
        match self {
            SolverStatus::Sat { .. } => "SAT",
            SolverStatus::Unsat { .. } => "UNSAT",
            SolverStatus::TimeLimit => "Timeout",
            SolverStatus::Unknown => "Unknown",
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
    InvalidSolution { solver_name: String },
    DifferentObjectives(MinObjectiveValue, MinObjectiveValue),
    OutputStatusDiffers(String, String),
    ProofCheckerFailed,
}

impl Display for BugReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BugReason::InvalidSolution {
                solver_name: program_name,
            } => {
                write!(f, "Invalid solution provided by solver {program_name}")
            }
            BugReason::DifferentObjectives(m1, m2) => {
                write!(f, "Different objectives found: {m1} and {m2}")
            }
            BugReason::OutputStatusDiffers(s1, s2) => {
                write!(f, "Different status found: {s1} and {s2}")
            }
            BugReason::ProofCheckerFailed => {
                f.write_str("Proof checker showed that generated prove is invalid")
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum ExternalProgramError {
    #[error("{program}: failed to execute: {source}")]
    ExecutionFailed {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{program}: missing required proof logging argument")]
    MissingProofLoggingArgument { program: String },
    #[error("Invalid Range specified: {0} needs to be <= {1}")]
    InvalidIntRange(i32, i32),
    #[error("Invalid Range specified: {0} needs to be <= {1}")]
    InvalidFloatRange(f32, f32),
    #[error("Failed to parse output of solver")]
    Parsing(#[from] OutputParsingError),
}

#[cfg(test)]
mod tests {
    use rand::{SeedableRng, rngs::StdRng};

    use super::*;

    #[test]
    fn test_collect_arg_into_string() {
        let mut rng = StdRng::seed_from_u64(42);
        let arg = OptionalArg {
            arg_name: "--myArg=".into(),
            arg_type: Some(ArgumentType::IntRange(1..=3)),
            space_separated: false,
        };

        let args = arg.collect_into_string_args(&mut rng);
        assert_eq!(1, args.len());
        let (arg_name, arg_type) = args[0].split_once("=").unwrap();
        assert_eq!("--myArg", arg_name);
        assert!(
            arg_type.parse::<i32>().is_ok(),
            "should be able to parse int argument value: {}",
            arg_type
        );
    }
}
