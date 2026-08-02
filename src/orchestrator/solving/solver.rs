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

use itertools::Itertools;
use log::info;
use rand::{seq::IndexedRandom, RngExt};
use serde::Deserialize;
use thiserror::Error;

use crate::{
    fuzzer::{
        constraint::ConstraintViolation,
        domain::{MinObjectiveValue, SolutionModel},
        grammar::{MinObjectiveInvalid, PBGrammar, PBKind},
    },
    orchestrator::{
        folders::{
            BUG_FOLDER, ERROR_FOLDER, SAT_FOLDER, TIMEOUT_FOLDER, UNKNOWN_FOLDER, UNSAT_FOLDER,
        },
        modes::proof_checking_mode::{check_proof, ProofError},
        parsing::parser::{OutputParsingError, Parser, ParserKind},
    },
};

/// The parsed and verified outcome of the  solved problem provided by a specific PB solver
///
/// The happy path returns the parsed status of the solver output as well as information about found model (if SAT) and
/// minimum objective (if PBO)
/// An valid outcome is either SAT/UNSAT or a timeout of the solver
///
/// Errors can be either a found bug, some error due to parsing or solver crashing and the result "UNKNONW"
pub type SolverResult = Result<SolverOutcome, IterationError>;

/// Trait, which holds methods a specific solver needs to implement in order to be able to run and parse its output
pub trait Solver {
    /// every solver needs to have a name, which works as an identifier
    fn get_name(&self) -> &str;

    fn external_execution_error(&self, source: std::io::Error) -> ExternalProgramError {
        ExternalProgramError::ExecutionFailed {
            program: self.get_name().into(),
            source,
        }
    }

    /// returns the specific parser needed in order to extract the output of the solver
    fn parser(&self) -> &dyn Parser;

    /// Makes use of the underlying parser for the solver in order to extract the computed status of the problem
    fn parse(
        &self,
        ext_result: SolverState,
        pb_kind: &PBKind,
    ) -> Result<ParsedSolverOutput, ExternalProgramError> {
        match ext_result {
            SolverState::Terminated(_, output) => match pb_kind {
                PBKind::PBS => self.parser().parse_pbs(&output),
                PBKind::PBO => self.parser().parse_pbo(&output),
            }
            .map(|s| ParsedSolverOutput::new(self.get_name().into(), s))
            .map_err(Into::into),
            SolverState::Timeout(_) => Ok(ParsedSolverOutput::new(
                self.get_name().into(),
                SolverStatus::TimeLimit,
            )),
        }
    }

    /// starts the solver in a child process and terminates this process after the given output
    ///
    /// Because we use libc crate to kill the child process, this method makes use of `unsafe` code
    fn run_with_timeout(&self, mut cmd: Command, timeout: Duration) -> io::Result<SolverState> {
        let child = cmd.stdout(Stdio::piped()).spawn()?;
        let (tx, rx) = mpsc::channel();
        let pid = child.id();

        thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });

        match rx.recv_timeout(timeout) {
            Ok(result) => result.map(|o| SolverState::Terminated(cmd, o)),
            Err(_) => {
                info!("Terminating solver {}...", self.get_name());
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                Ok(SolverState::Timeout(cmd))
            }
        }
    }
}

#[derive(Deserialize)]
pub struct ExternalProgramSer {
    binary: PathBuf,
    args: Vec<String>,
    #[serde(default)]
    optional_args: OptionalArgs,
    name: String,
    parser_kind: ParserKind,
    proof_arg: Option<String>,
}

#[derive(Deserialize, Default)]
struct OptionalArgs {
    args: Vec<OptionalArg>,
    /// if arg name and value are seperated by spaces, they need to be provided as separate arguments to the `Command` struct
    /// default to false
    #[serde(default)]
    space_separated: bool,
}

#[derive(Deserialize)]
struct OptionalArg {
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
enum ArgumentType {
    IntRange(RangeInclusive<i32>),
    FloatRange(RangeInclusive<f32>),
    Values(Vec<String>),
}

impl TryFrom<ArgumentTypeRaw> for ArgumentType {
    type Error = ExternalProgramError;

