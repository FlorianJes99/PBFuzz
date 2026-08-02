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

use itertools::Itertools;
use log::info;
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
        parsing::parser::{OutputParsingError, Parser},
        solving::solver_config::{OptionalArgs, SolverConfig},
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
impl<P> SolverData<P> {
    /// every solver needs to have a name, which works as an identifier
    pub fn get_name(&self) -> &str {
        &self.name
    }

    fn external_execution_error(&self, source: std::io::Error) -> ExternalProgramError {
        ExternalProgramError::ExecutionFailed {
            program: self.get_name().into(),
            source,
        }
    }

    /// returns the specific parser needed in order to extract the output of the solver
    fn parser(&self) -> &dyn Parser {
        &*self.parser
    }

    /// Makes use of the underlying parser for the solver in order to extract the computed status of the problem
    pub fn parse(
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

pub struct NoProof;
pub struct WithProof {
    proof_arg: String,
}

/// This struct holds all the relevant data in order to execute a Solver
pub struct SolverData<P = NoProof> {
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
    /// wheter solver supports generation of proofs or not
    proof: P,
}

impl SolverData<WithProof> {
    pub fn into_proof_solver(
        config: SolverConfig,
        timelimit: Duration,
    ) -> Result<Self, ExternalProgramError> {
        config
            .proof_arg
            .ok_or(ExternalProgramError::MissingProofLoggingArgument {
                program: config.name.clone(),
            })
            .map(|proof_arg| Self {
                binary: config.binary,
                args: config.args,
                optional_args: config.optional_args,
                name: config.name.clone(),
                parser: config.parser_kind.create_parser(config.name),
                timelimit,
                proof: WithProof { proof_arg },
            })
    }

    pub fn run_with_proof(
        &self,
        file_path_pb: &Path,
        file_path_proof: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverState, ExternalProgramError> {
        let mut command = Command::new(self.binary.as_os_str());
        // TODO: remove later, when profiling analysis is done
        let _prof_file_name = file_path_pb
            .file_stem()
            .expect("PB File should exist")
            .display();
        // add_profiling_to_command(&mut command, &prof_file_name);

        command.arg(format!(
            "{}{}",
            self.proof.proof_arg,
            file_path_proof.display()
        ));

        for arg in &self.args {
            command.arg(arg);
        }

        if with_option_fuzzing && !self.optional_args.is_empty() {
            let rng = &mut rand::rng();
            let sampled_args = &self.optional_args.sample(3, rng);
            info!("Sampled args for this run: {}", sampled_args.join(", "));
            for arg in sampled_args {
                command.arg(arg);
            }
        }

        command.arg(file_path_pb);
        self.run_with_timeout(command, self.timelimit)
            .map_err(|e| self.external_execution_error(e))
    }
}

impl SolverData<NoProof> {
    pub fn into_solver(config: SolverConfig, timelimit: Duration) -> Self {
        Self {
            binary: config.binary,
            args: config.args,
            optional_args: config.optional_args,
            name: config.name.clone(),
            parser: config.parser_kind.create_parser(config.name),
            timelimit,
            proof: NoProof,
        }
    }
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
            let rng = &mut rand::rng();
            for arg in &self.optional_args.sample(3, rng) {
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
    #[error("Failed to parse output of solver: {0}")]
    Parsing(#[from] OutputParsingError),
}
