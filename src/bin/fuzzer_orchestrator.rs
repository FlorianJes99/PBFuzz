use std::{path::PathBuf, process::Command};

use clap::Parser;
use fuzzer_pseudo_bool::orchestrator::runner::{
    OrchestratorInput, OutputParser, SolverCommand, run_orchestrator,
};
use regex::Regex;

fn main() {
    let args = OrchestratorArgs::parse();
    let parser1 = OutputParser::new(
        Regex::new(r"^s\s+SATISFIABLE$").unwrap(),
        Regex::new(r"^s\s+UNSATISFIABLE$").unwrap(),
    );
    let command1 = SolverCommand::new(
        "roundingsat".to_string(),
        Command::new("roundingsat"),
        parser1,
    );
    let parser2 = OutputParser::new(
        Regex::new(r"\[optimal solution found\]").unwrap(),
        Regex::new(r"\[infeasible\]").unwrap(),
    );
    let mut program2 = Command::new("software/scip");
    program2.arg("-f");
    let command2 = SolverCommand::new("scip".to_string(), program2, parser2);
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
