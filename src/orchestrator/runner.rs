use std::{
    fmt::Display,
    fs::File,
    io::{BufWriter, Write},
    ops::Range,
    path::PathBuf,
    process::{Command, Output},
    time::Instant,
};

use rand;
use regex::Regex;

use crate::grammar::{
    helpers::print_block,
    pbs_generator::{CancelCondition, GeneratorSettings, GeneratorSettingsBuilder, PBSGenerator},
};

pub fn run_orchestrator(mut input: OrchestratorInput) {
    let output_file = input.fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {:?}", &output_file);

    // Add file to solve to program command
    input.command1.set_input_file(&output_file);
    input.command2.set_input_file(&output_file);

    let mut stats = Stats::new();

    let cancel_condition = CancelCondition::CountConstraints {
        max_constraints: 250,
    };
    let settings = GeneratorSettingsBuilder::new(200)
        .max_unsat_for_single_constraint(3)
        .cancel_condition(cancel_condition)
        .coeff_range(200)
        .build();
    let percentage_range_sat = 0.25..0.7;
    for counter in 1..=input.iterations {
        if counter == 1001 && !stats.is_sat_percentage_in_range(&percentage_range_sat) {
            // Terminate early if amount sat instances not in range
            // and more than 1000 iterations are scheduled
            println!("Terminating early!");
            println!(
                "Percentage SAT Instances not in range ({}..{})",
                percentage_range_sat.start, percentage_range_sat.end
            );
            break;
        }

        let seed = rand::random();

        print_block("", format!("Iteration {counter}"));
        println!("Running Fuzzer with seed {seed}...");
        let mut grammar = PBSGenerator::create_grammar(seed, settings);
        let write_result = File::create(&output_file)
            .map(|file| BufWriter::new(file))
            .map(|mut buf_writer| grammar.produce(&mut buf_writer));
        if write_result.is_err() {
            println!("Error while writing into output file");
            println!("Terminating orchestrator");
            break;
        }

        let res_program1 = match run_external_programm(&mut input.command1) {
            Ok(val) => val,
            Err(e) => {
                println!("{}", e);
                break;
            }
        };
        let res_program2 = match run_external_programm(&mut input.command2) {
            Ok(val) => val,
            Err(e) => {
                println!("{}", e);
                break;
            }
        };
        let status_program1 = &res_program1.status;
        let status_program2 = &res_program2.status;
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
        stats.update(&res_program1);
    }

    let res = save_settings(&settings, &input.fuzzer_output_dir);
    if let std::io::Result::Err(e) = res {
        println!("Could not save settings!\n Error: {}", e)
    }
    // print out settings
    print_block("Settings", settings);
    // print out stats
    print_block("Run Statistics", stats);
}

fn run_external_programm(
    command: &mut SolverCommand,
) -> Result<SolverResult, ProgramExecutionFailed> {
    command
        .exec_command
        .output()
        .map(|out| handle_command_output(&mut command.parser, &out))
        .map_err(|err| ProgramExecutionFailed {
            program_name: command.exec_name.clone(),
            error_message: err.to_string(),
        })
}

