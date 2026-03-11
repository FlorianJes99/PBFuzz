use crate::{
    fuzzer::{
        grammar::PBKind,
        helpers::{RunningAverage, get_hms_parts, move_file},
    },
    orchestrator::{
        csv_data::{IterationRecord, ORCHESTRATOR_DATA_FILE_PREFIX},
        external_program::IterationOutcome,
        folders::{DATA_FOLDER, prepare_output_folder},
    },
};
use std::path::Path;
use std::{
    fmt::Display,
    fs::File,
    io::{BufWriter, Error, ErrorKind, Write},
    ops::Range,
    path::PathBuf,
    time::{Duration, Instant},
};

use itertools::Itertools;
use log::{error, info};
use rand::{self, Rng};
use ron::ser::PrettyConfig;

use crate::{
    fuzzer::{
        helpers::print_block,
        pbs_generator::{GeneratorSettings, PBSGenerator},
    },
    orchestrator::external_program::{PBOProcessor, PBSProcessor, SolverRunner},
};

pub fn run_orchestrator<U, T>(input: OrchestratorInput<T, U>) -> Result<(), String>
where
    T: SolverRunner + PBOProcessor + PBSProcessor,
    U: SolverRunner + PBOProcessor + PBSProcessor,
{
    prepare_output_folder(&input.fuzzer_output_dir)
        .map_err(|err| format!("Could not create workspace!\nError: {}", err))?;
    let program_start = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let csv_file_path = &input.fuzzer_output_dir.join(DATA_FOLDER.name).join(format!(
        "{}_{}.csv",
        ORCHESTRATOR_DATA_FILE_PREFIX, program_start
    ));
    let mut csv_writer = csv::Writer::from_path(csv_file_path)
        .map_err(|err| format!("Failed to create a csv writer\n Error: {}", err))?;

    let output_file = input.fuzzer_output_dir.join("test.opb");
    println!("Writing fuzzer output into {}", &output_file.display());

    let mut stats = Stats::new();

    let settings = input.generator_settings;
    print_block("Settings to use", &settings);

    let percentage_range_sat = 0.25..0.7;
    let mut rng = rand::rng();
    let timed_iter = input.stop_condition.into_timed_iterator();
    for (counter, elapsed_time) in timed_iter {
        let start_iteration = Instant::now();
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
            format!("Iteration {counter}\nTime elapsed: {}h {}m {}s", h, m, s),
        );
        println!("Running Fuzzer with seed {seed}...");
        let mut grammar = PBSGenerator::create_grammar(seed, settings);
        let duration_write_to_file = File::create(&output_file)
            .map(|file| BufWriter::new(file))
            .and_then(|mut buf_writer| grammar.produce(&mut buf_writer))
            .map_err(|e| e.to_string())?;
        let grammar_statistics = grammar.get_statistics();
        let output_generation_duration =
            *grammar_statistics.get_generation_time() + duration_write_to_file;
        info!(
            "Created problem in: {:.3}ms",
            (output_generation_duration.as_secs_f32() * 1000.0)
        );
        stats.update_average_input_generation_time(&output_generation_duration);
        println!(
            "Solving generated instance with {}",
            input.command1.get_name()
        );
        let start_bin1 = Instant::now();
        let output_bin1 = input.command1.run(&output_file);
        let time_bin1 = start_bin1.elapsed();
        println!(
            "Solving generated instance with {}",
            input.command2.get_name()
        );
        let start_bin2 = Instant::now();
        let output_bin2 = input.command2.run(&output_file);
        let time_bin2 = start_bin2.elapsed();
        println!("Parsing output of both binaries to extract results");
        let start_output_extraciton = Instant::now();
        let outcome = match settings.get_problem_kind() {
            PBKind::PBS => {
                let res_program1 = output_bin1.and_then(|output| {
                    PBSProcessor::extract_result_from_output(&input.command1, output)
                });
                let res_program2 = output_bin2.and_then(|output| {
                    PBSProcessor::extract_result_from_output(&input.command2, output)
                });
                match (res_program1, res_program2) {
                    (Ok(status1), Ok(status2)) => &status1.compare_status(&status2, &grammar),
                    (Err(err), _) => &IterationOutcome::Error { error: err },
                    (_, Err(err)) => &IterationOutcome::Error { error: err },
                }
            }
            PBKind::PBO => {
                let res_program1 = output_bin1.and_then(|output| {
                    PBOProcessor::extract_result_from_output(&input.command1, output)
                });
                let res_program2 = output_bin2.and_then(|output| {
                    PBOProcessor::extract_result_from_output(&input.command2, output)
                });
                match (res_program1, res_program2) {
                    (Ok(status1), Ok(status2)) => &status1.compare_status(&status2, &grammar),
                    (Err(err), _) => &IterationOutcome::Error { error: err },
                    (_, Err(err)) => &IterationOutcome::Error { error: err },
                }
            }
        };
        let time_output_extraction = start_output_extraciton.elapsed();

        if let IterationOutcome::Error { error: err } = outcome {
            error!("Some error occured in iteration!\nError: {}", err);
        }
        stats.update(outcome);
        let new_file_location = &input.fuzzer_output_dir.join(format!(
            "{}/{}_{}_{}.opb",
            outcome.get_folder_name(),
            outcome.get_file_name_prefix(),
            seed,
            counter
        ));
        let move_result = match outcome {
            // Only save the file if bug or error occurred
            IterationOutcome::Bug { .. }
            | IterationOutcome::Error { .. }
            | IterationOutcome::Unknown => move_file(&output_file, new_file_location),
            _ if input.save_all_files => move_file(&output_file, new_file_location),
            _ => Ok(()),
        };
        if let Err(err) = move_result {
            error!("Could not move file into specific folder\nError: {}", err);
        }
        let iteration_record = IterationRecord {
            iteration: counter,
            result: outcome.get_status_as_string(),
            duration_iteration_ms: start_iteration.elapsed().as_secs_f32() * 1000.0,
            problem_generation_ms: output_generation_duration.as_secs_f32() * 1000.0,
            problem_solving_duration_roundingsat_ms: time_bin1.as_secs_f32() * 1000.0,
            problem_solving_duration_scip_ms: time_bin2.as_secs_f32() * 1000.0,
            problem_parsing_duration_ms: time_output_extraction.as_secs_f32() * 1000.0,
            conflicts: outcome.get_conflicts(),
            seed,
        };
        let result = csv_writer.serialize(iteration_record);
        if let Err(err) = result {
            error!("Could not write record into csv\nError: {}", err);
        }
        if counter % 1000 == 0 {
            let result = csv_writer.flush();
            if let Err(err) = result {
                error!("Could not flush to file\n Error: {}", err);
            }
        }
    }
    let result = csv_writer.flush();
    if let Err(err) = result {
        error!("Could not flush to file\n Error: {}", err);
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
    save_all_files: bool,
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
        save_all_files: bool,
    ) -> Self {
        Self {
            stop_condition,
            fuzzer_output_dir,
            generator_settings,
            command1,
            command2,
            save_all_files,
        }
    }
}

