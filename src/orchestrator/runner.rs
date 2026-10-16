use crate::fuzzer::app_info::MetaData;
use crate::fuzzer::grammar::PBGrammar;
use crate::fuzzer::pb_generator::PbGenerationError;
use crate::fuzzer::pb_settings::{GenerationMethod, PbConfigFile, PbSetting};
use crate::orchestrator::csv_data::{
    CsvRecordsSingleIteration, OrchestratorCsvWriters, WorkRecord,
};
use crate::orchestrator::dashboard::{Dashboard, IterationInfo};
use crate::orchestrator::evaluation::common::{BugReason, StatusFlat, verify_solution};
use crate::orchestrator::evaluation::comparison::compare_status;
use crate::orchestrator::evaluation::proof_checking::check_proof;
use crate::orchestrator::issue_report::case_writer::{Artifact, CaseWriter, IssueReport};
use crate::orchestrator::issue_report::issue_record::{IssueRecord, SolverInfo};
use crate::orchestrator::orchestrator_args::ProofMode;
use crate::orchestrator::parsing::parser::{
    OutputParsingError, ParsedStatus, ParserImpl, ParserRun,
};
use crate::orchestrator::run_statistics::{
    RunStatistics, SimpleResult, SingleRunRecord, SingleRunStats, SolverStateAfterIteration,
};
use crate::orchestrator::solving::solver::{
    SolverExecutionFailed, SolverExecutionRecord, SolverOutput, SolverRun,
};
use crate::orchestrator::solving::solver_config::SolverConfig;
use crate::orchestrator::{
    csv_data::SolverRecord,
    folders::{DATA_FOLDER, prepare_sub_folders},
    orchestrator_args::OrchestratorArgs,
    solving::solver::SolverData,
};
use std::borrow::Cow;
use std::io::{self};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::{
    error::Error,
    fmt::Display,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
    sync::mpsc::channel,
    thread,
    time::{Duration, Instant},
};

use indicatif::MultiProgress;
use itertools::Itertools;
use log::{debug, error, info, warn};
use rand::{self, RngExt};
use serde::Serialize;
use thiserror::Error;

use crate::fuzzer::pb_generator::PBGenerator;

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
    mp: MultiProgress,
) -> Result<(), Box<dyn Error>> {
    let thread_count = args.thread_count.unwrap_or(1);
    info!(
        "Using {} threads for computing fuzzing runs in parallel",
        thread_count
    );
    let timelimit = Duration::from_secs(args.timeout_external_program_secs);
    let mut solvers = vec![];
    let mut one_solver_enabled = false;
    for config_file in &args.solver_configs {
        let config = SolverConfig::load_from_file(config_file)?;
        let solver = match args.proofs {
            ProofMode::All => SolverAndParser::from_config(config, timelimit, true),
            // We want to enable the first solver in the list which supports proofs
            ProofMode::One if !one_solver_enabled && config.proof_arg.is_some() => {
                one_solver_enabled = true;
                SolverAndParser::from_config(config, timelimit, true)
            }
            ProofMode::One | ProofMode::None => {
                SolverAndParser::from_config(config, timelimit, false)
            }
        };
        solvers.push(solver);
    }
    if solvers.is_empty() {
        return Err("No valid solver config provided!".into());
    }

    if solvers.len() == 1 && !solvers[0].solver.supports_proofs() {
        return Err("Only one solver specified, but it does not support proofs!".into());
    }

    if solvers.len() > 1 && args.no_comparison {
        if !matches!(args.proofs, ProofMode::All) {
            return Err("Misconfiguration: Provided amount of solvers is > 1, but no comparison mode and no 'All' Proof mode".into());
        }
        if solvers.iter().any(|sp| !sp.solver.supports_proofs()) {
            return Err("Misconfiguration: Not all provided solvers support Proofs but comparison mode is deactivated".into());
        }
    }
    let config = PbConfigFile::load_from_file(&args.settings_file)?;
    let stop_condition = StopCondition::new(
        args.iterations,
        args.timeout_min.map(|v| Duration::from_mins(v as u64)),
    )?;
    info!("Workspace set to: {}", run_root.display());
    prepare_sub_folders(&run_root)?;

    let index_file = run_root.join(DATA_FOLDER.name).join("cases.jsonl");
    let case_writer = CaseWriter::new(&run_root, index_file);
    let option_fuzzing_perc = if (0.0..=1.0).contains(&args.option_fuzzing_perc) {
        args.option_fuzzing_perc
    } else {
        return Err(format!(
            "Invalid value for option fuzzing, expted value between 0 and 1 but was: {}",
            args.option_fuzzing_perc
        ))?;
    };
    let harness_config = HarnessConfig {
        with_comparison: !args.no_comparison,
        save_all_files: args.save_all_files,
        option_fuzzing_perc,
    };
    let solver_names: Vec<_> = solvers.iter().map(|s| s.solver.get_name()).collect();
    Orchestrator {
        fuzzer_output_dir: run_root,
        args,
        config,
        orchestrator_start: Instant::now(),
        dashboard: Dashboard::new(mp, meta_data, &solver_names),
        solvers,
        thread_count,
        case_writer,
        harness_config,
    }
    .run(stop_condition, meta_data)
}

