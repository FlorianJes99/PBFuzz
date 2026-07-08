use crate::fuzzer::helpers::{move_files, remove_files, MoveFile};
use crate::orchestrator::csv_data::{CsvRecord, ProofRecord};
use crate::orchestrator::external_program::ExternalProgramWithProof;
use crate::{
    fuzzer::{
        helpers::{get_hms_parts, RunningAverage, RunningAverageDuration},
        pbs_generator::GeneratorSettingsBuilder,
    },
    orchestrator::{
        csv_data::{ComparisonRecord, ORCHESTRATOR_DATA_FILE_PREFIX},
        external_program::{ExternalProgram, ExternalProgramSer, IterationOutcome, Solver},
        folders::{prepare_output_folder, DATA_FOLDER},
        orchestrator_args::OrchestratorArgs,
    },
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{BufWriter, Write},
    ops::Range,
    path::PathBuf,
    sync::mpsc::channel,
    thread,
    time::{Duration, Instant},
};

use csv::Writer;
use itertools::Itertools;
use log::{debug, error, info};
use rand::{self, RngExt};

use crate::fuzzer::{
    helpers::print_block,
    pbs_generator::{GeneratorSettings, PBSGenerator},
};

/// Entry point for the orchestrator to run fuzzer and external programs until a stop condition is
/// met.
///
/// The workflow here goes as follows:
/// - extract cli args and initialize all relevant data relevant for orchestrator
/// - prepare the output folder
///   - create output folder, if not yet exist
///   - clean up subdirectories, if needed to
/// - create the CSV writer, which writes stats of each iteration into a file
/// - create the actual orchestrator struct
/// - run iterations until:
///   - elapsed time > StopCondition(duration)
///   - iteration_count > StopCondition(MaxIterations)
///
/// One Iterations consists of the following two blocks.
///
/// Block 1:
/// - Create a fuzzing output (`*.opb` file)
/// - Solve file with binary 1
/// - Solve file with binary 2
/// - Return output of both binaries as well as some time benchmarks and other needed information
///
/// Block 2:
/// - Extract result from both outputs
///   - Verify found model for each program, if SAT
///   - Verify found minimum objective for each program, if SAT and PBO
/// - Compare if both programs output the same result
/// - Update stats for the orchestrator
/// - write relevant data points to the CSV file
pub fn run_orchestrator(
    time_stamp_as_str: &str,
    args: OrchestratorArgs,
) -> Result<(), Box<dyn Error>> {
    let thread_count = args.thread_count.unwrap_or(1);
    info!(
        "Using {} threads for computing fuzzing runs in parallel",
        thread_count
    );
    let timelimit = Duration::from_secs(args.timeout_external_program_secs);
    let mode = match &args.program_config2 {
        Some(p2) => OrchestrationMode::Comparison(
            ExternalProgramSer::load_from_file(&args.program_config1)?
                .to_external_program(timelimit),
            ExternalProgramSer::load_from_file(p2)?.to_external_program(timelimit),
        ),
        None => OrchestrationMode::ProofChecking(
            ExternalProgramSer::load_from_file(&args.program_config1)?
                .to_external_program_with_proof(timelimit)?,
        ),
    };
    let mut generator_settings_builder = match args.settings_file {
        Some(file) => GeneratorSettingsBuilder::load_from_file(&file)?,
        None => GeneratorSettingsBuilder::default(),
    };

    if let Some(pb_kind) = args.mode {
        generator_settings_builder = generator_settings_builder.pb_kind(pb_kind);
    }
    let generator_settings = generator_settings_builder.build();
    let stop_condition = StopCondition::new(
        args.iterations,
        args.timeout_min.map(|v| Duration::from_mins(v as u64)),
    )?;
    let output_dir = &args.output_dir;
    info!("Workspace set to: {}", output_dir.display());
    let fuzzer_output_dir = prepare_output_folder(
        output_dir,
        time_stamp_as_str,
        generator_settings.get_problem_kind().get_short_str(),
    )?;
    print_block("Settings to use", generator_settings);

    Orchestrator {
        fuzzer_output_dir,
        generator_settings,
        orchestrator_start: Instant::now(),
        save_all_files: args.save_all_files,
        mode,
        thread_count,
    }
    .run(stop_condition)
}

enum OrchestrationMode {
    Comparison(ExternalProgram, ExternalProgram),
    ProofChecking(ExternalProgramWithProof),
}

struct Orchestrator {
    fuzzer_output_dir: PathBuf,
    generator_settings: GeneratorSettings,
    save_all_files: bool,
    orchestrator_start: Instant,
    mode: OrchestrationMode,
    thread_count: u8,
}

