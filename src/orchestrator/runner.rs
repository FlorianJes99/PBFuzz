use crate::fuzzer::app_info::MetaData;
use crate::fuzzer::helpers::{MoveFile, OFFSET, move_file, move_files, remove_files};
use crate::fuzzer::pb_generator::PbGenerationError;
use crate::fuzzer::pb_settings::{GenerationMethod, PbConfigFile, PbSetting};
use crate::orchestrator::csv_data::{CsvRecord, ProofRecord};
use crate::orchestrator::evaluation::common::VerificationOutcome;
use crate::orchestrator::evaluation::comparison::{ComparisonRun, compare_runs};
use crate::orchestrator::evaluation::proof_checking::{ProofRun, verify_with_proof};
use crate::orchestrator::folders::ERROR_FOLDER;
use crate::orchestrator::parsing::parser::{
    OutputParsingError, ParsedStatus, ParserImpl, ParserRun,
};
use crate::orchestrator::run_statistics::{
    RunStatistics, SimpleResult, SingleRunRecord, SingleRunStats,
};
use crate::orchestrator::solving::solver::{
    NoProof, SolverExecutionFailed, SolverExecutionRecord, SolverInitializationError, WithProof,
};
use crate::orchestrator::solving::solver_config::SolverConfig;
use crate::{
    fuzzer::helpers::get_hms_parts,
    orchestrator::{
        csv_data::{ComparisonRecord, ORCHESTRATOR_DATA_FILE_PREFIX},
        folders::{DATA_FOLDER, prepare_sub_folders},
        orchestrator_args::OrchestratorArgs,
        solving::solver::SolverData,
    },
};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
use log::{debug, error, info, warn};
use rand::{self, RngExt};
use serde::Serialize;
use thiserror::Error;

use crate::fuzzer::{helpers::print_block, pb_generator::PBGenerator};

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
    meta_data: MetaData,
    args: OrchestratorArgs,
    run_root: PathBuf,
) -> Result<(), Box<dyn Error>> {
    let thread_count = args.thread_count.unwrap_or(1);
    info!(
        "Using {} threads for computing fuzzing runs in parallel",
        thread_count
    );
    let timelimit = Duration::from_secs(args.timeout_external_program_secs);
    let mode = match &args.program_config2 {
        Some(p2) => {
            let c1 = SolverConfig::load_from_file(&args.program_config1)?;
            let solver1 = SolverAndParser::from_config(c1, timelimit)?;
            let c2 = SolverConfig::load_from_file(p2)?;
            let solver2 = SolverAndParser::from_config(c2, timelimit)?;
            OrchestrationMode::Comparison(solver1, solver2)
        }
        None => {
            let config = SolverConfig::load_from_file(&args.program_config1)?;
            OrchestrationMode::ProofChecking(SolverAndParser::from_config_proof(config, timelimit)?)
        }
    };
    let config = PbConfigFile::load_from_file(&args.settings_file)?;
    let stop_condition = StopCondition::new(
        args.iterations,
        args.timeout_min.map(|v| Duration::from_mins(v as u64)),
    )?;
    info!("Workspace set to: {}", run_root.display());
    prepare_sub_folders(&run_root)?;
    print_block("Configuration", config);

    let save_all_files = args.save_all_files;
    Orchestrator {
        fuzzer_output_dir: run_root,
        config,
        orchestrator_start: Instant::now(),
        save_all_files,
        mode,
        thread_count,
    }
    .run(stop_condition, meta_data)
}

struct SolverAndParser<T = NoProof> {
    solver: SolverData<T>,
    parser: ParserImpl,
}

impl SolverAndParser {
    fn from_config(
        config: SolverConfig,
        timelimit: Duration,
    ) -> Result<Self, SolverInitializationError> {
        let parser = ParserImpl::from_kind(&config.parser_kind);
        Ok(Self {
            solver: SolverData::into_solver(config, timelimit)?,
            parser,
        })
    }
}

