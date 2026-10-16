use std::{
    fs::create_dir,
    io::{self, Error, ErrorKind},
    path::{Path, PathBuf},
};

use clap::Parser;
use fuzzer_pseudo_bool::{
    fuzzer::{app_info::MetaData, helpers::current_time_as_timestamp},
    orchestrator::{orchestrator_args::OrchestratorArgs, runner::run_orchestrator},
};
use indicatif::MultiProgress;
use indicatif_log_bridge::LogWrapper;
use log::Level;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let program_version = env!("CARGO_PKG_VERSION");
    let commit_hash = option_env!("GIT_HASH");
    let args = OrchestratorArgs::parse();
    let start_as_str = current_time_as_timestamp();
    let run_root = prepare_run_root(&args.output_dir, &start_as_str)?;

    let mp = init_logger(args.log, &run_root)?;
    let meta_data = MetaData {
        program_version: program_version.into(),
        commit_hash: commit_hash.map(Into::into),
    };
    run_orchestrator(meta_data, args, run_root, mp)
}

fn prepare_run_root(output_path: &Path, start_as_str: &str) -> io::Result<PathBuf> {
    if output_path.exists() && output_path.is_file() {
        return Err(Error::new(
            ErrorKind::NotADirectory,
            format!(
                "Current workspace needs to be a directory, but was: {}",
                output_path.display()
            ),
        ));
    }
    if !output_path.exists() {
        create_dir(output_path)?;
    }
    let folder_fuzzing_run = output_path.join(format!("run_{}", start_as_str));
    create_dir(&folder_fuzzing_run)?;
    Ok(folder_fuzzing_run)
}

fn init_logger(level: Option<Level>, run_root: &Path) -> Result<MultiProgress, fern::InitError> {
    let log_file = run_root.join("run.log");
    let (max_level, logger) = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                current_time_as_timestamp(),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(level.unwrap_or(Level::Error).to_level_filter())
        .chain(std::io::stdout())
        .chain(fern::log_file(log_file)?)
        .into_log();
    let mp = MultiProgress::new();
    LogWrapper::new(mp.clone(), logger).try_init()?;
    log::set_max_level(max_level);
    Ok(mp)
}
