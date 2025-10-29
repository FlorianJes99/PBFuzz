use clap::Parser;
use log::error;
use log::info;

use crate::grammar::PBSGenerator;

mod fuzzer;
mod fuzzer_args;
mod grammar;

fn main() {
    // initialize the logger
    env_logger::init();

    // start of actual program
    let args = fuzzer_args::FuzzerArgs::parse();

    info!(
        "Running fuzzer with seed {}, {} variables and {} constraints.",
        args.seed, args.amount_vars, args.amount_constraints
    );
    let grammar = PBSGenerator::new(args.seed, args.amount_vars, args.amount_constraints);
    let mut fuzzer = fuzzer::Fuzzer::new(grammar);
    let output_path = &args.output_path;
    let write_result = fuzzer.write_to_file(output_path);
    match write_result {
        Ok(_) => info!("Wrote generated grammar into file: {}", output_path),
        Err(_) => error!("Could not write into file: {}", output_path),
    }
}