impl SolverAndParser<WithProof> {
    fn from_config_proof(
        config: SolverConfig,
        timelimit: Duration,
    ) -> Result<Self, SolverInitializationError> {
        let parser = ParserImpl::from_kind(&config.parser_kind);
        Ok(Self {
            solver: SolverData::into_proof_solver(config, timelimit)?,
            parser,
        })
    }
}
enum OrchestrationMode {
    Comparison(SolverAndParser, SolverAndParser),
    ProofChecking(SolverAndParser<WithProof>),
}

struct Orchestrator {
    fuzzer_output_dir: PathBuf,
    config: PbConfigFile,
    save_all_files: bool,
    orchestrator_start: Instant,
    mode: OrchestrationMode,
    thread_count: u8,
}

#[derive(Serialize)]
struct Summary {
    meta_data: MetaData,
    duration_program: Duration,
    statistics_run: RunStatistics,
}

impl Summary {
    fn save_to_file(self, file: &Path) -> io::Result<()> {
        let data = toml::ser::to_string_pretty(&self).map_err(io::Error::other)?;
        File::create(file).and_then(|mut f| f.write_all(data.as_bytes()))
    }
}

impl Orchestrator {
    fn run(
        &self,
        stop_condition: StopCondition,
        meta_data: MetaData,
    ) -> Result<(), Box<dyn Error>> {
        let csv_file_path = &self
            .fuzzer_output_dir
            .join(DATA_FOLDER.name)
            .join(format!("{}.csv", ORCHESTRATOR_DATA_FILE_PREFIX,));
        let mut csv_writer = csv::Writer::from_path(csv_file_path)?;
        let stop_condtion = Arc::new(stop_condition);
        let cancelled = Arc::new(AtomicBool::new(false));
        let ctrc_flag = cancelled.clone();
        ctrlc::set_handler(move || {
            println!("Registered user termination!");
            println!("Shutting down gracefully...");
            ctrc_flag.store(true, Ordering::Relaxed);
        })?;
        let count_runs = Arc::new(AtomicU32::new(0));
        let (sender, receiver) = channel::<Result<WorkResult, Box<WorkError>>>();
        let start = Instant::now();
        thread::scope(|s| {
            for i in 1..=self.thread_count {
                let sender = sender.clone();
                let cancelled = Arc::clone(&cancelled);
                let count_runs = Arc::clone(&count_runs);
                let stop_condition = Arc::clone(&stop_condtion);
                thread::Builder::new()
                    .name(format!("thread-{i}"))
                    .spawn_scoped(s, move || {
                        while !cancelled.load(Ordering::Relaxed) {
                            let iteration = count_runs.fetch_add(1, Ordering::Relaxed);
                            if stop_condition.is_done(iteration, start) {
                                break;
                            }
                            let result = work_iteration(
                                &self.fuzzer_output_dir,
                                self.config.generate_settings(),
                                &self.mode,
                                meta_data,
                                i,
                            );
                            if let Err(e) = sender.send(result) {
                                error!("{e}");
                            }
                        }
                    })
                    .expect("creating thread should not fail!");
            }
            // immediately drop the initial sender reference, only the clones inside the worker threads are kept alive
            drop(sender);
            let (mut random_count, mut from_solution_count) = (0, 0);
            let mut stats = RunStatistics::new();
            for (iteration, res) in receiver.iter().enumerate() {
                let iteration = iteration as u32 + 1; // we want to start counting from 1.. upwards
                let stats_iteration = match res {
                    Ok(work_result) => {
                        match &work_result.generation_method {
                            GenerationMethod::Random => random_count += 1,
                            GenerationMethod::FromSolution => from_solution_count += 1,
                        }
                        let percentage_range_sat = 0.20..0.7;
                        if terminate_early(
                            iteration,
                            &work_result.generation_method,
                            &stats,
                            &percentage_range_sat,
                        ) {
                            println!("Terminating early!");
                            println!(
                                "Percentage SAT Instances not in range ({}..{})",
                                percentage_range_sat.start, percentage_range_sat.end
                            );
                            cancelled.store(true, Ordering::Relaxed);
                            // We don't break here, because we want to process the remaining running threads
                            // By setting cancelled to false, we ensure threads won't start new work, but finish already running work
                        }
                        self.process_result(work_result, iteration, &mut csv_writer)
                    }
                    Err(e) => self.handle_error(*e),
                };

                stats.update(stats_iteration);

                let elapsed_since_start = self.orchestrator_start.elapsed();
                let (h, m, s) = get_hms_parts(&elapsed_since_start);
                let throughput = iteration as f64 / elapsed_since_start.as_secs_f64();
                #[rustfmt::skip]
                let body = [
                    format!("Throughput: {:.3} Iterations/s ({} threads)", throughput, self.thread_count),
                    format!("Time elapsed: {}h {}m {}s", h, m, s),
                ].join("\n");
                print_block(&format!("Iteration {iteration}"), body);
            }
            let result = &mut csv_writer.flush();
            if let Err(err) = result {
                error!("Could not flush to file\n Error: {}", err);
            }

            // print out settings
            print_block("Settings", self.config);
            // print out stats
            print_block("Run Statistics", &stats);

            // print out distribution generation mode
            print_block(
                "Distribution Generation",
                format!(
                    "{:<OFFSET$}: {}\n{:<OFFSET$}: {}",
                    "FromSolution", from_solution_count, "Random", random_count
                ),
            );

            println!();
            let duration_program = self.orchestrator_start.elapsed();
            let (h, m, s) = get_hms_parts(&duration_program);
            println!("Duration of run: {h}h, {m}m, {s}s",);

            // save important files
            if let Err(e) = self.save_settings() {
                error!("{e}");
            }

            let summary = Summary {
                meta_data,
                duration_program,
                statistics_run: stats,
            };
            let summary_file = self
                .fuzzer_output_dir
                .join(DATA_FOLDER.name)
                .join("summary.toml");
            if let Err(e) = summary.save_to_file(&summary_file) {
                error!("{e}");
            }
        });
        Ok(())
    }