struct SolverAndParser {
    solver: SolverData,
    parser: ParserImpl,
}

impl SolverAndParser {
    fn from_config(config: SolverConfig, timelimit: Duration, proof_enabled: bool) -> Self {
        let parser = ParserImpl::from_kind(&config.parser_kind);
        Self {
            solver: SolverData::into_solver(config, timelimit, proof_enabled),
            parser,
        }
    }
}

struct HarnessConfig {
    with_comparison: bool,
    option_fuzzing_perc: f64,
    save_all_files: bool,
}

struct Orchestrator {
    fuzzer_output_dir: PathBuf,
    args: OrchestratorArgs,
    config: PbConfigFile,
    harness_config: HarnessConfig,
    orchestrator_start: Instant,
    solvers: Vec<SolverAndParser>,
    thread_count: u8,
    case_writer: CaseWriter,
    dashboard: Dashboard,
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
        let data_folder = self.fuzzer_output_dir.join(DATA_FOLDER.name);
        let mut csv_writers = OrchestratorCsvWriters::new(&data_folder)?;
        let stop_condtion = Arc::new(stop_condition);
        let cancelled = Arc::new(AtomicBool::new(false));
        let ctrc_flag = cancelled.clone();
        ctrlc::set_handler(move || {
            ctrc_flag.store(true, Ordering::Relaxed);
        })?;
        let count_runs = Arc::new(AtomicU32::new(0));
        let (sender, receiver) = channel::<WorkResult>();
        let start = Instant::now();
        thread::scope(|s| {
            let handles: Vec<_> = (1..=self.thread_count)
                .map(|thread_id| {
                    let sender = sender.clone();
                    let cancelled = Arc::clone(&cancelled);
                    let count_runs = Arc::clone(&count_runs);
                    let stop_condition = Arc::clone(&stop_condtion);
                    thread::Builder::new()
                        .name(format!("thread-{thread_id}"))
                        .spawn_scoped(s, move || {
                            while !cancelled.load(Ordering::Relaxed) {
                                let iteration = count_runs.fetch_add(1, Ordering::Relaxed);
                                if stop_condition.is_done(iteration, start) {
                                    break;
                                }
                                let result = work_iteration(
                                    &self.fuzzer_output_dir,
                                    self.config.generate_settings(),
                                    self.harness_config.with_comparison,
                                    self.harness_config.option_fuzzing_perc,
                                    &self.solvers,
                                    meta_data,
                                    thread_id,
                                );
                                if let Err(e) = sender.send(result) {
                                    error!("{e}");
                                }
                            }
                        })
                        .expect("creating thread should not fail!")
                })
                .collect();
            // immediately drop the initial sender reference, only the clones inside the worker threads are kept alive
            drop(sender);
            let (mut random_count, mut from_solution_count) = (0, 0);
            let mut stats = RunStatistics::new();
            let mut processed_results = 0u32;
            loop {
                match receiver.recv_timeout(Duration::from_millis(500)) {
                    Ok(res) => {
                        match &res.meta.generation_method {
                            GenerationMethod::Random => random_count += 1,
                            GenerationMethod::FromSolution => from_solution_count += 1,
                        }
                        let iteration = processed_results + 1;
                        let stats_iteration = self.process_result(res, iteration, &mut csv_writers);

                        stats.update(stats_iteration);

                        processed_results += 1;
                    }
                    Err(RecvTimeoutError::Timeout) => {} // nothing to do, just update the dashboard
                    Err(RecvTimeoutError::Disconnected) => break,
                }

                let solver_stats = stats.get_solver_stats();
                let iteration_info = IterationInfo {
                    total: processed_results,
                    amount_random: random_count,
                    amount_planted: from_solution_count,
                };
                let workers_active = handles.iter().filter(|h| !h.is_finished()).count();
                let user_interrupted = cancelled.load(Ordering::Relaxed);
                self.dashboard.update(
                    &self.orchestrator_start.elapsed(),
                    &iteration_info,
                    workers_active as u8,
                    user_interrupted,
                    solver_stats,
                );
            }
            self.dashboard.finish(&self.orchestrator_start.elapsed());
            let result = &mut csv_writers.flush();
            if let Err(err) = result {
                error!("Could not flush to file\n Error: {}", err);
            }

            let duration_program = self.orchestrator_start.elapsed();

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

            let used_args_file = self
                .fuzzer_output_dir
                .join(DATA_FOLDER.name)
                .join("used_args.toml");
            match toml::ser::to_string_pretty(&self.args) {
                Ok(data) => {
                    if let Err(e) =
                        File::create(used_args_file).and_then(|mut f| f.write_all(data.as_bytes()))
                    {
                        error!("{e}")
                    }
                }
                Err(e) => error!("{e}"),
            }
        });
        Ok(())
    }

    fn process_result(
        &self,
        work_result: WorkResult,
        iteration: u32,
        csv_writers: &mut OrchestratorCsvWriters,
    ) -> SingleRunStats {
        let seed = work_result.meta.seed;

        let issue_record = IssueRecord {
            iteration,
            seed,
            generation_method: work_result.meta.generation_method,
            status: work_result.payload.get_status_as_string(),
            with_option_fuzzing: work_result.meta.with_option_fuzzing,
            solver_infos: work_result.get_execution_records(),
        };

        match &work_result.payload {
            WorkPayload::Verdict(solver_verdict) => {
                let mut large_solving_difference = false;
                let rs = solver_verdict.records.get_records();
                let min = rs
                    .iter()
                    .filter_map(|r| r.problem_solving_duration_ms)
                    .fold(f32::INFINITY, f32::min);
                let max = rs
                    .iter()
                    .filter_map(|r| r.problem_solving_duration_ms)
                    .fold(f32::NEG_INFINITY, f32::max);
                if max * 0.01 > min {
                    warn!(
                        "Large solving time discrepancy!: fastest {}ms, slowest {}ms",
                        min, max
                    );
                    large_solving_difference = true;
                }

                let iteration_result = &solver_verdict.outcome;

                let mut files_to_move: Vec<&Path> = vec![];
                let output_file = &solver_verdict.pb_file;
                files_to_move.push(output_file);
                let proof_files = &solver_verdict.proof_files;
                if !proof_files.is_empty() {
                    proof_files.iter().for_each(|p| files_to_move.push(p));
                }

                let should_process =
                    self.harness_config.save_all_files || iteration_result.should_process_data();
                if !iteration_result.errors.is_empty() {
                    info!("Found bug(s) with seed {})!", seed,);
                }
                if should_process {
                    if let Err(e) =
                        self.case_writer
                            .process(&issue_record, &work_result, &files_to_move)
                    {
                        error!("{e}");
                    }
                } else {
                    for file in files_to_move {
                        if file.exists()
                            && let Err(e) = std::fs::remove_file(file)
                        {
                            error! {"{e}"};
                        }
                    }
                }
                let duration_iteration_ms = work_result.meta.start_worker.elapsed();
                let result = solver_verdict.records.serialize(
                    iteration,
                    duration_iteration_ms.as_secs_f32() * 1000.0,
                    csv_writers,
                );
                if let Err(err) = result {
                    error!("Could not write record into csv\nError: {}", err);
                }
                if iteration.is_multiple_of(1000) {
                    // flush every 1000 iterations to file
                    let result = csv_writers.flush();
                    if let Err(err) = result {
                        error!("Could not flush to file\n Error: {}", err);
                    }
                }
                let simple_result = iteration_result.get_simple_result();
                let conflicts = iteration_result
                    .successfull_runs
                    .iter()
                    .filter_map(|r| r.parsed_status.get_conflicts())
                    .collect();
                let mut solver_states: Vec<(String, SolverStateAfterIteration)> = iteration_result
                    .successfull_runs
                    .iter()
                    .map(|run| {
                        if run.parsed_status.did_not_timeout() {
                            (
                                run.record.name.to_string(),
                                SolverStateAfterIteration::Expected,
                            )
                        } else {
                            (
                                run.record.name.to_string(),
                                SolverStateAfterIteration::Timeout,
                            )
                        }
                    })
                    .collect();
                for (name_opt, _) in &iteration_result.errors {
                    if let Some(name) = name_opt {
                        solver_states.push((name.clone(), SolverStateAfterIteration::Failure));
                    }
                }
                SingleRunStats::Successfull(SingleRunRecord::new(
                    simple_result,
                    conflicts,
                    duration_iteration_ms,
                    large_solving_difference,
                    solver_states,
                ))
            }
            WorkPayload::PbGenerationError(work_error) => {
                error!("{}", work_error.kind);
                let files_to_move: Vec<&Path> = vec![&work_error.pb_file];
                if let Err(e) =
                    self.case_writer
                        .process(&issue_record, &work_result, &files_to_move)
                {
                    error!("{e}");
                }
                SingleRunStats::Error
            }
        }
    }

    fn save_settings(&self) -> io::Result<()> {
        let data = toml::ser::to_string_pretty(&self.config).map_err(io::Error::other)?;
        let file_name = self
            .fuzzer_output_dir
            .join(DATA_FOLDER.name)
            .join("settings.toml");
        File::create(file_name).and_then(|mut f| f.write_all(data.as_bytes()))
    }
}

