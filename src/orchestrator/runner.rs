use std::{
    fmt::Display,
    fs::{File},
    io::{BufWriter, Error, ErrorKind, Write},
    ops::Range,
    path::PathBuf,
    time::{Duration, Instant},
};

use itertools::Itertools;
use rand;
use ron::ser::PrettyConfig;

use crate::{
    fuzzer::{
        helpers::print_block,
        pbs_generator::{
            GeneratorSettings, PBSGenerator,
        },
    },
    orchestrator::external_program::{Solver, SolverResult, SolverStatus},
};

pub fn run_orchestrator<T: Solver, U: Solver>(
    input: OrchestratorInput<T, U>,
) -> Result<(), String> {
    let output_file = input.fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {}", &output_file.display());

    let mut stats = Stats::new();

    let settings = input.generator_settings;
    print_block("Settings to use", &settings);

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
        let _ok = File::create(&output_file)
            .map(|file| BufWriter::new(file))
            .map(|mut buf_writer| grammar.produce(&mut buf_writer))
            .map_err(|e| e.to_string())?;

        println!(
            "Solving generated instance with {}",
            input.command1.get_name()
        );
        let res_program1 = input
            .command1
            .execute_solver(&output_file)
            .map_err(|e| e.to_string())?;
        println!(
            "Solving generated instance with {}",
            input.command2.get_name()
        );
        let res_program2 = input
            .command2
            .execute_solver(&output_file)
            .map_err(|e| e.to_string())?;
        let status_program1 = res_program1.get_status();
        let status_program2 = res_program2.get_status();
        match (status_program1, status_program2) {
            (
                SolverStatus::Sat {
                    found_model: solution1,
                },
                SolverStatus::Sat {
                    found_model: solution2,
                },
            ) => {
                let valid1 = grammar.verify_solution(solution1);
                let valid2 = grammar.verify_solution(solution2);
                if valid1 && valid2 {
                    let file_path = input
                        .fuzzer_output_dir
                        .join(format!("sat/sat_{}_{}.opb", seed, counter));
                    move_file(&output_file, &file_path)?;
                    stats.update(&res_program1);
                } else {
                    let file_path = input
                        .fuzzer_output_dir
                        .join(format!("bug/invalid_solution_{}_{}.opb", seed, counter));
                    move_file(&output_file, &file_path)?;
                    stats.count_bug();
                }
            }
            (SolverStatus::Unsat, SolverStatus::Unsat) => {
                let file_path = input
                    .fuzzer_output_dir
                    .join(format!("unsat/unsat_{}_{}.opb", seed, counter));

                move_file(&output_file, &file_path)?;
                stats.update(&res_program1);
            }
            (SolverStatus::Unknown, SolverStatus::Unknown) => {
                let file_path = input
                    .fuzzer_output_dir
                    .join(format!("unknown/unknown_{}_{}.opb", seed, counter));

                move_file(&output_file, &file_path)?;
                stats.update(&res_program1);
            }
            // If status 1 and status 2 are unequal
            _ => {
                let file_path = input
                    .fuzzer_output_dir
                    .join(format!("bug/bug_{}_{}.opb", seed, counter));
                move_file(&output_file, &file_path)?;
                stats.count_bug();
            }
        }
    }

    save_settings(&settings, &input.fuzzer_output_dir, Some(&stats))
        .map_err(|e| format!("Could not save settings!\n Error: {}", e))?;
    // print out settings
    print_block("Settings", settings);
    // print out stats
    print_block("Run Statistics", stats);
    Ok(())
}

fn move_file(src: &PathBuf, dst: &PathBuf) -> Result<(), String> {
    std::fs::rename(&src, &dst).map_err(|e| {
        format!(
            "Could not move file from {} to {}\n Error: {}",
            &src.display(),
            &dst.display(),
            e
        )
    })
}

pub struct OrchestratorInput<T: Solver, U: Solver> {
    iterations: u32,
    fuzzer_output_dir: PathBuf,
    generator_settings: GeneratorSettings,
    command1: T,
    command2: U,
}