fn handle_command_output(parser: &mut OutputParser, output: &Output) -> SolverResult {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut conflicts = 0;
    for line in stdout.lines() {
        let line = line.trim();
        parser.parse_line(line);
        if line.contains("conflicts") {
            conflicts = line
                .split_whitespace()
                .last()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }
    let status = parser.status;
    SolverResult { status, conflicts }
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

pub struct OutputParser {
    status: SolverStatus,
    reg_sat: Regex,
    reg_unsat: Regex,
}

impl OutputParser {
    pub fn new(reg_sat: Regex, reg_unsat: Regex) -> Self {
        Self {
            status: SolverStatus::Unknown,
            reg_sat,
            reg_unsat,
        }
    }

    fn parse_line(&mut self, line: &str) {
        if self.reg_unsat.is_match(line) {
            self.status = SolverStatus::Unsat;
        } else if self.reg_sat.is_match(line) {
            self.status = SolverStatus::Sat;
        }
    }
}

pub struct SolverCommand {
    exec_name: String,
    exec_command: Command,
    parser: OutputParser,
}

impl SolverCommand {
    pub fn new(exec_name: String, exec_command: Command, parser: OutputParser) -> Self {
        Self {
            exec_name,
            exec_command,
            parser,
        }
    }

    fn set_input_file(&mut self, output_file: &PathBuf) {
        self.exec_command.arg(output_file);
    }
}

struct Stats {
    amount_sat: u32,
    amount_unsat: u32,
    amount_unknown: u32,
    average_conflicts: f32,
    start: Instant,
}

impl Stats {
    fn new() -> Self {
        Self {
            amount_sat: 0,
            amount_unsat: 0,
            amount_unknown: 0,
            average_conflicts: 0.0,
            start: Instant::now(),
        }
    }
    fn update(&mut self, result: &SolverResult) {
        // running average
        let n = self.amount_runs();
        self.average_conflicts =
            (n as f32 * self.average_conflicts + result.conflicts as f32) / (n + 1) as f32;
        match result.status {
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

    fn is_sat_percentage_in_range(&self, range: &Range<f32>) -> bool {
        range.contains(&self.percentage_sat())
    }
}

impl Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "Total amount of instances generated      : {}",
            self.amount_runs()
        )?;
        writeln!(
            f,
            "Generated amount of SAT instances        : {}",
            self.amount_sat
        )?;
        writeln!(
            f,
            "Generated amount of UNSAT instances      : {}",
            self.amount_unsat
        )?;
        writeln!(
            f,
            "Generated amount of unknown instances    : {}",
            self.amount_unknown
        )?;
        writeln!(
            f,
            "Percentage SAT                           : {}",
            self.percentage_sat()
        )?;
        writeln!(
            f,
            "Average conficts to solve                : {}",
            self.average_conflicts
        )?;
        let program_duration = Instant::now().duration_since(self.start);
        write!(
            f,
            "Duration of program                      : {}.{:.3}s",
            program_duration.as_secs(),
            program_duration.subsec_millis()
        )
    }
}

fn save_settings(settings: &GeneratorSettings, dir: &PathBuf) -> std::io::Result<()> {
    let json = serde_json::to_string_pretty(settings)?;
    let file_name = dir.join("settings.json");
    let mut file = File::create(file_name)?;
    file.write_all(json.as_bytes())
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

struct SolverResult {
    status: SolverStatus,
    conflicts: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum SolverStatus {
    Sat,
    Unsat,
    Unknown,
}

#[test]
fn test_stats() {
    let mut stats = Stats::new();
    let res = SolverResult {
        status: SolverStatus::Sat,
        conflicts: 4,
    };

    stats.update(&res);
    assert_eq!(1, stats.amount_runs());
    assert_eq!(1, stats.amount_sat);
    assert_eq!(0, stats.amount_unsat);
    assert_eq!(0, stats.amount_unknown);
    assert_float_approx(4.0, stats.average_conflicts, 0.0001);

    let res = SolverResult {
        status: SolverStatus::Unsat,
        conflicts: 3,
    };

    stats.update(&res);
    assert_eq!(2, stats.amount_runs());
    assert_eq!(1, stats.amount_sat);
    assert_eq!(1, stats.amount_unsat);
    assert_eq!(0, stats.amount_unknown);
    assert_float_approx(3.5, stats.average_conflicts, 0.0001);
}

#[cfg(test)]
fn assert_float_approx(a: f32, b: f32, epsilon: f32) {
    assert!(epsilon >= 0.0, "Epsilon needs to be a value >= 0");
    assert!(
        (a - b).abs() <= epsilon,
        "Assertion failed, float values {} and {} are not identical (epsilon: {})",
        a,
        b,
        epsilon
    )
}
