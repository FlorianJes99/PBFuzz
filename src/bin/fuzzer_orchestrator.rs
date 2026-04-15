use std::{fs, path::Path};

use clap::Parser;
use fuzzer_pseudo_bool::{
    fuzzer::helpers::current_time_as_timestamp,
    orchestrator::{orchestrator_args::OrchestratorArgs, runner::run_orchestrator},
};
use log::{Level, error};

fn main() {
    let args = OrchestratorArgs::parse();
    let start_as_str = current_time_as_timestamp();
    if let Err(e) = init_logger(args.log, &start_as_str) {
        println!("Logger initalization failed\nError: {}", e);
        return;
    }
    let res = run_orchestrator(&start_as_str, args);
    if let Err(e) = res {
        error!("{}", e)
    }
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
