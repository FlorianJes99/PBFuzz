use std::{path::PathBuf, process::Command};

use clap::Parser;
use fuzzer_pseudo_bool::orchestrator::runner::{
    OrchestratorInput, SolverCommand, run_orchestrator,
};

fn main() {
    let args = OrchestratorArgs::parse();
    let command1 = SolverCommand::new("roundingsat".to_string(), Command::new("roundingsat"));
    let command2 = SolverCommand::new("naps".to_string(), Command::new("software/naps"));
    run_orchestrator(OrchestratorInput::new(
        args.iterations,
        args.ouput_dir,
        command1,
        command2,
    ));
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct OrchestratorArgs {
    #[arg(short, long)]
    ouput_dir: PathBuf,
    #[arg(short, long, default_value_t = 1000)]
    iterations: u32,
}
