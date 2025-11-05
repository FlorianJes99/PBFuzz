use std::fs::File;

use clap::Parser;
use log::error;
use log::info;

use crate::grammar::Grammar;
use crate::grammar::PBSGenerator;

mod fuzzer_args;
mod grammar;
mod domain;

fn main() {
    // initialize the logger
    env_logger::init();

    // start of actual program
    let args = fuzzer_args::FuzzerArgs::parse();

    info!(
        "Running fuzzer with seed {} and {} variables.",
        args.seed, args.amount_vars
    );
    let mut grammar = PBSGenerator::new(args.seed, args.amount_vars);
    let output_path = &args.output_path;
    let write_result = File::create(output_path).map(|mut file| grammar.produce(&mut file));
    match write_result {
        Ok(_) => info!("Wrote generated grammar into file: {}", output_path),
        Err(_) => error!("Could not write into file: {}", output_path),
    }
}
