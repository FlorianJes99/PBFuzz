use std::fs::File;
use std::io::BufWriter;

use fuzzer_pseudo_bool::grammar::pbs_generator::GeneratorSettingsBuilder;
use fuzzer_pseudo_bool::grammar::pbs_generator::PBSGenerator;
use log::error;
use log::info;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct FuzzerArgs {
    #[arg(long)]
    pub seed: u64,
    #[arg(long)]
    pub output_path: String,
    // Optional arguments
    #[arg(
        long,
        default_value_t = 10,
        value_parser = clap::value_parser!(u32).range(1..),
        help = "Number of constraints used (must be > 0)"
    )]
    pub amount_vars: u32,
    #[arg(long, default_value_t = false)]
    pub print_statistics: bool,
}

fn main() {
    // initialize the logger
    env_logger::init();

    // start of actual program
    let args = FuzzerArgs::parse();

    info!(
        "Running fuzzer with seed {} and {} variables.",
        args.seed, args.amount_vars
    );
    let settings = GeneratorSettingsBuilder::new(args.amount_vars)
        .max_unsat_for_single_constraint(3)
        .set_bounds(0.3, 0.6)
        .coeff_range(6)
        .max_unsat_assignments(250)
        .build();
    let mut grammar = PBSGenerator::create_grammar(args.seed, settings);
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