fn get_seed() -> u64 {
    rand::rng().random()
}

fn work_iteration(
    fuzzer_output_dir: &Path,
    settings: PbSetting,
    with_comparison: bool,
    option_fuzzing_perc: f64,
    sps: &[SolverAndParser],
    program_info: MetaData,
    thread_id: u8,
) -> WorkResult {
    let start_worker = Instant::now();
    let seed = get_seed();
    let with_option_fuzzing = rand::random_bool(option_fuzzing_perc); // option fuzzing with 20% probability
    if with_option_fuzzing {
        info!("Using option fuzzing in this run...")
    }
    let generation_method = *settings.get_generation_method();

    let meta = WorkMetaData {
        start_worker,
        seed,
        generation_method,
        with_option_fuzzing,
    };
    let file_name = format!("{thread_id}_test_{seed}");
    let output_file = fuzzer_output_dir.join(format!("{file_name}.opb"));
    debug!("Running Fuzzer with seed {seed}...");

    let grammar = match PBGenerator::create_grammar(seed, settings, program_info) {
        Ok(g) => g,
        Err(e) => {
            return WorkResult {
                meta,
                payload: WorkPayload::PbGenerationError(FuzzGenerationError {
                    pb_file: output_file,
                    kind: e.into(),
                }),
            };
        }
    };

    let duration_write_to_file = match File::create(&output_file)
        .map(BufWriter::new)
        .and_then(|mut buf_writer| grammar.produce(&mut buf_writer))
    {
        Ok(d) => d,
        Err(e) => {
            return WorkResult {
                meta,
                payload: WorkPayload::PbGenerationError(FuzzGenerationError {
                    pb_file: output_file,
                    kind: e.into(),
                }),
            };
        }
    };

    let grammar_statistics = grammar.get_statistics();
    let output_generation_duration =
        *grammar_statistics.get_generation_time() + duration_write_to_file;
    debug!(
        "Created problem in: {:.3}ms",
        (output_generation_duration.as_secs_f32() * 1000.0)
    );

    let pb_kind = settings.get_problem_kind();

    let mut comparison_records = Vec::with_capacity(sps.len());
    let mut valid_runs = Vec::with_capacity(sps.len());
    let mut errors = vec![];
    let mut proof_files = vec![];
    let mut solver_outputs = vec![];
    for sp in sps {
        let SolverAndParser {
            solver: s,
            parser: p,
        } = sp;
        let proof_file = fuzzer_output_dir.join(format!("{}_{file_name}.pbp", s.get_name()));
        let SolverRun {
            record,
            solver_output,
            state,
        } = match s.run(&output_file, &proof_file, with_option_fuzzing) {
            Ok(o) => o,
            Err(e) => {
                let work_error = WorkError {
                    solver_run: None,
                    kind: e.into(),
                };
                errors.push((Some(s.get_name().to_string()), work_error));
                let comparison_record = SolverRecord::create_not_parseable_run(
                    s.get_name().to_string(),
                    "run_error",
                    None,
                    None,
                );
                comparison_records.push(comparison_record);
                continue;
            }
        };
        proof_files.push(proof_file.clone());
        let time_bin = record.execution_time;
        let start_time_parsing = Instant::now();
        let execution_record = record.clone();
        let r = match p.parse(state, record, &solver_output.stdout, pb_kind) {
            Ok(r) => r,
            Err(e) => {
                solver_outputs.push((s.get_name().to_string(), solver_output));
                let run_record = SolverRunRecord {
                    execution: execution_record.clone(),
                    parsed_status: None,
                };
                let work_error = WorkError {
                    solver_run: Some(run_record),
                    kind: e.into(),
                };
                errors.push((Some(s.get_name().to_string()), work_error));
                let comparison_record = SolverRecord::create_not_parseable_run(
                    s.get_name().to_string(),
                    "parse_error",
                    execution_record.signal,
                    Some(time_bin.as_secs_f32() * 1000.0),
                );
                comparison_records.push(comparison_record);
                continue;
            }
        };
        solver_outputs.push((s.get_name().to_string(), solver_output));
        let time_parsing = start_time_parsing.elapsed();
        let conflicts = r.parsed_status.get_conflicts();
        let solver_name = r.record.name.clone();
        let term_signal = r.record.signal;
        let minimum = r
            .parsed_status
            .get_min_objective()
            .map(|min| *min.get_value());
        let ValidationResult {
            claimed_status,
            verification_time,
            model_verified,
            proof_verified,
            error_kind,
        } = validate_parsed_status_of_solver(
            &r.parsed_status,
            &grammar,
            s.supports_proofs(),
            &output_file,
            &proof_file,
        );
        if let Some(e) = error_kind {
            let run_record = SolverRunRecord {
                execution: execution_record.clone(),
                parsed_status: Some(r.parsed_status.clone()),
            };
            let work_error = WorkError {
                solver_run: Some(run_record),
                kind: e.into(),
            };
            errors.push((Some(r.record.name.to_string()), work_error));
        } else {
            valid_runs.push(r);
        }
        let comparison_record = SolverRecord {
            solver_name,
            solver_conflicts: conflicts,
            solver_status: claimed_status,
            found_optimum: minimum,
            model_verified,
            proof_verified,
            term_signal,
            problem_solving_duration_ms: Some(time_bin.as_secs_f32() * 1000.0),
            problem_parsing_duration_ms: Some(time_parsing.as_secs_f32() * 1000.0),
            proof_verification_time_ms: verification_time.map(|t| t.as_secs_f32() * 1000.0),
        };
        comparison_records.push(comparison_record);
    }

    let runs_without_timeout: Vec<_> = valid_runs
        .iter()
        .filter(|r| r.parsed_status.did_not_timeout())
        .collect();
    // compare all solvers if enabled
    if with_comparison && let Err(b) = compare_status(&runs_without_timeout) {
        let work_error = WorkError {
            solver_run: None,
            kind: b.into(),
        };
        errors.push((None, work_error));
    };
    let status = runs_without_timeout
        .first()
        .map(|r| StatusFlat::from_parsed_status(&r.parsed_status))
        .or(valid_runs
            .first()
            .map(|r| StatusFlat::from_parsed_status(&r.parsed_status)));
    let had_errors = !errors.is_empty();
    let iteration_result = IterationResult {
        errors,
        successfull_runs: valid_runs,
        status,
        solver_outputs,
    };
    let work_record = WorkRecord::new(
        seed,
        generation_method.to_string(),
        duration_write_to_file.as_secs_f32() * 1000.0,
        with_option_fuzzing,
        had_errors,
        iteration_result.get_status_as_string(),
    );
    let verdict = SolverVerdict {
        pb_file: output_file,
        proof_files,
        outcome: iteration_result,
        records: CsvRecordsSingleIteration::new(comparison_records, work_record),
    };

    let payload = WorkPayload::Verdict(Box::new(verdict));

    WorkResult { meta, payload }
}

