use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use log::error;
use log::info;

use crate::grammar::pbs_generator::GeneratorSettingsBuilder;
use crate::grammar::pbs_generator::PBSGenerator;

mod fuzzer_args;
mod grammar {
    pub mod domain;
    pub mod pbs_generator;
}

fn main() {
    // initialize the logger
    env_logger::init();

    // start of actual program
    let args = fuzzer_args::FuzzerArgs::parse();

    info!(
        "Running fuzzer with seed {} and {} variables.",
        args.seed, args.amount_vars
    );
    let settings = GeneratorSettingsBuilder::new(args.seed, args.amount_vars).build();
    let mut grammar = PBSGenerator::create_grammar(settings);
    if args.print_statistics {
        println!("{}", grammar.get_statistics());
    }
    let output_path = &args.output_path;
    let write_result = File::create(output_path)
        .map(|file| BufWriter::new(file))
        .map(|mut buf_writer| grammar.produce(&mut buf_writer));
    match write_result {
        Ok(_) => info!("Wrote generated grammar into file: {}", output_path),
        Err(e) => error!("Could not write into file: {}\n Error: {}", output_path, e),
    }
}
