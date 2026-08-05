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

use crate::orchestrator::solving::solver_config::{OptionalArgs, SolverConfig};

/// Marker for Solvers, which do not support proofs
pub struct NoProof;

/// Marker for Solvers, which can generate proofs
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

/// Shared behavior for all Solvers, no matter if they supports proofs or not
impl<P> SolverData<P> {
    /// every solver needs to have a name, which works as an identifier
    pub fn get_name(&self) -> &str {
        &self.name
    }

    fn external_execution_error(&self, source: std::io::Error) -> SolverExecutionFailed {
        SolverExecutionFailed {
            program: self.get_name().into(),
            source,
        }
    }

    /// starts the solver in a child process and terminates this process after the given output
    fn run_with_timeout(&self, mut cmd: Command, timeout: Duration) -> io::Result<SolverRun> {
        let mut child = cmd.stdout(Stdio::piped()).spawn()?;
        let mut stdout = child.stdout.take().expect("stdout was piped");
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stdout, &mut buf);
            let _ = tx.send(buf);
        });

        let name = self.name.clone();
        let used_command = format!(
            "{} {}",
            cmd.get_program().display(),
            cmd.get_args().map(|a| a.display()).join(" ")
        );
        let record = SolverExecutionRecord { name, used_command };
        let state = match rx.recv_timeout(timeout) {
            Ok(stdout_buf) => {
                let status = child.wait()?;
                SolverState::Terminated(Output {
                    status,
                    stdout: stdout_buf,
                    stderr: vec![],
                })
            }
            Err(_) => {
                info!("Terminating solver {}...", self.get_name());
                child.kill()?;
                let _ = child.wait(); // reap
                SolverState::Timeout
            }
        };
        Ok(SolverRun { record, state })
    }
}

impl SolverData<WithProof> {
    pub fn into_proof_solver(
        config: SolverConfig,
        timelimit: Duration,
    ) -> Result<Self, MissingProofLoggingArgument> {
        config
            .proof_arg
            .ok_or(MissingProofLoggingArgument {
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
    ) -> Result<SolverRun, SolverExecutionFailed> {
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
    ) -> Result<SolverRun, SolverExecutionFailed> {
        let mut command = Command::new(self.binary.as_os_str());
        for arg in &self.args {
            command.arg(arg);
        }

        if with_option_fuzzing && !self.optional_args.is_empty() {
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

#[derive(Debug, Clone)]
pub struct SolverExecutionRecord {
    pub name: String,
    pub used_command: String,
}

#[derive(Debug)]
pub struct SolverRun {
    pub record: SolverExecutionRecord,
    pub state: SolverState,
}

#[derive(Debug)]
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

#[derive(Debug, Error)]
#[error("{program}: missing required proof logging argument")]
pub struct MissingProofLoggingArgument {
    program: String,
}

#[derive(Debug, Error)]
#[error("{program}: failed to execute: {source}")]
pub struct SolverExecutionFailed {
    program: String,
    #[source]
    source: std::io::Error,
}