struct ValidationResult {
    claimed_status: &'static str,
    verification_time: Option<Duration>,
    model_verified: Option<bool>,
    proof_verified: Option<bool>,
    error_kind: Option<BugReason>,
}

impl ValidationResult {
    fn new(claimed_status: &'static str) -> Self {
        Self {
            claimed_status,
            verification_time: None,
            model_verified: None,
            proof_verified: None,
            error_kind: None,
        }
    }
}

fn validate_parsed_status_of_solver(
    parsed_status: &ParsedStatus,
    grammar: &PBGrammar,
    supports_proof: bool,
    output_file: &Path,
    proof_file: &Path,
) -> ValidationResult {
    let parsed_status_as_str = parsed_status.get_status_str();
    let has_model = parsed_status.has_model();
    let mut res = ValidationResult::new(parsed_status_as_str);
    if has_model {
        match verify_solution(parsed_status, grammar) {
            Ok(_) => {
                res.model_verified = Some(true);
            }
            Err(e) => {
                res.model_verified = Some(false);
                res.error_kind = Some(e.into());
                return res;
            }
        }
    }

    let needs_proof_check = has_model || matches!(parsed_status, ParsedStatus::Unsat { .. });
    if supports_proof && needs_proof_check {
        let start = Instant::now();
        let r = check_proof(output_file, proof_file);
        res.verification_time = Some(start.elapsed());
        res.proof_verified = Some(r.is_ok());
        if let Err(e) = r {
            res.error_kind = Some(e.into());
        }
    }
    res
}

