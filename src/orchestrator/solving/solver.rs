use std::{
    fmt::Display,
    io,
    ops::Not,
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
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
    proof_enabled: bool,
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
        let (state, stdout, stderr, signal) = match rx.recv_timeout(timeout) {
            Ok((stdout, stderr)) => {
                let status = child.wait()?;
                let signal = status.signal();
                (SolverState::Terminated, stdout, stderr, signal)
            }
            Err(e) => {
                warn!("Terminating solver {}... - {}", self.get_name(), e);
                child.kill()?;
                let _ = child.wait(); // reap
                (SolverState::Timeout, vec![], vec![], None)
            }
        };
        let record = SolverExecutionRecord {
            name,
            binary: cmd.get_program().display().to_string(),
            used_args: cmd.get_args().map(|a| a.display().to_string()).collect(),
            execution_time: start.elapsed(),
            signal,
        };
        let solver_output = SolverOutput { stdout, stderr };
        Ok(SolverRun {
            record,
            solver_output,
            state,
        })
    }
}

impl SolverData {
    pub fn into_solver(config: SolverConfig, timelimit: Duration, proof_enabled: bool) -> Self {
        let proof = config.proof_arg.map(ProofArg::new);
        Self {
            binary: config.binary,
            args: config.args,
            optional_args: config.optional_args,
            name: config.name,
            timelimit,
            proof,
            proof_enabled,
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

        if self.proof_enabled
            && let Some(proof_arg) = &self.proof
        {
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
        self.proof_enabled && self.proof.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SolverExecutionRecord {
    pub name: String,
    pub binary: String,
    pub used_args: Vec<String>,
    pub execution_time: Duration,
    pub signal: Option<i32>,
}

#[derive(Debug)]
pub struct SolverOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl SolverOutput {
    pub fn to_combined(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.stdout.len() + self.stderr.len() + 32);
        buf.extend_from_slice(b"stdout:\n");
        buf.extend_from_slice(&self.stdout);
        if !self.stdout.ends_with(b"\n") {
            buf.push(b'\n');
        }
        buf.extend_from_slice(b"\nstderr:\n");
        buf.extend_from_slice(&self.stderr);
        if !self.stderr.ends_with(b"\n") {
            buf.push(b'\n');
        }
        buf
    }
}

impl Display for SolverExecutionRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let SolverExecutionRecord {
            name,
            binary,
            used_args,
            execution_time,
            signal,
        } = &self;
        writeln!(f, "Solver {}: {}", name, binary)?;
        writeln!(f, "Arguments: {}", used_args.join(" "))?;
        writeln!(
            f,
            "Execution Time: {}s {}ms",
            execution_time.as_secs(),
            execution_time.as_millis()
        )?;
        if let Some(s) = signal {
            writeln!(f, "Termination signal: {}", s)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SolverRun {
    pub record: SolverExecutionRecord,
    pub solver_output: SolverOutput,
    pub state: SolverState,
}

#[derive(Debug)]
pub enum SolverState {
    Terminated,
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
