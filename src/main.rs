use clap::Parser;

use crate::grammar::PBSGenerator;

mod fuzzer;
mod fuzzer_args;
mod grammar;

fn main() {
    let args = fuzzer_args::FuzzerArgs::parse();
    let grammar = PBSGenerator::new(args.seed, args.amount_vars, args.amount_constraints);
    let mut fuzzer = fuzzer::Fuzzer::new(grammar);
    let output_path = &args.output_path;
    fuzzer
        .write_to_file(output_path)
        .expect(format!("Content could not be written to {}", output_path).as_str());
}
