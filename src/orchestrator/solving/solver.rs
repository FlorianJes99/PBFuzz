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
        grammar::MinObjectiveInvalid,
    },
    orchestrator::{
        parsing::parser::OutputParsingError,
        solving::solver_config::{OptionalArgs, SolverConfig},
    },
};

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

    /// starts the solver in a child process and terminates this process after the given output
    ///
    /// Because we use libc crate to kill the child process, this method makes use of `unsafe` code
    fn run_with_timeout(&self, mut cmd: Command, timeout: Duration) -> io::Result<SolverRun> {
        let child = cmd.stdout(Stdio::piped()).spawn()?;
        let (tx, rx) = mpsc::channel();
        let pid = child.id();

        thread::spawn(move || {
            let _ = tx.send(child.wait_with_output());
        });

        let name = self.name.clone();
        let used_command = format!(
            "{} {}",
            cmd.get_program().display(),
            cmd.get_args().map(|a| a.display()).join(" ")
        );
        let record = SolverExecutionRecord { name, used_command };
        let state = match rx.recv_timeout(timeout) {
            Ok(result) => result.map(SolverState::Terminated),
            Err(_) => {
                info!("Terminating solver {}...", self.get_name());
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
                Ok(SolverState::Timeout)
            }
        }?;
        Ok(SolverRun { record, state })
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
                timelimit,
                proof: WithProof { proof_arg },
            })
    }

    pub fn run_with_proof(
        &self,
        file_path_pb: &Path,
        file_path_proof: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverRun, ExternalProgramError> {
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
            name: config.name,
            timelimit,
            proof: NoProof,
        }
    }
    pub fn run(
        &self,
        file_path: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverRun, ExternalProgramError> {
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

pub struct SolverExecutionRecord {
    pub name: String,
    pub used_command: String,
}
pub struct SolverRun {
    pub record: SolverExecutionRecord,
    pub state: SolverState,
}

pub enum SolverState {
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

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(transparent)]
    Constraint(#[from] ConstraintViolation),
    #[error(transparent)]
    Objective(#[from] MinObjectiveInvalid),
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

pub enum SatModel {
    Comparison(SolutionModel, SolutionModel),
    Proof(SolutionModel),
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
