use std::{path::PathBuf, time::Duration};

use clap::Parser;
use fuzzer_pseudo_bool::{
    fuzzer::{grammar::PBKind, pbs_generator::GeneratorSettingsBuilder},
    orchestrator::{
        roundingsat::Roundingsat,
        runner::{StopCondition, run_orchestrator},
        scip::Scip,
    },
};
use log::Level;
use rayon::ThreadPoolBuilder;

fn main() {
    let args = OrchestratorArgs::parse();
    init_logger(args.log);
    if let Some(t) = args.thread_count {
        ThreadPoolBuilder::new()
            .num_threads(t)
            .build_global()
            .expect("Global rayon thread pool should only be initialized once!");
    }
    println!(
        "Using {} threads for computing fuzzing runs in parallel",
        rayon::current_num_threads()
    );
    let roundingsat_bin = args.roundingsat_bin.unwrap_or("roundingsat".into());
    let scip_bin = args.scip_bin.unwrap_or("scip".into());
    let rounding_sat = Roundingsat::new(roundingsat_bin);
    let scip = Scip::new(scip_bin);
    let mut settings = match args.settings_file {
        Some(file) => GeneratorSettingsBuilder::load_from_file(&file).unwrap_or_else(|e| {
            println!("Error occured while loading settings: {}", e);
            println!("Using default settings...");
            GeneratorSettingsBuilder::default()
        }),
        None => GeneratorSettingsBuilder::default(),
    };

    if let Some(mode) = args.mode {
        settings = settings.pb_kind(mode);
    }
    let cancel = StopCondition::new(
        args.iterations,
        args.timeout_min.map(|v| Duration::from_mins(v as u64)),
    );
    if cancel.is_err() {
        println!("{}", cancel.as_ref().unwrap_err());
    }
    let output_dir = args.output_dir.unwrap_or("out/".into());
    println!("Workspace set to: {}", &output_dir.display());
    let res = run_orchestrator(
        output_dir,
        settings.build(),
        rounding_sat,
        scip,
        args.save_all_files,
        cancel.expect("Error handled already"),
    );
    if res.is_err() {
        println!("{}", res.unwrap_err())
    }
}

fn init_logger(level: Option<Level>) {
    env_logger::Builder::new()
        .filter_level(level.unwrap_or(Level::Error).to_level_filter())
        .init();
}

/// Arguments that can be passed to the orchestration pipeline for testing multiple solvers against
/// there outputs.
/// IMPORTANT: if both settings file and parameters are provided, then parameters will override
/// settings specified in the file
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct OrchestratorArgs {
    /// Set logging level (debug, info, warn, error). Default is 'error'
    ///
    /// Logging currently only supports printing to standard out.
    #[arg(short, long)]
    log: Option<log::Level>,
    /// Specify the base directory for the orchestrator (default: `out/`).
    ///
    /// Inside the base directory the runner creates a directory with the current timestamp.
    /// All data gets written into this subdirectory.
    /// BE CAREFUL to either specify a path relative to the execution of the binary or specify a
    /// correct absolute path for the base directory.
    #[arg(short, long)]
    output_dir: Option<PathBuf>,
    /// Terminate orchestrator after n iterations. Conflicts with the timeout_min flag.
    ///
    #[arg(short, long, conflicts_with = "timeout_min")]
    iterations: Option<u32>,
    /// Terminate orchestrator after n elapsed minutes. Conflicts with the iterations flag.
    #[arg(short, long, conflicts_with = "iterations")]
    timeout_min: Option<u32>,
    /// Choose mode for Fuzzer (default: PBS)
    #[arg(long, value_enum)]
    mode: Option<PBKind>,
    /// Use a settings file for the fuzzer.
    ///
    /// The expected file should be in the [ron-format](https://docs.rs/ron/latest/ron/).
    /// This file allows for more granular control about the settings.
    /// Not only `PBMode` can be set here, but also relevant meta parameters for the fuzzer.
    /// Those include things like maximum generated variables per instance and more.
    /// After a successful run the used settings are stored in the current working space and can be
    /// used for the next run.
    #[arg(long)]
    settings_file: Option<PathBuf>,

    /// Specify custom path to the `roundingsat` binary
    ///
    /// If nothing is provided the program expects to find the binary in the $PATH.
    #[arg(long)]
    roundingsat_bin: Option<PathBuf>,

    /// Specify custom path to the `scip` binary
    ///
    /// If nothing is provided the program expects to find the binary in the $PATH.
    #[arg(long)]
    scip_bin: Option<PathBuf>,

    /// Set to true if `valid` results should be saved as well (default: false)
    ///
    /// A valid result is considered to be SAT or UNSAT
    #[arg(long, default_value_t = false)]
    save_all_files: bool,

    /// Provide number of threads used to run parallel fuzzing runs
    ///
    /// If nothing is provided, the program will use up all available threads
    /// See: [Rayon FAQ](https://github.com/rayon-rs/rayon/blob/main/FAQ.md)
    #[arg(long)]
    thread_count: Option<usize>,
}