impl Orchestrator {
    fn run(&self, stop_condition: StopCondition) -> Result<(), Box<dyn Error>> {
        let csv_file_path = &self
            .fuzzer_output_dir
            .join(DATA_FOLDER.name)
            .join(format!("{}.csv", ORCHESTRATOR_DATA_FILE_PREFIX,));
        let mut csv_writer = csv::Writer::from_path(csv_file_path)?;
        let mut stats = Stats::new();
        let stop_condtion = Arc::new(stop_condition);
        let cancelled = Arc::new(AtomicBool::new(false));
        let ctrc_flag = cancelled.clone();
        ctrlc::set_handler(move || {
            println!("Registered user termination!");
            println!("Shutting down gracefully...");
            ctrc_flag.store(true, Ordering::Relaxed);
        })?;
        let count_runs = Arc::new(AtomicU32::new(0));
        let (sender, receiver) = channel::<WorkResult>();
        let start = Instant::now();
        thread::scope(|s| {
            for i in 1..=self.thread_count {
                let sender = sender.clone();
                let cancelled = Arc::clone(&cancelled);
                let count_runs = Arc::clone(&count_runs);
                let stop_condition = Arc::clone(&stop_condtion);
                s.spawn(move || {
                    while !cancelled.load(Ordering::Relaxed) {
                        let iteration = count_runs.fetch_add(1, Ordering::Relaxed);
                        if stop_condition.is_done(iteration, start) {
                            break;
                        }
                        match work_iteration(
                            &self.fuzzer_output_dir,
                            self.generator_settings,
                            &self.mode,
                            i,
                        ) {
                            Ok(work_result) => {
                                if let Err(e) = sender.send(work_result) {
                                    error!("{e}");
                                }
                            }
                            Err(e) => error!("{e}"),
                        }
                    }
                });
            }
            // immediately drop the initial sender reference, only the clones inside the worker threads are kept alive
            drop(sender);
            for (iteration, work_result) in receiver.iter().enumerate() {
                let iteration = iteration as u32 + 1; // we want to start counting from 1.. upwards
                let percentage_range_sat = 0.20..0.7;
                if iteration == 1001 && !stats.is_sat_percentage_in_range(&percentage_range_sat) {
                    // Terminate early if amount sat instances not in range
                    // and more than 1000 iterations are scheduled
                    println!("Terminating early!");
                    println!(
                        "Percentage SAT Instances not in range ({}..{})",
                        percentage_range_sat.start, percentage_range_sat.end
                    );
                    cancelled.store(true, Ordering::Relaxed);
                    // We don't break here, because we want to process the remaining running threads
                    // By setting cancelled to false, we ensure threads won't start new work, but finish already running work
                }
                self.process_result(work_result, iteration, &mut csv_writer, &mut stats);
            }
            let result = &mut csv_writer.flush();
            if let Err(err) = result {
                error!("Could not flush to file\n Error: {}", err);
            }

            if let Err(e) = self.save_settings(&stats) {
                error!("{e}");
            }
            // print out settings
            print_block("Settings", self.generator_settings);
            // print out stats
            print_block("Run Statistics", &stats);
        });
        Ok(())
    }

    fn process_result(
        &self,
        work_result: WorkResult,
        iteration: u32,
        csv_writer: &mut Writer<File>,
        stats: &mut Stats,
    ) {
        let elapsed_since_start = self.orchestrator_start.elapsed();
        let (h, m, s) = get_hms_parts(&elapsed_since_start);
        let throughput = iteration as f64 / elapsed_since_start.as_secs_f64();
        #[rustfmt::skip]
        let body = [
            format!("Throughput: {:.3} Iterations/s ({} threads)", throughput, self.thread_count),
            format!("Time elapsed: {}h {}m {}s", h, m, s),
        ].join("\n");
        print_block(&format!("Iteration {iteration}"), body);
        let seed = work_result.record.get_seed();
        let outcome = &work_result.outcome;
        if let IterationOutcome::Error { error: err } = outcome {
            error!(
                "Some error occured in iteration for seed {})!\nError: {}",
                seed, err
            );
        }
        if let IterationOutcome::Bug { reason } = outcome {
            info!("Found bug!\nBug: {}", reason)
        }
        let mut files_to_move = vec![];
        let output_file = work_result.pb_file;
        let new_file_location = self.fuzzer_output_dir.join(format!(
            "{}/{}_{}_{}.opb",
            outcome.get_folder_name(),
            outcome.get_file_name_prefix(),
            seed,
            iteration
        ));
        files_to_move.push(MoveFile::new(output_file, new_file_location));
        if let Some(proof_file) = work_result.proof_file {
            let new_file_location_proof = self.fuzzer_output_dir.join(format!(
                "{}/{}_{}_{}.pbp",
                outcome.get_folder_name(),
                outcome.get_file_name_prefix(),
                seed,
                iteration
            ));
            files_to_move.push(MoveFile::new(proof_file, new_file_location_proof));
        }
        let file_manipulation_result = match outcome {
            // Only save the file if bug or error or `unknown` occurred
            IterationOutcome::Bug { .. }
            | IterationOutcome::Error { .. }
            | IterationOutcome::TimeLimt
            | IterationOutcome::Unknown => move_files(&files_to_move),
            _ if self.save_all_files => move_files(&files_to_move),
            // Delete the generated fuzzing result if not needed anymore
            _ => remove_files(&files_to_move).map_err(|e| e.to_string()),
        };
        if let Err(err) = file_manipulation_result {
            error!("{err}");
        }
        let duration_iteration_ms = work_result.start_worker.elapsed();
        stats.update(outcome);
        stats.update_iteration_duration(&duration_iteration_ms);
        let result = work_result.record.serialize(
            iteration,
            duration_iteration_ms.as_secs_f32() * 1000.0,
            csv_writer,
        );
        if let Err(err) = result {
            error!("Could not write record into csv\nError: {}", err);
        }
        if iteration.is_multiple_of(1000) {
            // flush every 1000 iterations to file
            let result = csv_writer.flush();
            if let Err(err) = result {
                error!("Could not flush to file\n Error: {}", err);
            }
        }
    }