    fn process_result(
        &self,
        work_result: WorkResult,
        iteration: u32,
        csv_writer: &mut Writer<File>,
    ) -> SingleRunStats {
        let seed = work_result.record.get_seed();

        let mut large_solving_difference = false;
        if let CsvRecord::Comparison(r) = &work_result.record
            && (r.problem_solving_duration_solver1_ms - r.problem_solving_duration_solver2_ms).abs()
                >= 2000.0
        {
            warn!(
                "Solving time diverged more than 2 seconds!: {}ms - {}ms",
                r.problem_solving_duration_solver1_ms, r.problem_solving_duration_solver2_ms
            );
            large_solving_difference = true;
        }
        let outcome = work_result.outcome.get_outcome();

        let outcome_folder = outcome.get_sub_folder(&self.fuzzer_output_dir);

        if !outcome_folder.exists()
            && let Err(e) = std::fs::create_dir(&outcome_folder)
        {
            error!("{e}");
        }
        match outcome {
            VerificationOutcome::Sat
            | VerificationOutcome::Unsat
            | VerificationOutcome::Timelimit => {}
            VerificationOutcome::Bug(bug_reason) => {
                info!("Found bug with seed {})!\nBug: {}", seed, bug_reason);
                let file = outcome_folder.join(format!("{}_{}_report.md", seed, iteration));
                let writer = File::create(file).map(BufWriter::new);
                match writer {
                    Ok(mut w) => {
                        if let Err(e) = create_report(&mut w, &work_result.outcome) {
                            error!("{e}")
                        }
                    }
                    Err(e) => error!("{e}"),
                }
            }
            VerificationOutcome::Unknown => {
                warn!("Solver outputted 'Unknown'");
                let file = outcome_folder.join(format!("{}_{}_report.md", seed, iteration));
                let writer = File::create(file).map(BufWriter::new);
                match writer {
                    Ok(mut w) => {
                        if let Err(e) = create_report(&mut w, &work_result.outcome) {
                            error!("{e}")
                        }
                    }
                    Err(e) => error!("{e}"),
                }
            }
        }
        let mut files_to_move = vec![];
        let output_file = work_result.pb_file;
        let new_file_location = outcome_folder.join(format!("{}_{}.opb", seed, iteration));
        files_to_move.push(MoveFile::new(output_file, new_file_location));
        if let Some(proof_file) = work_result.proof_file {
            let new_file_location_proof =
                outcome_folder.join(format!("{}_{}.pbp", seed, iteration));
            files_to_move.push(MoveFile::new(proof_file, new_file_location_proof));
        }
        let file_manipulation_result = match outcome {
            // Only save the file if bug or unknown occurred
            VerificationOutcome::Bug(_) | VerificationOutcome::Unknown => {
                move_files(&files_to_move)
            }
            _ if self.save_all_files => move_files(&files_to_move),
            // Delete the generated fuzzing result if not needed anymore
            _ => remove_files(&files_to_move).map_err(|e| e.to_string()),
        };
        if let Err(err) = file_manipulation_result {
            error!("{err}");
        }
        let duration_iteration_ms = work_result.start_worker.elapsed();
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
        let simple_result = match outcome {
            VerificationOutcome::Sat => SimpleResult::Sat,
            VerificationOutcome::Unsat => SimpleResult::Unsat,
            VerificationOutcome::Timelimit => SimpleResult::Timeout,
            VerificationOutcome::Bug(_) => SimpleResult::Bug,
            VerificationOutcome::Unknown => SimpleResult::Unknown,
        };
        SingleRunStats::Successfull(SingleRunRecord::new(
            simple_result,
            0,
            duration_iteration_ms,
            large_solving_difference,
        ))
    }

