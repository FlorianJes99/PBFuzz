use std::path::PathBuf;

use clap::Parser;
use fuzzer_pseudo_bool::orchestrator::runner::run_orchestrator;

fn main() {
    let args = OrchestratorArgs::parse();
    run_orchestrator(args.ouput_dir);
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct OrchestratorArgs {
    #[arg(short, long)]
    ouput_dir: PathBuf,
}