    fn save_settings(&self, stats: &Stats) -> std::io::Result<()> {
        let dir = self.fuzzer_output_dir.join(DATA_FOLDER.name);
        let data =
            toml::ser::to_string_pretty(&self.generator_settings).map_err(std::io::Error::other)?;
        let file_name = dir.join("settings.toml");
        let mut file = File::create(file_name)?;
        file.write_all(data.as_bytes())?;
        file.write_all("\n".as_bytes())?;
        let commented_stats: String = stats
            .to_string()
            .lines()
            .map(|line| format!("# {}", line)) // # denotes a comment in toml
            .join("\n");
        file.write_all(commented_stats.as_bytes())
    }
}

fn get_seed() -> u64 {
    rand::rng().random()
}

fn work_iteration(
    fuzzer_output_dir: &Path,
    settings: GeneratorSettings,
    mode: &OrchestrationMode,
    thread_id: u8,
) -> Result<WorkResult, Box<dyn Error>> {
    let start_worker = Instant::now();
    let seed = get_seed();
    let file_name = format!("{thread_id}_test_{seed}");
    let output_file = fuzzer_output_dir.join(format!("{file_name}.opb"));
    debug!("Running Fuzzer with seed {seed}...");
    let grammar = PBSGenerator::create_grammar(seed, settings)?;
    let duration_write_to_file = File::create(&output_file)
        .map(BufWriter::new)
        .and_then(|mut buf_writer| grammar.produce(&mut buf_writer))?;
    let grammar_statistics = grammar.get_statistics();
    let output_generation_duration =
        *grammar_statistics.get_generation_time() + duration_write_to_file;
    debug!(
        "Created problem in: {:.3}ms",
        (output_generation_duration.as_secs_f32() * 1000.0)
    );

    let pb_kind = settings.get_problem_kind();
    let with_option_fuzzing = rand::random_bool(0.1); // option fuzzing with 10% probability
    if with_option_fuzzing {
        info!("Using option fuzzing in this run...")
    }
    match mode {
        OrchestrationMode::Comparison(p1, p2) => {
            let start_bin1 = Instant::now();
            let output_bin1 = p1.run(&output_file, with_option_fuzzing)?;
            let time_bin1 = start_bin1.elapsed();

            debug!("Solving generated instance with {}", p2.get_name());
            let start_bin2 = Instant::now();
            let output_bin2 = p2.run(&output_file, with_option_fuzzing)?;
            let time_bin2 = start_bin2.elapsed();

            let start_output_extraction = Instant::now();
            let res_program1 = p1.parse(output_bin1, &pb_kind);
            let res_program2 = p2.parse(output_bin2, &pb_kind);
            let outcome = match (res_program1, res_program2) {
                (Ok(status1), Ok(status2)) => status1.verify_and_compare_status(
                    &status2,
                    &grammar,
                    p1.get_name(),
                    p2.get_name(),
                ),
                (Err(err), _) => IterationOutcome::Error { error: err },
                (_, Err(err)) => IterationOutcome::Error { error: err },
            };
            let time_output_extraction = start_output_extraction.elapsed();
            let comparison_record = ComparisonRecord {
                result: outcome.get_status_as_string(),
                problem_generation_ms: output_generation_duration.as_secs_f32() * 1000.0,
                problem_solving_duration_roundingsat_ms: time_bin1.as_secs_f32() * 1000.0,
                problem_solving_duration_scip_ms: time_bin2.as_secs_f32() * 1000.0,
                problem_parsing_duration_ms: time_output_extraction.as_secs_f32() * 1000.0,
                conflicts: outcome.get_conflicts(),
                seed,
            };
            Ok(WorkResult {
                start_worker,
                pb_file: output_file,
                proof_file: None,
                outcome,
                record: CsvRecord::Comparison(comparison_record),
            })
        }
        OrchestrationMode::ProofChecking(p) => {
            let proof_file = fuzzer_output_dir.join(format!("{file_name}.pbp"));
            let problem_solving_start = Instant::now();
            let output = p.run_with_proof(&output_file, &proof_file, with_option_fuzzing)?;
            let problem_solving_duration = problem_solving_start.elapsed();
            let start_output_parsing = Instant::now();
            let parsing_result = p.parse(output, &pb_kind);
            let duration_output_parsing = start_output_parsing.elapsed();
            let proof_checking_start = Instant::now();
            let outcome = parsing_result.map_or_else(
                |err| IterationOutcome::Error { error: err },
                |r| r.verify_with_proof_checking(&grammar, &output_file, &proof_file, p.get_name()),
            );
            let proof_checking_duration = proof_checking_start.elapsed();
            let proof_record = ProofRecord {
                result: outcome.get_status_as_string(),
                problem_generation_ms: output_generation_duration.as_secs_f32() * 1000.0,
                problem_solving_duration_program: problem_solving_duration.as_secs_f32() * 1000.0,
                problem_parsing_duration_ms: duration_output_parsing.as_secs_f32() * 1000.0,
                problem_checking_duration_ms: proof_checking_duration.as_secs_f32() * 1000.0,
                conflicts: outcome.get_conflicts(),
                seed,
            };
            Ok(WorkResult {
                start_worker,
                pb_file: output_file,
                proof_file: Some(proof_file),
                outcome,
                record: CsvRecord::Proof(proof_record),
            })
        }
    }
}