    fn save_settings(&self) -> io::Result<()> {
        let data = toml::ser::to_string_pretty(&self.config).map_err(io::Error::other)?;
        let file_name = self
            .fuzzer_output_dir
            .join(DATA_FOLDER.name)
            .join("settings.toml");
        File::create(file_name).and_then(|mut f| f.write_all(data.as_bytes()))
    }

    fn handle_error(&self, error: WorkError) -> SingleRunStats {
        error!("{error}");
        let context = &error.context;
        let folder = self
            .fuzzer_output_dir
            .join(ERROR_FOLDER.name)
            .join(error.kind.get_folder_name());
        if !folder.exists()
            && let Err(e) = std::fs::create_dir(&folder)
        {
            error!("{e}");
        }
        if let Some(pb_file) = &context.pb_file {
            let dest = folder.join(format!("{}_error.opb", context.seed));
            if let Err(e) = move_file(pb_file, &dest) {
                error!("{e}");
            }
        }
        if let Some(proof_file) = &context.proof_file {
            let dest = folder.join(format!("{}_error.pbp", context.seed));
            if let Err(e) = move_file(proof_file, &dest) {
                error!("{e}");
            }
        }
        let report_path = folder.join(format!("{}_error_report.md", context.seed));
        if let Ok(mut w) = File::create(&report_path).map(BufWriter::new) {
            let _ = writeln!(w, "Seed: {}", context.seed);
            let _ = writeln!(w, "Error: {error}");
            for (i, run) in context.solver_runs.iter().enumerate() {
                let _ = writeln!(w, "\n{:=^50}", format!("Run {}", i + 1));
                let _ = writeln!(w, "{run}");
            }
        } else {
            error!("Failed to create error report for seed {}", context.seed);
        }
        SingleRunStats::Error
    }
}

fn get_seed() -> u64 {
    rand::rng().random()
}

/// Terminate early if:
/// - random constraint generation and
/// - amount sat instances not in range and
/// - more than 1000 iterations are scheduled
fn terminate_early(
    iteration: u32,
    generation_method: &GenerationMethod,
    stats: &RunStatistics,
    percentage_range_sat: &Range<f32>,
) -> bool {
    match generation_method {
        GenerationMethod::FromSolution => false,
        GenerationMethod::Random => {
            iteration == 1001 && !stats.is_sat_percentage_in_range(percentage_range_sat)
        }
    }
}

fn create_report<W: Write>(writer: &mut W, outcome: &Outcome) -> io::Result<()> {
    let mut write_run_data = |data: &ParserRun| -> io::Result<()> {
        writeln!(writer, "{:=^50}", "Solving")?;
        writeln!(writer, "{}", data.record)?;
        writeln!(writer)?;
        writeln!(writer, "{:=^50}", "Parsing")?;
        writeln!(writer, "{}", data.parsed_status)
    };
    match outcome {
        Outcome::Comparison(comparison_run) => {
            write_run_data(&comparison_run.parser_run1)?;
            write_run_data(&comparison_run.parser_run2)?;
        }
        Outcome::Proof(proof_run) => write_run_data(&proof_run.parser_run)?,
    }
    writeln!(writer)?;
    writeln!(writer, "{:=^50}", "Outcome")?;
    writeln!(writer, "{}", outcome.get_outcome())
}

