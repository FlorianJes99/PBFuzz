use crate::fuzzer::{
    grammar::PBKind,
    helpers::{get_hms_parts, move_file},
};
use std::{
    fmt::Display,
    fs::File,
    io::{BufWriter, Error, ErrorKind, Write},
    ops::Range,
    path::PathBuf,
    time::{Duration, Instant},
};

use itertools::Itertools;
use rand::{self, Rng};
use ron::ser::PrettyConfig;

use crate::{
    fuzzer::{
        helpers::{compute_running_average, print_block},
        pbs_generator::{GeneratorSettings, PBSGenerator},
    },
    orchestrator::external_program::{
        PBOProcessor, PBSProcessor, SolverResult, SolverRunner, SolverStatus, run_pbo, run_pbs,
    },
};

pub fn run_orchestrator<U, T>(input: OrchestratorInput<T, U>) -> Result<(), String>
where
    T: SolverRunner + PBOProcessor + PBSProcessor,
    U: SolverRunner + PBOProcessor + PBSProcessor,
{
    let output_file = input.fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {}", &output_file.display());

    let mut stats = Stats::new();

    let settings = input.generator_settings;
    print_block("Settings to use", &settings);

    let percentage_range_sat = 0.25..0.7;
    let mut rng = rand::rng();
    let timed_iter = input.stop_condition.into_timed_iterator();
    for (counter, elapsed_time) in timed_iter {
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

        let seed = rng.random();

        let (h, m, s) = get_hms_parts(&elapsed_time);
        print_block(
            "",
            format!("Iteration {counter}\nTime elapsed {}h {}m {}s", h, m, s),
        );
        println!("Running Fuzzer with seed {seed}...");
        let mut grammar = PBSGenerator::create_grammar(seed, settings);
        let _ok = File::create(&output_file)
            .map(|file| BufWriter::new(file))
            .map(|mut buf_writer| grammar.produce(&mut buf_writer))
            .map_err(|e| e.to_string())?;

        match settings.get_problem_kind() {
            PBKind::PBS => {
                println!(
                    "Solving generated instance with {}",
                    input.command1.get_name()
                );
                let res_program1 =
                    run_pbs(&input.command1, &output_file).map_err(|e| e.to_string())?;
                println!(
                    "Solving generated instance with {}",
                    input.command2.get_name()
                );
                let res_program2 =
                    run_pbs(&input.command2, &output_file).map_err(|e| e.to_string())?;
                let status_program1 = res_program1.get_status();
                let status_program2 = res_program2.get_status();
                match (status_program1, status_program2) {
                    (SolverStatus::Sat(solution1), SolverStatus::Sat(solution2)) => {
                        let valid1 = grammar.verify_solution(solution1.get_true_model());
                        let valid2 = grammar.verify_solution(solution2.get_true_model());
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
            PBKind::PBO => {
                println!(
                    "Solving generated instance with {}",
                    input.command1.get_name()
                );
                let res_program1 =
                    run_pbo(&input.command1, &output_file).map_err(|e| e.to_string())?;
                println!(
                    "Solving generated instance with {}",
                    input.command2.get_name()
                );
                let res_program2 =
                    run_pbo(&input.command2, &output_file).map_err(|e| e.to_string())?;
                let status_program1 = res_program1.get_status();
                let status_program2 = res_program2.get_status();
                match (status_program1, status_program2) {
                    (SolverStatus::Sat(solution1), SolverStatus::Sat(solution2)) => {
                        let valid1 = grammar.verify_solution(solution1.get_true_model())
                            && grammar.verify_min_constraint(
                                solution1.get_true_model(),
                                solution1.get_min_objective_values(),
                            );
                        let valid2 = grammar.verify_solution(solution2.get_true_model())
                            && grammar.verify_min_constraint(
                                solution2.get_true_model(),
                                solution2.get_min_objective_values(),
                            );
                        let same_objective = solution1.get_min_objective_values()
                            == solution2.get_min_objective_values();
                        if !same_objective {
                            let file_path = input
                                .fuzzer_output_dir
                                .join(format!("bug/different_objectives_{}_{}.opb", seed, counter));
                            move_file(&output_file, &file_path)?;
                            stats.count_bug();
                        } else if !(valid1 || valid2) {
                            let file_path = input
                                .fuzzer_output_dir
                                .join(format!("bug/invalid_solution_{}_{}.opb", seed, counter));
                            move_file(&output_file, &file_path)?;
                            stats.count_bug();
                        } else {
                            let file_path = input
                                .fuzzer_output_dir
                                .join(format!("sat/sat_{}_{}.opb", seed, counter));
                            move_file(&output_file, &file_path)?;
                            stats.update(&res_program1);
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
                        let file_path = input.fuzzer_output_dir.join(format!(
                            "bug/output_status_differs_{}_{}.opb",
                            seed, counter
                        ));
                        move_file(&output_file, &file_path)?;
                        stats.count_bug();
                    }
                }
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

/// Iterator, which additionally keeps track of elapsed time during iterations
///
/// Depending on the stop condition, this iterator will stop producing values
/// after reaching the maximum iterations or a maximum duration is elapsed
struct TimedIterator {
    start: Instant,
    stop_condition: StopCondition,
    current: u32,
}

impl Iterator for TimedIterator {
    type Item = (u32, Duration);

    fn next(&mut self) -> Option<Self::Item> {
        match self.stop_condition {
            StopCondition::MaxIterations(iter) if self.current > iter => return None,
            StopCondition::MaxDuration(duration) if self.start.elapsed() >= duration => {
                return None;
            }
            _ => {
                // Clone the current value for returning before incrementing
                let result = Some((self.current.clone(), self.start.elapsed()));
                self.current += 1;
                result
            }
        }
    }
}

/// Cancel iterations of fuzzing runs
///
/// Stop either after n iterations or after time t is passed
#[derive(Debug)]
pub enum StopCondition {
    MaxIterations(u32),
    MaxDuration(Duration),
}

impl StopCondition {
    /// Create the stop condition from either a maximum iteration threshold or a maximum duration
    ///
    /// Creation can fail if:
    ///     max iterations < 1
    ///     both passed values are Some
    ///     both passed values are None
    pub fn new(max_iter: Option<u32>, max_duration: Option<Duration>) -> Result<Self, String> {
        match (max_iter, max_duration) {
            (None, None) => Err("One cancelation method exptected, zero provided".to_string()),
            (None, Some(d)) => Ok(Self::MaxDuration(d)),
            // Because we start iterations at 1, max iterations needs to be at least 1
            (Some(i), None) if i < 1 => Err(format!(
                "Max iteration needs to be larger than 0, actual: {}",
                i
            )),
            (Some(i), None) => Ok(Self::MaxIterations(i)),
            (Some(_), Some(_)) => {
                Err("Expected one cancelation method, but provided two".to_string())
            }
        }
    }

    fn into_timed_iterator(self) -> TimedIterator {
        TimedIterator {
            start: Instant::now(),
            stop_condition: self,
            // start at index one
            current: 1,
        }
    }
}

pub struct OrchestratorInput<T, U>
where
    T: SolverRunner + PBOProcessor + PBSProcessor,
    U: SolverRunner + PBOProcessor + PBSProcessor,
{
    stop_condition: StopCondition,
    fuzzer_output_dir: PathBuf,
    generator_settings: GeneratorSettings,
    command1: T,
    command2: U,
}

impl<T, U> OrchestratorInput<T, U>
where
    T: SolverRunner + PBOProcessor + PBSProcessor,
    U: SolverRunner + PBOProcessor + PBSProcessor,
{
    pub fn new(
        stop_condition: StopCondition,
        fuzzer_output_dir: PathBuf,
        generator_settings: GeneratorSettings,
        command1: T,
        command2: U,
    ) -> Self {
        Self {
            stop_condition,
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
    fn update<M>(&mut self, result: &SolverResult<M>) {
        // running average
        let n = self.amount_runs();
        self.average_conflicts =
            compute_running_average(self.average_conflicts, result.get_conflicts(), n);
        match result.get_status() {
            SolverStatus::Sat(_) => self.amount_sat += 1,
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
    use crate::fuzzer::helpers::float_equals;
    use crate::orchestrator::external_program::{PBSValues, SolverResultPBS};
    use std::collections::HashSet;

    use super::*;
    #[test]
    fn test_stats() {
        let mut stats = Stats::new();
        let res = SolverResultPBS::new(SolverStatus::Sat(PBSValues::new(HashSet::new())), 4);

        stats.update(&res);
        assert_eq!(1, stats.amount_runs());
        assert_eq!(1, stats.amount_sat);
        assert_eq!(0, stats.amount_unsat);
        assert_eq!(0, stats.amount_unknown);
        float_equals(4.0, stats.average_conflicts, 0.0001);

        let res = SolverResultPBS::new(SolverStatus::Unsat, 3);

        stats.update(&res);
        assert_eq!(2, stats.amount_runs());
        assert_eq!(1, stats.amount_sat);
        assert_eq!(1, stats.amount_unsat);
        assert_eq!(0, stats.amount_unknown);
        float_equals(3.5, stats.average_conflicts, 0.0001);
    }

    #[test]
    fn test_iteration_timer() {
        let err = StopCondition::new(Some(3), Some(Duration::from_mins(1)));
        assert!(err.is_err());
        let err = StopCondition::new(None, None);
        assert!(err.is_err());
        let err = StopCondition::new(Some(0), None);
        assert!(err.is_err());
        let count_iter = StopCondition::new(Some(3), None)
            .unwrap()
            .into_timed_iterator();
        let vec: Vec<u32> = count_iter.into_iter().map(|t| t.0).sorted().collect();
        assert_eq!(vec![1, 2, 3], vec);
        let count_iter = StopCondition::new(Some(1), None)
            .unwrap()
            .into_timed_iterator();
        let vec: Vec<u32> = count_iter.into_iter().map(|t| t.0).sorted().collect();
        assert_eq!(vec![1], vec);
        let duration_iter = StopCondition::new(None, Some(Duration::from_millis(10)))
            .unwrap()
            .into_timed_iterator();
        use std::thread;
        let wait_time = 1;
        for (i, elapsed) in duration_iter {
            assert!(elapsed >= Duration::from_millis((i as u64 -1) * wait_time));
            thread::sleep(Duration::from_millis(wait_time));
        }
    }
}