struct IterationResult {
    successfull_runs: Vec<ParserRun>,
    errors: Vec<(Option<String>, WorkError)>,
    status: Option<StatusFlat>,
    solver_outputs: Vec<(String, SolverOutput)>,
}

impl IterationResult {
    fn get_solver_infos(&self) -> Vec<SolverInfo> {
        self.successfull_runs
            .iter()
            .map(convert_to_solver_info)
            .collect()
    }

    fn get_status_as_string(&self) -> String {
        if self.errors.is_empty() {
            self.status
                .as_ref()
                .map(|s| s.get_status_as_str())
                .unwrap_or("unexpected_failure")
                .to_string()
        } else {
            self.errors
                .iter()
                .map(|(_, e)| &e.kind)
                .map(|kind| kind.classify_error())
                .sorted()
                .join(", ")
        }
    }

    fn should_process_data(&self) -> bool {
        if !self.errors.is_empty() {
            return true;
        }
        match &self.status {
            Some(s) => match s {
                StatusFlat::Sat => false,
                StatusFlat::Optimum => false,
                StatusFlat::Unsat => false,
                StatusFlat::TimeLimit => false,
                StatusFlat::Unknown => true,
            },
            None => true,
        }
    }

    fn get_simple_result(&self) -> SimpleResult {
        match self.errors.len() {
            0 => match self.status.as_ref() {
                Some(s) => match s {
                    StatusFlat::Sat => SimpleResult::Sat,
                    StatusFlat::Optimum => SimpleResult::Optimum,
                    StatusFlat::Unsat => SimpleResult::Unsat,
                    StatusFlat::TimeLimit => SimpleResult::Timeout,
                    StatusFlat::Unknown => SimpleResult::Unknown,
                },
                None => SimpleResult::Other,
            },
            n => SimpleResult::Errors(n as u32),
        }
    }
}

