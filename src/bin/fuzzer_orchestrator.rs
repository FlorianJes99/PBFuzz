use std::{error::Error, fs, path::Path};

use clap::Parser;
use fuzzer_pseudo_bool::{
    fuzzer::{app_info::MetaData, helpers::current_time_as_timestamp},
    orchestrator::{orchestrator_args::OrchestratorArgs, runner::run_orchestrator},
};
use log::Level;

fn main() -> Result<(), Box<dyn Error>> {
    let program_version = env!("CARGO_PKG_VERSION");
    let commit_hash = env!("GIT_HASH");
    println!("Program version: {program_version}");
    println!("Program commit: {commit_hash}");
    let args = OrchestratorArgs::parse();
    let start_as_str = current_time_as_timestamp();

    init_logger(args.log, &start_as_str)?;
    let meta_data = MetaData {
        program_version: program_version.into(),
        commit_hash: commit_hash.into(),
    };
    run_orchestrator(&start_as_str, meta_data, args)
}

fn init_logger(level: Option<Level>, timestamp: &str) -> Result<(), fern::InitError> {
    if !Path::new("logs").exists() {
        fs::create_dir("logs")?;
    }
    fern::Dispatch::new()
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
        .chain(fern::log_file(format!("logs/{}_output.log", timestamp))?)
        .apply()?;
    Ok(())
}