impl<T: Solver, U: Solver> OrchestratorInput<T, U> {
    pub fn new(
        iterations: u32,
        fuzzer_output_dir: PathBuf,
        generator_settings: GeneratorSettings,
        command1: T,
        command2: U,
    ) -> Self {
        Self {
            iterations,
            fuzzer_output_dir,
            generator_settings,
            command1,
            command2,
        }
    }
}

struct Stats {
    amount_sat: u32,
    amount_unsat: u32,
    amount_unknown: u32,
    average_conflicts: f32,
    bugs_found: u32,
    start: Instant,
}

impl Stats {
    fn new() -> Self {
        Self {
            amount_sat: 0,
            amount_unsat: 0,
            amount_unknown: 0,
            average_conflicts: 0.0,
            bugs_found: 0,
            start: Instant::now(),
        }
    }
    fn update(&mut self, result: &SolverResult) {
        // running average
        let n = self.amount_runs();
        self.average_conflicts =
            (n as f32 * self.average_conflicts + result.get_conflicts() as f32) / (n + 1) as f32;
        match result.get_status() {
            SolverStatus::Sat { found_model: _ } => self.amount_sat += 1,
            SolverStatus::Unsat => self.amount_unsat += 1,
            SolverStatus::Unknown => self.amount_unknown += 1,
        }
    }

    fn count_bug(&mut self) {
        self.bugs_found += 1
    }

    fn amount_runs(&self) -> u32 {
        self.amount_sat + self.amount_unsat + self.amount_unknown
    }

    fn percentage_sat(&self) -> f32 {
        if self.amount_runs() == 0 {
            return 0.0;
        }
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
            "Percentage SAT                           : {}%",
            self.percentage_sat() * 100.0
        )?;
        writeln!(
            f,
            "Average conficts to solve                : {}",
            self.average_conflicts
        )?;
        writeln!(
            f,
            "Bugs found                               : {}",
            self.bugs_found
        )?;
        let program_duration = Instant::now().duration_since(self.start);
        let avg_time_per_instance = if self.amount_runs() == 0 {
            Duration::new(0, 0)
        } else {
            program_duration / self.amount_runs()
        };
        writeln!(
            f,
            "Average Time per Instance                : {}.{:.3}s",
            avg_time_per_instance.as_secs(),
            avg_time_per_instance.subsec_millis()
        )?;
        write!(
            f,
            "Duration of program                      : {}.{:.3}s",
            program_duration.as_secs(),
            program_duration.subsec_millis()
        )
    }
}

fn save_settings(
    settings: &GeneratorSettings,
    dir: &PathBuf,
    stats_opt: Option<&Stats>,
) -> std::io::Result<()> {
    let data = ron::ser::to_string_pretty(settings, PrettyConfig::default())
        .map_err(|e| Error::new(ErrorKind::Other, e))?;
    let file_name = dir.join("settings.ron");
    let mut file = File::create(file_name)?;
    file.write_all(data.as_bytes())?;
    file.write_all("\n".as_bytes())?;
    if let Some(stats) = stats_opt {
        let commented_stats: String = stats
            .to_string()
            .lines()
            .map(|line| format!("// {}", line))
            .join("\n");
        file.write_all(commented_stats.as_bytes())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    #[test]
    fn test_stats() {
        let mut stats = Stats::new();
        let res = SolverResult::new(
            SolverStatus::Sat {
                found_model: HashSet::new(),
            },
            4,
        );

        stats.update(&res);
        assert_eq!(1, stats.amount_runs());
        assert_eq!(1, stats.amount_sat);
        assert_eq!(0, stats.amount_unsat);
        assert_eq!(0, stats.amount_unknown);
        assert_float_approx(4.0, stats.average_conflicts, 0.0001);

        let res = SolverResult::new(SolverStatus::Unsat, 3);

        stats.update(&res);
        assert_eq!(2, stats.amount_runs());
        assert_eq!(1, stats.amount_sat);
        assert_eq!(1, stats.amount_unsat);
        assert_eq!(0, stats.amount_unknown);
        assert_float_approx(3.5, stats.average_conflicts, 0.0001);
    }

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
}
