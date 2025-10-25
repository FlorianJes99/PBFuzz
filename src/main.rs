use crate::grammar::OPBGenerator;

mod fuzzer;
mod grammar;

fn main() {
    let grammar = OPBGenerator::new(42, 3, 10);
    let mut fuzzer = fuzzer::Fuzzer::new(grammar);
    let output_path = "out/test.opb";
    fuzzer
        .write_to_file(output_path)
        .expect(format!("Content could not be written to {}", output_path).as_str());
}
