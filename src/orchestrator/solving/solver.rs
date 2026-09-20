use std::{
    fmt::Display,
    io,
    ops::Not,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread::{self},
    time::{Duration, Instant},
};

use log::{error, warn};
use serde::Serialize;
use thiserror::Error;

use crate::orchestrator::solving::solver_config::{OptionalArgs, SolverConfig};

enum ProofArg {
    Joined(String),
    Separate(String),
}

impl Display for ProofArg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ProofArg::Joined(s) => s,
                ProofArg::Separate(s) => s,
            }
        )
    }
}

impl ProofArg {
    pub fn new(arg: String) -> Self {
        // TODO: ugly hack, look for better solution
        if arg.ends_with("=") {
            ProofArg::Joined(arg)
        } else {
            ProofArg::Separate(arg)
        }
    }

    fn apply(&self, command: &mut Command, proof_file: &Path) {
        match self {
            ProofArg::Joined(prefix) => {
                command.arg(format!("{prefix}{}", proof_file.display()));
            }
            ProofArg::Separate(flag) => {
                command.args([flag.as_str(), &proof_file.display().to_string()]);
            }
        }
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
    /// terminate running process of the solver after this duration
    timelimit: Duration,
    /// wheter solver supports generation of proofs or not
    proof: Option<ProofArg>,
}

/// Shared behavior for all Solvers, no matter if they supports proofs or not
impl SolverData {
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
        let start = Instant::now();
        let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                error!(
                    "Given binary was not found: {}",
                    cmd.get_program().display()
                );
                // if the binary is not found, panic and stop the entire run
                panic!("Invalid binary: {}", cmd.get_program().display());
            }
            Err(e) => return Err(e),
        };
        let mut stdout = child.stdout.take().expect("stdout was piped");
        let mut stderr = child.stderr.take().expect("stderr was piped");
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf_stdout = Vec::new();
            let mut buf_stderr = Vec::new();
            let _ = std::io::Read::read_to_end(&mut stdout, &mut buf_stdout);
            let _ = std::io::Read::read_to_end(&mut stderr, &mut buf_stderr);
            let _ = tx.send((buf_stdout, buf_stderr));
        });

        let name = self.name.clone();
        let (state, stderr) = match rx.recv_timeout(timeout) {
            Ok((stdout, stderr)) => {
                let status = child.wait()?;
                (
                    SolverState::Terminated(Output {
                        status,
                        stdout,
                        stderr: vec![],
                    }),
                    stderr,
                )
            }
            Err(e) => {
                warn!("Terminating solver {}... - {}", self.get_name(), e);
                child.kill()?;
                let _ = child.wait(); // reap
                (SolverState::Timeout, vec![])
            }
        };
        let record = SolverExecutionRecord {
            name,
            binary: cmd.get_program().display().to_string(),
            used_args: cmd.get_args().map(|a| a.display().to_string()).collect(),
            execution_time: start.elapsed(),
            stderr,
        };
        Ok(SolverRun { record, state })
    }
}

impl SolverData {
    pub fn into_solver(config: SolverConfig, timelimit: Duration) -> Self {
        let proof = config.proof_arg.map(ProofArg::new);
        Self {
            binary: config.binary,
            args: config.args,
            optional_args: config.optional_args,
            name: config.name,
            timelimit,
            proof,
        }
    }
    pub fn run(
        &self,
        file_path: &Path,
        proof_file: &Path,
        with_option_fuzzing: bool,
    ) -> Result<SolverRun, SolverExecutionFailed> {
        let mut command = Command::new(self.binary.as_os_str());
        for arg in &self.args {
            command.arg(arg);
        }

        if let Some(proof_arg) = &self.proof {
            proof_arg.apply(&mut command, proof_file);
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

    pub fn supports_proofs(&self) -> bool {
        self.proof.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverExecutionRecord {
    pub name: String,
    pub binary: String,
    pub used_args: Vec<String>,
    pub execution_time: Duration,
    pub stderr: Vec<u8>,
}

impl Display for SolverExecutionRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let SolverExecutionRecord {
            name,
            binary,
            used_args,
            execution_time,
            stderr,
        } = &self;
        writeln!(f, "Solver {}: {}", name, binary)?;
        writeln!(f, "Arguments: {}", used_args.join(" "))?;
        writeln!(
            f,
            "Execution Time: {}s {}ms",
            execution_time.as_secs(),
            execution_time.as_millis()
        )?;
        if !stderr.is_empty() {
            write!(f, "\nStderr:")?;
            for line in String::from_utf8_lossy(stderr).lines() {
                write!(f, "\n {line}")?;
            }
        }
        Ok(())
    }
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
#[error("{program}: failed to execute: {source}")]
pub struct SolverExecutionFailed {
    program: String,
    #[source]
    source: std::io::Error,
}