fn work_iteration(
    fuzzer_output_dir: &Path,
    settings: PbSetting,
    mode: &OrchestrationMode,
    program_info: MetaData,
    thread_id: u8,
) -> Result<WorkResult, Box<WorkError>> {
    let start_worker = Instant::now();
    let seed = get_seed();
    let file_name = format!("{thread_id}_test_{seed}");
    let output_file = fuzzer_output_dir.join(format!("{file_name}.opb"));
    debug!("Running Fuzzer with seed {seed}...");
    let mut context = WorkErrorContext::new(seed);
    let grammar = PBGenerator::create_grammar(seed, settings, program_info)
        .map_err(|kind| context.clone().into_error(kind.into()))?;
    let duration_write_to_file = File::create(&output_file)
        .map(BufWriter::new)
        .and_then(|mut buf_writer| grammar.produce(&mut buf_writer))
        .map_err(|e| context.clone().into_error(e.into()))?;

    context.pb_file = Some(output_file.clone());
    let grammar_statistics = grammar.get_statistics();
    let output_generation_duration =
        *grammar_statistics.get_generation_time() + duration_write_to_file;
    debug!(
        "Created problem in: {:.3}ms",
        (output_generation_duration.as_secs_f32() * 1000.0)
    );

    let pb_kind = settings.get_problem_kind();
    let with_option_fuzzing = rand::random_bool(0.2); // option fuzzing with 20% probability
    if with_option_fuzzing {
        info!("Using option fuzzing in this run...")
    }
    let generation_method = *settings.get_generation_method();
    match mode {
        OrchestrationMode::Comparison(p1, p2) => {
            let SolverAndParser {
                solver: s1,
                parser: p1,
            } = p1;
            let SolverAndParser {
                solver: s2,
                parser: p2,
            } = p2;
            let output_bin1 = s1
                .run(&output_file, with_option_fuzzing)
                .map_err(|e| context.clone().into_error(e.into()))?;
            let time_bin1 = output_bin1.record.execution_time;

            context.solver_runs = vec![output_bin1.record.clone()];

            let output_bin2 = s2
                .run(&output_file, with_option_fuzzing)
                .map_err(|e| context.clone().into_error(e.into()))?;
            let time_bin2 = output_bin2.record.execution_time;

            context.solver_runs = context
                .solver_runs
                .into_iter()
                .chain(vec![output_bin2.record.clone()])
                .collect();

            let start_output_extraction = Instant::now();
            let r1 = p1
                .parse(output_bin1, pb_kind)
                .map_err(|e| context.clone().into_error(e.into()))?;
            context.parser_status = Some(r1.parsed_status.clone());
            let r2 = p2
                .parse(output_bin2, pb_kind)
                .map_err(|e| context.into_error(e.into()))?;
            let conflicts1 = r1.parsed_status.get_conflicts();
            let conflicts2 = r2.parsed_status.get_conflicts();
            let comparison_run = compare_runs(r1, r2, &grammar);
            let time_output_extraction = start_output_extraction.elapsed();
            let comparison_record = ComparisonRecord {
                result: comparison_run.outcome.get_status_as_string(),
                problem_generation_ms: output_generation_duration.as_secs_f32() * 1000.0,
                problem_solving_duration_solver1_ms: time_bin1.as_secs_f32() * 1000.0,
                problem_solving_duration_solver2_ms: time_bin2.as_secs_f32() * 1000.0,
                problem_parsing_duration_ms: time_output_extraction.as_secs_f32() * 1000.0,
                conflicts1,
                conflicts2,
                seed,
            };
            Ok(WorkResult {
                start_worker,
                pb_file: output_file,
                proof_file: None,
                outcome: Outcome::Comparison(comparison_run),
                record: CsvRecord::Comparison(comparison_record),
                generation_method,
            })
        }
        OrchestrationMode::ProofChecking(solver_and_parser) => {
            let proof_file = fuzzer_output_dir.join(format!("{file_name}.pbp"));
            let problem_solving_start = Instant::now();
            let SolverAndParser { solver, parser } = solver_and_parser;
            let output = solver
                .run_with_proof(&output_file, &proof_file, with_option_fuzzing)
                .map_err(|e| context.clone().into_error(e.into()))?;
            let problem_solving_duration = problem_solving_start.elapsed();
            let context = WorkErrorContext {
                solver_runs: vec![output.record.clone()],
                proof_file: Some(proof_file.clone()),
                ..context
            };
            let start_output_parsing = Instant::now();
            let r = parser
                .parse(output, pb_kind)
                .map_err(|e| context.into_error(e.into()))?;
            let duration_output_parsing = start_output_parsing.elapsed();
            let conflicts = r.parsed_status.get_conflicts();
            let proof_checking_start = Instant::now();
            let proof_run = verify_with_proof(r, &grammar, &output_file, &proof_file);
            let proof_checking_duration = proof_checking_start.elapsed();
            let proof_record = ProofRecord {
                result: proof_run.outcome.get_status_as_string(),
                problem_generation_ms: output_generation_duration.as_secs_f32() * 1000.0,
                problem_solving_duration_program: problem_solving_duration.as_secs_f32() * 1000.0,
                problem_parsing_duration_ms: duration_output_parsing.as_secs_f32() * 1000.0,
                problem_checking_duration_ms: proof_checking_duration.as_secs_f32() * 1000.0,
                conflicts,
                seed,
            };
            Ok(WorkResult {
                start_worker,
                pb_file: output_file,
                proof_file: Some(proof_file),
                outcome: Outcome::Proof(proof_run),
                record: CsvRecord::Proof(proof_record),
                generation_method,
            })
        }
    }
}

