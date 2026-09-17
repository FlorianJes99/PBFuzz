use clap::Parser;
use serde::Serialize;
use std::path::PathBuf;

/// Arguments that can be passed to the orchestration pipeline for testing multiple solvers against
/// their outputs.
/// IMPORTANT: if both settings file and parameters are provided, then parameters will override
/// settings specified in the file
#[derive(Debug, Parser, Serialize)]
#[command(version, about, long_about = None)]
pub struct OrchestratorArgs {
    /// Set logging level (debug, info, warn, error). Default is 'error'
    ///
    /// Logging currently only supports printing to standard out.
    #[arg(short, long)]
    pub log: Option<log::Level>,
    /// Specify the base directory for the orchestrator (default: `out/`).
    ///
    /// Inside the base directory the runner creates a directory with the current timestamp.
    /// All data gets written into this subdirectory.
    /// BE CAREFUL to either specify a path relative to the execution of the binary or specify a
    /// correct absolute path for the base directory.
    #[arg(short, long, default_value = "out/")]
    pub output_dir: PathBuf,
    /// Terminate orchestrator after n iterations. Conflicts with the timeout_min flag.
    ///
    #[arg(short, long, conflicts_with = "timeout_min")]
    pub(crate) iterations: Option<u32>,
    /// Terminate orchestrator after n elapsed minutes. Conflicts with the iterations flag.
    #[arg(short, long, conflicts_with = "iterations")]
    pub(crate) timeout_min: Option<u32>,
    /// Use a settings file for the fuzzer.
    ///
    /// The expected file should be in the [toml-format](https://docs.rs/toml/latest/toml/).
    /// This file allows for more granular control about the settings.
    /// Not only `PBMode` can be set here, but also relevant meta parameters for the fuzzer.
    /// Those include things like maximum generated variables per instance and more.
    /// After a successful run the used settings are stored in the current working space and can be
    /// used for the next run.
    #[arg(long)]
    pub(crate) settings_file: PathBuf,

    /// Specify the config file in toml format for all solvers used in this run.
    /// The first specified solver works as the ground truth when used in comparison to others.
    #[arg(long, num_args=1..)]
    pub(crate) solver_configs: Vec<PathBuf>,

    /// Set to true if `valid` results should be saved as well (default: false)
    ///
    /// A valid result is considered to be SAT or UNSAT
    #[arg(long, default_value_t = false)]
    pub(crate) save_all_files: bool,

    /// Provide number of threads used to run parallel fuzzing runs
    ///
    /// If nothing is provided, the program will use only 1 thread
    #[arg(long)]
    pub(crate) thread_count: Option<u8>,

    /// Use this flag to terminate an external solver after n seconds
    ///
    /// This is only ment to be used, if the solver itself does not provide any
    /// flags to stop computation after some amount of time
    #[arg(long, default_value_t = u64::MAX)]
    pub(crate) timeout_external_program_secs: u64,
}