struct WorkResult {
    start_worker: Instant,
    pb_file: PathBuf,
    proof_file: Option<PathBuf>,
    outcome: IterationOutcome,
    record: CsvRecord,
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

    fn is_done(&self, iteration: u32, start: Instant) -> bool {
        match self {
            StopCondition::MaxIterations(n) => iteration >= *n,
            StopCondition::MaxDuration(duration) => start.elapsed() >= *duration,
        }
    }
}

struct Stats {
    amount_sat: u32,
    amount_unsat: u32,
    amount_unknown: u32,
    average_conflicts: RunningAverage,
    average_iteration_duration: RunningAverageDuration,
    bugs_found: u32,
    errors: u32,
    timeouts: u32,
    start: Instant,
}

impl Stats {
    fn new() -> Self {
        Self {
            amount_sat: 0,
            amount_unsat: 0,
            amount_unknown: 0,
            average_conflicts: RunningAverage::default(),
            average_iteration_duration: RunningAverageDuration::default(),
            bugs_found: 0,
            errors: 0,
            timeouts: 0,
            start: Instant::now(),
        }
    }

    fn update_iteration_duration(&mut self, iteration_duration: &Duration) {
        self.average_iteration_duration.update(iteration_duration);
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
            IterationOutcome::Error { .. } => self.errors += 1,
            IterationOutcome::TimeLimt => self.timeouts += 1,
        }
    }
    fn amount_runs(&self) -> u32 {
        self.amount_sat + self.amount_unsat + self.amount_unknown + self.bugs_found + self.timeouts
    }

    fn percentage_sat(&self) -> f32 {
        let sat_unsat = self.amount_sat + self.amount_unsat;
        if sat_unsat == 0 {
            return 0.0;
        }
        self.amount_sat as f32 / sat_unsat as f32
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
            "Average conflicts to solve               : {}",
            self.average_conflicts.get_average()
        )?;
        writeln!(
            f,
            "Bugs found                               : {}",
            self.bugs_found
        )?;
        writeln!(
            f,
            "Errors occured                           : {}",
            self.errors
        )?;
        writeln!(
            f,
            "Timeouts                                 : {}",
            self.timeouts
        )?;
        writeln!(
            f,
            "Average Time per Instance                : {:.3}ms",
            self.average_iteration_duration.get_average().as_secs_f32() * 1_000.0,
        )?;
        let program_duration = self.start.elapsed();
        let (h, m, s) = get_hms_parts(&program_duration);
        write!(
            f,
            "Duration of program                      : {}h {}m {}s",
            h, m, s
        )
    }
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
    }
}