struct Stats {
    amount_sat: u32,
    amount_unsat: u32,
    amount_unknown: u32,
    average_conflicts: RunningAverage,
    average_problem_generation_time: RunningAverage,
    bugs_found: u32,
    start: Instant,
}

impl Stats {
    fn new() -> Self {
        Self {
            amount_sat: 0,
            amount_unsat: 0,
            amount_unknown: 0,
            average_conflicts: RunningAverage::default(),
            average_problem_generation_time: RunningAverage::default(),
            bugs_found: 0,
            start: Instant::now(),
        }
    }

    fn update_average_input_generation_time(&mut self, output_generation_duration: &Duration) {
        match output_generation_duration.as_nanos().try_into() {
            Ok(millis) => self.average_problem_generation_time.update(millis),
            Err(err) => error!(
                "Could not update average problem generation!\nError: {}",
                err
            ),
        }
    }

    fn update(&mut self, outcome: &IterationOutcome) {
        match outcome {
            IterationOutcome::Sat { conflicts: c } => {
                self.amount_sat += 1;
                self.average_conflicts.update(*c);
            }
            IterationOutcome::Unsat { conflicts: c } => {
                self.amount_unsat += 1;
                self.average_conflicts.update(*c);
            }
            IterationOutcome::Unknown => self.amount_unknown += 1,
            IterationOutcome::Bug { .. } => self.bugs_found += 1,
            // Do nothing in case of error
            IterationOutcome::Error { .. } => (),
        }
    }
    fn amount_runs(&self) -> u32 {
        self.amount_sat + self.amount_unsat + self.amount_unknown + self.bugs_found
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
            self.average_conflicts.get_average()
        )?;
        writeln!(
            f,
            "Bugs found                               : {}",
            self.bugs_found
        )?;
        let program_duration = self.start.elapsed();
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
        writeln!(
            f,
            "Average time for problem generation      : {:.3}ms",
            self.average_problem_generation_time.get_average() / 1_000_000.0
        )?;
        let (h, m, s) = get_hms_parts(&program_duration);
        write!(
            f,
            "Duration of program                      : {}h {}m {}s",
            h, m, s
        )
    }
}

fn save_settings(
    settings: &GeneratorSettings,
    dir: &Path,
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

    use super::*;
    #[test]
    fn test_stats() {
        let mut stats = Stats::new();
        let res = IterationOutcome::Sat { conflicts: 0 };

        stats.update(&res);
        assert_eq!(1, stats.amount_runs());
        assert_eq!(1, stats.amount_sat);
        assert_eq!(0, stats.amount_unsat);
        assert_eq!(0, stats.amount_unknown);
        float_equals(4.0, stats.average_conflicts.get_average(), 0.0001);

        let res = IterationOutcome::Unsat { conflicts: 1 };

        stats.update(&res);
        assert_eq!(2, stats.amount_runs());
        assert_eq!(1, stats.amount_sat);
        assert_eq!(1, stats.amount_unsat);
        assert_eq!(0, stats.amount_unknown);
        float_equals(3.5, stats.average_conflicts.get_average(), 0.0001);
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
            assert!(elapsed >= Duration::from_millis((i - 1) as u64 * wait_time));
            thread::sleep(Duration::from_millis(wait_time));
        }
    }
}
