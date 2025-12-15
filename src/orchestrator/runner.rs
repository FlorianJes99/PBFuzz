use std::{
    fmt::Display,
    fs::File,
    io::BufWriter,
    path::PathBuf,
    process::{Command, Output},
};

use rand;

use crate::grammar::pbs_generator::{GeneratorSettingsBuilder, PBSGenerator};

pub fn run_orchestrator(mut input: OrchestratorInput) {
    let output_file = input.fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {:?}", &output_file);

    // add file to solve to programm command
    input.command1.set_input_file(&output_file);
    input.command2.set_input_file(&output_file);

    let mut stats = Stats::default();
    for counter in 1..=input.iterations {
        if counter == 1000 && stats.is_sat_percentage_below(0.25) {
            // terminate early if amount sat instances below 25%
            // and more than 1000 iterations are scheduled
            println!("Terminating early!");
            println!("Percentage SAT Instances below 25%");
            break;
        }

        let seed = rand::random();
        let settings = GeneratorSettingsBuilder::new(seed, 50)
            .max_unsat_for_single_constraint(3)
            .max_unsat_assignments(170)
            .coeff_range(5)
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

    // print out stats
    println!("{stats}")
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

pub struct OrchestratorInput {
    iterations: u32,
    fuzzer_output_dir: PathBuf,
    command1: SolverCommand,
    command2: SolverCommand,
}

impl OrchestratorInput {
    pub fn new(
        iterations: u32,
        fuzzer_output_dir: PathBuf,
        command1: SolverCommand,
        command2: SolverCommand,
    ) -> Self {
        Self {
            iterations,
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

    fn is_sat_percentage_below(&self, threshold: f32) -> bool {
        self.percentage_sat() < threshold
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "===============Statistics===============")?;
        writeln!(
            f,
            "Total amount of instances generated: {}",
            self.amount_runs()
        )?;
        writeln!(f, "Generated amount of SAT instances: {}", self.amount_sat)?;
        writeln!(
            f,
            "Generated amount of UNSAT instances: {}",
            self.amount_unsat
        )?;
        writeln!(
            f,
            "Generated amount of unknown instances: {}",
            self.amount_unknown
        )?;
        writeln!(f, "Percentage SAT: {}", self.percentage_sat())?;
        writeln!(f, "========================================")
    }
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

#[derive(Debug, PartialEq, Eq)]
enum SolverStatus {
    Sat,
    Unsat,
    Unknown,
}
