use std::{path::PathBuf};

use clap::Parser;
use fuzzer_pseudo_bool::orchestrator::{
    roundingsat::Roundingsat,
    runner::{OrchestratorInput, run_orchestrator},
    scip::Scip,
};

fn main() {
    let args = OrchestratorArgs::parse();
    let rounding_sat = Roundingsat::new("roundingsat".into());
    let scip = Scip::new("software/scip".into());
    let res = run_orchestrator(OrchestratorInput::new(
        args.iterations,
        args.ouput_dir,
        args.settings_file,
        rounding_sat,
        scip,
    ));
    if res.is_err() {
        println!("{}", res.unwrap_err())
    }
}

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct OrchestratorArgs {
    #[arg(short, long)]
    ouput_dir: PathBuf,
    #[arg(short, long, default_value_t = 1000)]
    iterations: u32,
    #[arg(long)]
    settings_file: Option<PathBuf>,
}