fn convert_to_solver_info(parser_run: &ParserRun) -> SolverInfo {
    SolverInfo {
        binary: parser_run.record.binary.clone(),
        used_args: parser_run.record.used_args.clone(),
        parsed_status: parser_run.parsed_status.get_status_str().to_string(),
    }
}

struct WorkResult {
    meta: WorkMetaData,
    payload: WorkPayload,
}

impl WorkResult {
    fn get_execution_records(&self) -> Vec<SolverInfo> {
        match &self.payload {
            WorkPayload::Verdict(solver_verdict) => solver_verdict.outcome.get_solver_infos(),
            WorkPayload::PbGenerationError(_) => vec![],
        }
    }
}

impl IssueReport for WorkResult {
    fn create_report<W: Write>(&self, w: &mut W) -> io::Result<()> {
        let WorkMetaData {
            start_worker: _,
            seed,
            generation_method,
            with_option_fuzzing,
        } = &self.meta;
        writeln!(w, "Seed: {seed}")?;
        writeln!(w, "Generation Method: {generation_method}")?;
        writeln!(w, "Option fuzzing: {with_option_fuzzing}")?;
        writeln!(w)?;
        match &self.payload {
            WorkPayload::Verdict(solver_verdict) => {
                let IterationResult {
                    errors,
                    successfull_runs,
                    status,
                    solver_outputs: _,
                } = &solver_verdict.outcome;
                writeln!(w, "{:=^50}", "Errors")?;
                for (solver_name, e) in errors {
                    match solver_name {
                        Some(name) => writeln!(w, "Solver {name}: {}", e.kind)?,
                        None => writeln!(w, "Other error: {}", e.kind)?,
                    }
                    if let Some(solver_run) = &e.solver_run {
                        let SolverRunRecord {
                            execution,
                            parsed_status,
                        } = solver_run;
                        writeln!(w, "{:-^50}", "Solving")?;
                        writeln!(w, "{}", execution)?;
                        if let Some(status) = parsed_status {
                            writeln!(w)?;
                            writeln!(w, "{:-^50}", "Parsing")?;
                            writeln!(w, "{}", status)?;
                        }
                    }
                }
                writeln!(w)?;
                let mut write_run_data = |data: &ParserRun| -> io::Result<()> {
                    writeln!(w, "{:=^50}", "Solving")?;
                    writeln!(w, "{}", data.record)?;
                    writeln!(w)?;
                    writeln!(w, "{:=^50}", "Parsing")?;
                    writeln!(w, "{}", data.parsed_status)
                };
                for r in successfull_runs {
                    write_run_data(r)?;
                }
                writeln!(w)?;
                writeln!(w, "{:=^50}", "Outcome")?;
                writeln!(w, "Successfull runs: {}", successfull_runs.len())?;
                writeln!(w, "Errors: {}", errors.len())?;
                if let Some(s) = status {
                    writeln!(w, "Status {}", s.get_status_as_str())?;
                }
                Ok(())
            }

            WorkPayload::PbGenerationError(error) => {
                writeln!(w, "Error: {}", error.kind)
            }
        }
    }