enum Outcome {
    Comparison(ComparisonRun),
    Proof(ProofRun),
}

impl Outcome {
    fn get_outcome(&self) -> &VerificationOutcome {
        match self {
            Outcome::Comparison(comparison_run) => &comparison_run.outcome,
            Outcome::Proof(proof_run) => &proof_run.outcome,
        }
    }
}

struct WorkResult {
    start_worker: Instant,
    pb_file: PathBuf,
    proof_file: Option<PathBuf>,
    outcome: Outcome,
    record: CsvRecord,
    generation_method: GenerationMethod,
}

#[derive(Debug, Error)]
struct WorkError {
    context: WorkErrorContext,
    kind: WorkErrorKind,
}

impl Display for WorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "[seed={}] {}", self.context.seed, self.kind)
    }
}

#[derive(Debug, Clone, Default)]
struct WorkErrorContext {
    seed: u64,
    pb_file: Option<PathBuf>,
    proof_file: Option<PathBuf>,
    solver_runs: Vec<SolverExecutionRecord>,
    parser_status: Option<ParsedStatus>,
}

impl WorkErrorContext {
    fn new(seed: u64) -> Self {
        WorkErrorContext {
            seed,
            ..Default::default()
        }
    }

    fn into_error(self, kind: WorkErrorKind) -> WorkError {
        WorkError {
            context: self,
            kind,
        }
    }
}

#[derive(Debug, Error)]
enum WorkErrorKind {
    #[error("Error in fuzzer: {0}")]
    Fuzzer(#[from] PbGenerationError),
    #[error("IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Solver failed: {0}")]
    Solver(#[from] SolverExecutionFailed),
    #[error("Parser failed: {0}")]
    Parser(#[from] OutputParsingError),
}

impl WorkErrorKind {
    fn get_folder_name(&self) -> &'static str {
        match self {
            WorkErrorKind::Fuzzer(_) => "fuzzer",
            WorkErrorKind::Io(_) => "io",
            WorkErrorKind::Solver(_) => "solver",
            WorkErrorKind::Parser(_) => "parser",
        }
    }
}

/// Cancel iterations of fuzzing runs
///
/// Stop either after n iterations or after time t is passed
#[derive(Debug)]
enum StopCondition {
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
    fn new(max_iter: Option<u32>, max_duration: Option<Duration>) -> Result<Self, String> {
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

#[cfg(test)]
mod tests {

    use super::*;

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
