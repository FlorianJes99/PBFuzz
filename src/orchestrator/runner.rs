use std::{
    fs::File,
    io::BufWriter,
    path::PathBuf,
    process::{Command, Output},
};

use rand;

use crate::grammar::pbs_generator::{GeneratorSettingsBuilder, PBSGenerator};

pub fn run_orchestrator(fuzzer_output_dir: PathBuf) {
    let output_file = fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {:?}", &output_file);

    let mut stats = Stats::default();
    for counter in 1..=1000 {
        let seed = rand::random();
        let settings = GeneratorSettingsBuilder::new(seed, 50)
            .max_unsat_for_single_constraint(4)
            .max_unsat_assignments(150)
            .build();

        println!("========================================");
        println!("Iteration {counter}");
        println!("========================================");
        println!("");
        println!("Running Fuzzer with seed {seed}...");

        let mut grammar = PBSGenerator::create_grammar(settings);
        let write_result = File::create(&output_file)
            .map(|file| BufWriter::new(file))
            .map(|mut buf_writer| grammar.produce(&mut buf_writer));
        if write_result.is_err() {
            println!("Error while writing into output file");
            println!("Terminating orchestrator");
            break;
        }

        let output_roundingsat = Command::new("roundingsat")
            .arg(&output_file)
            .output()
            .map(handle_command_output);

        let status_roundingsat = match output_roundingsat {
            Ok(val) => val,
            Err(e) => {
                println!("Error while executing roundingsat. Terminating program...");
                println!("{e}");
                break;
            }
        };
        let output_sat4j = Command::new("java")
            .arg("-jar")
            .arg("software/sat4j-pb.jar")
            .arg(&output_file)
            .output()
            .map(handle_command_output);

        let status_sat4j = match output_sat4j {
            Ok(val) => val,
            Err(e) => {
                println!("Error while executing sat4j. Terminating program...");
                println!("{e}");
                break;
            }
        };
        if status_sat4j != status_roundingsat {
            println!("Status sat4j: {:?}", status_sat4j);
            println!("Status roundingsat: {:?}", status_roundingsat);
            panic!("Test panick for now")
        }
        let status = status_roundingsat;
        let file_path = match &status {
            SolverStatus::Sat => {
                fuzzer_output_dir.join(format!("sat/sat_{}_{}.opb", seed, counter))
            }
            SolverStatus::Unsat => {
                fuzzer_output_dir.join(format!("unsat/unsat_{}_{}.opb", seed, counter))
            }
            SolverStatus::Unknown => fuzzer_output_dir.join(format!("unknown_{}.opb", seed)),
        };

        let write_r = std::fs::rename(&output_file, &file_path);
        if write_r.is_err() {
            println!("Could not write to file. Terminating...");
            break;
        }
        stats.update(&status);
    }

    println!("===============Statistics===============");
    println!(
        "Total amount of instances generated: {}",
        stats.amount_runs()
    );
    println!("Generated amount of SAT instances: {}", stats.amount_sat);
    println!(
        "Generated amount of UNSAT instances: {}",
        stats.amount_unsat
    );
    println!(
        "Generated amount of unknown instances: {}",
        stats.amount_unknown
    );
    println!("Percentage SAT: {}", stats.percentage_sat());
    println!("========================================");
}

fn handle_command_output(output: Output) -> SolverStatus {
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("UNSATISFIABLE") {
        SolverStatus::Unsat
    } else if stdout.contains("SATISFIABLE") {
        SolverStatus::Sat
    } else {
        SolverStatus::Unknown
    }
}

#[derive(Default)]
struct Stats {
    amount_sat: u32,
    amount_unsat: u32,
    amount_unknown: u32,
}

impl Stats {
    fn update(&mut self, status: &SolverStatus) {
        match status {
            SolverStatus::Sat => self.amount_sat += 1,
            SolverStatus::Unsat => self.amount_unsat += 1,
            SolverStatus::Unknown => self.amount_unknown += 1,
        }
    }

    fn amount_runs(&self) -> u32 {
        self.amount_sat + self.amount_unsat + self.amount_unknown
    }

    fn percentage_sat(&self) -> f32 {
        self.amount_sat as f32 / self.amount_runs() as f32
    }
}

#[derive(Debug, PartialEq, Eq)]
enum SolverStatus {
    Sat,
    Unsat,
    Unknown,
}
