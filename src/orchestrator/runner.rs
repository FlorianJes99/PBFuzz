use std::{
    fmt::Display,
    fs::File,
    io::BufWriter,
    path::PathBuf,
    process::{Command, Output},
};

use rand;

use crate::grammar::pbs_generator::{GeneratorSettingsBuilder, PBSGenerator};

pub struct OrchestratorInput {
    fuzzer_output_dir: PathBuf,
    command1: SolverCommand,
    command2: SolverCommand,
}

impl OrchestratorInput {
    pub fn new(
        fuzzer_output_dir: PathBuf,
        command1: SolverCommand,
        command2: SolverCommand,
    ) -> Self {
        Self {
            fuzzer_output_dir,
            command1,
            command2,
        }
    }
}

pub struct SolverCommand {
    exec_name: String,
    exec_command: Command,
}

impl SolverCommand {
    pub fn new(exec_name: String, exec_command: Command) -> Self {
        Self {
            exec_name,
            exec_command,
        }
    }

    fn set_input_file(&mut self, output_file: &PathBuf) {
        self.exec_command.arg(output_file);
    }
}
pub fn run_orchestrator(mut input: OrchestratorInput) {
    let output_file = input.fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {:?}", &output_file);

    // add file to solve to programm command
    input.command1.set_input_file(&output_file);
    input.command2.set_input_file(&output_file);

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

        let res_program1 = run_external_programm(&mut input.command1);
        let res_program2 = run_external_programm(&mut input.command2);
        let status_program1 = match res_program1 {
            Ok(val) => val,
            Err(e) => {
                println!("{}", e);
                break;
            }
        };
        let status_program2 = match res_program2 {
            Ok(val) => val,
            Err(e) => {
                println!("{}", e);
                break;
            }
        };
        if status_program2 != status_program1 {
            println!("Status sat4j: {:?}", status_program2);
            println!("Status roundingsat: {:?}", status_program1);
            panic!("Test panick for now")
        }
        let status = status_program1;
        let file_path = match &status {
            SolverStatus::Sat => input
                .fuzzer_output_dir
                .join(format!("sat/sat_{}_{}.opb", seed, counter)),
            SolverStatus::Unsat => input
                .fuzzer_output_dir
                .join(format!("unsat/unsat_{}_{}.opb", seed, counter)),
            SolverStatus::Unknown => input
                .fuzzer_output_dir
                .join(format!("unknown_{}.opb", seed)),
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

struct ProgramExecutionFailed {
    program_name: String,
    error_message: String,
}

impl Display for ProgramExecutionFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Program {} failed.", self.program_name)?;
        writeln!(f, "{}", self.error_message)
    }
}

fn run_external_programm(
    command: &mut SolverCommand,
) -> Result<SolverStatus, ProgramExecutionFailed> {
    command
        .exec_command
        .output()
        .map(handle_command_output)
        .map_err(|err| ProgramExecutionFailed {
            program_name: command.exec_name.clone(),
            error_message: err.to_string(),
        })
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