    fn try_from(value: ArgumentTypeRaw) -> Result<Self, Self::Error> {
        match value {
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

impl ExternalProgramSer {
    pub fn to_external_program(self, timelimit: Duration) -> SolverData {
        SolverData {
            parser: self.parser_kind.create_parser(self.name.clone()),
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
    ) -> Result<SolverDataWithProof, ExternalProgramError> {
        match self.proof_arg.as_ref() {
            Some(proof_arg) => Ok(SolverDataWithProof {
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
}

/// This struct holds all the relevant data in order to execute a Solver
pub struct SolverData {
    /// path to the binary of the solver to execute
    binary: PathBuf,
    /// essential args needed in order to run the solver
    args: Vec<String>,
    /// optional arguments, which are used to implement option fuzzing
    optional_args: OptionalArgs,
    /// name of the solver, used as an identifier in info/error messages
    name: String,
    /// parser used to read output of the solver
    parser: Box<dyn Parser>,
    /// terminate running process of the solver after this duration
    timelimit: Duration,
}

pub struct SolverDataWithProof {
    program: SolverData,
    proof_arg: String,
}

impl Solver for SolverDataWithProof {
    fn get_name(&self) -> &str {
        &self.program.name
    }

    fn parser(&self) -> &dyn Parser {
        &*self.program.parser
    }
}

impl SolverDataWithProof {
    pub fn run_with_proof(
        &self,
        file_path_pb: &Path,
        file_path_proof: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverState, ExternalProgramError> {
        let mut command = Command::new(self.program.binary.as_os_str());
        // TODO: remove later, when profiling analysis is done
        let _prof_file_name = file_path_pb
            .file_stem()
            .expect("PB File should exist")
            .display();
        // add_profiling_to_command(&mut command, &prof_file_name);

        command.arg(format!("{}{}", self.proof_arg, file_path_proof.display()));

        for arg in &self.program.args {
            command.arg(arg);
        }

        if with_option_fuzzing && !self.program.optional_args.args.is_empty() {
            let sampled_args = &self.program.sample_optional_args();
            info!("Sampled args for this run: {}", sampled_args.join(", "));
            for arg in sampled_args {
                command.arg(arg);
            }
        }

        command.arg(file_path_pb);
        self.run_with_timeout(command, self.program.timelimit)
            .map_err(|e| self.external_execution_error(e))
    }
}

impl Solver for SolverData {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn parser(&self) -> &dyn Parser {
        &*self.parser
    }
}

impl SolverData {
    pub fn run(
        &self,
        file_path: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverState, ExternalProgramError> {
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
        self.run_with_timeout(command, self.timelimit)
            .map_err(|e| self.external_execution_error(e))
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    fn sample_optional_args(&self) -> Vec<String> {
        let rng = &mut rand::rng();
        let amount = 3.min(self.optional_args.args.len());
        self.optional_args
            .args
            .sample(rng, amount)
            .flat_map(|arg| arg.collect_into_string_args(self.optional_args.space_separated, rng))
            .collect()
    }
}

pub enum SolverState {
    Terminated(Command, Output),
    Timeout(Command),
}

impl SolverState {
    pub fn get_command_as_string(&self) -> String {
        let command = match self {
            SolverState::Terminated(command, _) => command,
            SolverState::Timeout(command) => command,
        };
        format!(
            "{} {}",
            command.get_program().display(),
            command.get_args().map(|a| a.display()).join(" ")
        )
    }
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
    fn is_valid(&self, grammar: &PBGrammar) -> Result<(), ValidationError> {
        match self {
            SolverSolution::PBS(solution_model) => grammar
                .verify_solution(solution_model)
                .map_err(|e| e.into()),
            SolverSolution::PBO(solution_model, min_objective_value) => grammar
                .verify_solution(solution_model)
                .map_err(|e| e.into())
                .and_then(|_| {
                    grammar
                        .verify_min_constraint(solution_model, min_objective_value.clone())
                        .map_err(|e| e.into())
                }),
        }
    }
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    Constraint(#[from] ConstraintViolation),
    #[error(transparent)]
    Objective(#[from] MinObjectiveInvalid),
}

pub struct ParsedSolverOutput {
    name: String,
    parsed_status: SolverStatus,
}

impl ParsedSolverOutput {
    pub fn new(name: String, parsed_status: SolverStatus) -> Self {
        Self {
            name,
            parsed_status,
        }
    }

    pub fn verify_and_compare_status(
        self,
        other: ParsedSolverOutput,
        grammar: &PBGrammar,
    ) -> SolverResult {
        let status_self = self.parsed_status;
        let status_other = other.parsed_status;
        let status_str1 = status_self.get_status_string();
        let status_str2 = status_other.get_status_string();
        match (status_self, status_other) {
            (
                SolverStatus::Sat {
                    conflicts: c1,
                    solution: s1,
                },
                SolverStatus::Sat {
                    conflicts: c2,
                    solution: s2,
                },
            ) => {
                if let Err(e) = s1.is_valid(grammar) {
                    return Err(BugReason::Solution(InvalidSolution {
                        solver_name: self.name,
                        violation: e,
                    })
                    .into());
                }
                if let Err(e) = s2.is_valid(grammar) {
                    return Err(BugReason::Solution(InvalidSolution {
                        solver_name: other.name,
                        violation: e,
                    })
                    .into());
                }
                let conflicts = c1.max(c2);
                match (s1, s2) {
                    (SolverSolution::PBS(m1), SolverSolution::PBS(m2)) => Ok(SolverOutcome::Sat {
                        conflicts,
                        solution: SatModel::Comparison(m1, m2),
                        optimum: None,
                    }),
                    (SolverSolution::PBO(m1, min1), SolverSolution::PBO(m2, min2)) => {
                        if min1 == min2 {
                            Ok(SolverOutcome::Sat {
                                conflicts,
                                solution: SatModel::Comparison(m1, m2),
                                optimum: Some(min1),
                            })
                        } else {
                            let i1 = ObjectiveInfo {
                                solver_name: self.name,
                                min_objective: min1,
                            };
                            let i2 = ObjectiveInfo {
                                solver_name: other.name,
                                min_objective: min2,
                            };
                            Err(BugReason::DifferentObjectives(i1, i2).into())
                        }
                    }
                    _ => unreachable!("cant compare pbs with pbo"),
                }
            }
            (SolverStatus::Unsat { conflicts: c1 }, SolverStatus::Unsat { conflicts: c2 }) => {
                Ok(SolverOutcome::Unsat {
                    conflicts: c1.max(c2),
                })
            }
            (
                SolverStatus::Unknown {
                    solver_name: n1,
                    status,
                },
                SolverStatus::Unknown {
                    solver_name: n2, ..
                },
            ) => Err(IterationError::Unknown {
                solver_name: format!("{n1} + {n2}"),
                status,
            }),
            (SolverStatus::TimeLimit, _) => Ok(SolverOutcome::TimeLimit),
            (_, SolverStatus::TimeLimit) => Ok(SolverOutcome::TimeLimit),
            _ => Err(BugReason::OutputStatusDiffers(status_str1.into(), status_str2.into()).into()),
        }
    }
    pub fn verify_with_proof_checking(
        self,
        grammar: &PBGrammar,
        formula: &Path,
        derivation: &Path,
        solver_name: &str,
    ) -> SolverResult {
        let status = self.parsed_status;
        match status {
            SolverStatus::Sat {
                conflicts,
                solution,
            } => {
                if let Err(e) = solution.is_valid(grammar) {
                    return Err(BugReason::Solution(InvalidSolution {
                        solver_name: solver_name.into(),
                        violation: e,
                    })
                    .into());
                }
                if let Err(e) = check_proof(formula, derivation) {
                    return Err(BugReason::ProofCheckerFailed(e).into());
                }
                match solution {
                    SolverSolution::PBS(solution_model) => Ok(SolverOutcome::Sat {
                        conflicts,
                        solution: SatModel::Proof(solution_model),
                        optimum: None,
                    }),
                    SolverSolution::PBO(solution_model, min_objective_value) => {
                        Ok(SolverOutcome::Sat {
                            conflicts,
                            solution: SatModel::Proof(solution_model),
                            optimum: Some(min_objective_value),
                        })
                    }
                }
            }
            SolverStatus::Unsat { conflicts } => {
                if let Err(e) = check_proof(formula, derivation) {
                    return Err(BugReason::ProofCheckerFailed(e).into());
                }
                Ok(SolverOutcome::Unsat { conflicts })
            }
            SolverStatus::TimeLimit => Ok(SolverOutcome::TimeLimit),
            SolverStatus::Unknown {
                solver_name,
                status,
            } => Err(IterationError::Unknown {
                solver_name,
                status,
            }),
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
    Unknown {
        solver_name: String,
        status: String,
    },
}

impl SolverStatus {
    fn get_status_string(&self) -> &'static str {
        match self {
            SolverStatus::Sat { .. } => "SAT",
            SolverStatus::Unsat { .. } => "UNSAT",
            SolverStatus::TimeLimit => "Timeout",
            SolverStatus::Unknown { .. } => "Unknown",
        }
    }
}

pub enum SatModel {
    Comparison(SolutionModel, SolutionModel),
    Proof(SolutionModel),
}

pub trait IterationResult {
    fn get_folder_name(&self) -> &'static str;
    fn get_status_as_string(&self) -> &'static str;
    fn get_file_name_prefix(&self) -> &'static str {
        self.get_status_as_string()
    }
}

#[derive(Debug, Error)]
pub enum IterationError {
    #[error(transparent)]
    Bug(#[from] BugReason),
    #[error(transparent)]
    Error(#[from] ExternalProgramError),
    #[error("{solver_name} - Unknown status: {status}")]
    Unknown { solver_name: String, status: String },
}

impl IterationResult for IterationError {
    fn get_folder_name(&self) -> &'static str {
        match self {
            IterationError::Bug(_) => BUG_FOLDER.name,
            IterationError::Error(_) => ERROR_FOLDER.name,
            IterationError::Unknown { .. } => UNKNOWN_FOLDER.name,
        }
    }

    fn get_status_as_string(&self) -> &'static str {
        match self {
            IterationError::Bug(_) => "bug",
            IterationError::Error(_) => "error",
            IterationError::Unknown { .. } => "unknown",
        }
    }
}

/// Models the different states of one iteration in the orchestration pipeline
pub enum SolverOutcome {
    /// result SAT containts data about the solution and in the Optimization case the found minimum value
    Sat {
        conflicts: u32,
        solution: SatModel,
        optimum: Option<MinObjectiveValue>,
    },
    Unsat {
        conflicts: u32,
    },
    TimeLimit,
}

impl IterationResult for SolverResult {
    fn get_folder_name(&self) -> &'static str {
        match self {
            Ok(ok) => ok.get_folder_name(),
            Err(err) => err.get_folder_name(),
        }
    }

    fn get_status_as_string(&self) -> &'static str {
        match self {
            Ok(ok) => ok.get_status_as_string(),
            Err(err) => err.get_status_as_string(),
        }
    }
}

impl IterationResult for SolverOutcome {
    fn get_folder_name(&self) -> &'static str {
        match self {
            SolverOutcome::Sat { .. } => SAT_FOLDER.name,
            SolverOutcome::Unsat { .. } => UNSAT_FOLDER.name,
            SolverOutcome::TimeLimit => TIMEOUT_FOLDER.name,
        }
    }

    fn get_status_as_string(&self) -> &'static str {
        match self {
            SolverOutcome::Sat { .. } => "sat",
            SolverOutcome::Unsat { .. } => "unsat",
            SolverOutcome::TimeLimit => "timeout",
        }
    }
}
impl SolverOutcome {
    pub fn get_conflicts(&self) -> u32 {
        match self {
            SolverOutcome::Sat { conflicts, .. } => *conflicts,
            SolverOutcome::Unsat { conflicts } => *conflicts,
            SolverOutcome::TimeLimit => 0,
        }
    }
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
    #[error("Failed to parse output of solver: {0}")]
    Parsing(#[from] OutputParsingError),
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