    fn artifacts(&self) -> Vec<Artifact<'_>> {
        match &self.payload {
            WorkPayload::Verdict(solver_verdict) => solver_verdict
                .outcome
                .solver_outputs
                .iter()
                .map(|(name, out)| Artifact {
                    file_name: format!("output_{}.txt", name),
                    contents: Cow::Owned(out.to_combined()),
                })
                .collect(),
            WorkPayload::PbGenerationError(_) => vec![],
        }
    }
}

struct WorkMetaData {
    start_worker: Instant,
    seed: u64,
    generation_method: GenerationMethod,
    with_option_fuzzing: bool,
}

enum WorkPayload {
    Verdict(Box<SolverVerdict>),
    PbGenerationError(FuzzGenerationError),
}
impl WorkPayload {
    fn get_status_as_string(&self) -> String {
        match self {
            WorkPayload::Verdict(solver_verdict) => solver_verdict.outcome.get_status_as_string(),
            WorkPayload::PbGenerationError(_) => "pb file generation error".to_string(),
        }
    }
}

struct SolverVerdict {
    pb_file: PathBuf,
    proof_files: Vec<PathBuf>,
    outcome: IterationResult,
    records: CsvRecordsSingleIteration,
}

#[derive(Debug, Error)]
struct WorkError {
    solver_run: Option<SolverRunRecord>,
    kind: WorkErrorKind,
}

impl Display for WorkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "WorkError - Kind: {}", self.kind)
    }
}

#[derive(Debug, Clone)]
struct SolverRunRecord {
    execution: SolverExecutionRecord,
    parsed_status: Option<ParsedStatus>,
}

struct FuzzGenerationError {
    pb_file: PathBuf,
    kind: FuzzGenerationErrorKind,
}

#[derive(Debug, Error)]
enum FuzzGenerationErrorKind {
    #[error("Error in fuzzer: {0}")]
    Fuzzer(#[from] PbGenerationError),
    #[error("IO failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
enum WorkErrorKind {
    #[error("Solver failed: {0}")]
    Solver(#[from] SolverExecutionFailed),
    #[error("Parser failed: {0}")]
    Parser(#[from] OutputParsingError),
    #[error("Bug found: {0}")]
    Bug(#[from] BugReason),
}

impl WorkErrorKind {
    fn classify_error(&self) -> &'static str {
        match self {
            WorkErrorKind::Solver(_) => "solver_failed",
            WorkErrorKind::Parser(_) => "parser_failed",
            WorkErrorKind::Bug(bug_reason) => bug_reason.classify_bug(),
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
