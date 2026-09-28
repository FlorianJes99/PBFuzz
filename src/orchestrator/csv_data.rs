use std::{fs::File, path::Path};

use csv::Writer;
use serde::Serialize;

pub struct OrchestratorCsvWriters {
    work_writer: Writer<File>,
    solver_writer: Writer<File>,
}

impl OrchestratorCsvWriters {
    pub fn new(data_folder: &Path) -> Result<Self, csv::Error> {
        let work_writer = csv::Writer::from_path(data_folder.join("orchestrator_data.csv"))?;
        let solver_writer = csv::Writer::from_path(data_folder.join("solver_data.csv"))?;
        Ok(Self {
            work_writer,
            solver_writer,
        })
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.solver_writer.flush()?;
        self.work_writer.flush()
    }
}

pub struct CsvRecordsSingleIteration {
    records: Vec<SolverRecord>,
    work_record: WorkRecord,
}

impl CsvRecordsSingleIteration {
    pub fn new(records: Vec<SolverRecord>, work_record: WorkRecord) -> Self {
        Self {
            records,
            work_record,
        }
    }

    pub fn get_records(&self) -> &[SolverRecord] {
        &self.records
    }

    pub fn serialize(
        &self,
        iteration: u32,
        duration_iteration_ms: f32,
        writers: &mut OrchestratorCsvWriters,
    ) -> Result<(), csv::Error> {
        for cr in &self.records {
            let r = cr.create_iteration_record(iteration);
            writers.solver_writer.serialize(r)?;
        }
        let r = self
            .work_record
            .crete_iteration_record(iteration, duration_iteration_ms);
        writers.work_writer.serialize(r)
    }
}

pub struct SolverRecord {
    pub solver_name: String,
    pub solver_status: &'static str,
    pub found_optimum: Option<i32>,
    pub model_verified: Option<bool>,
    pub proof_verified: Option<bool>,
    pub term_signal: Option<i32>,
    pub solver_conflicts: Option<u32>,
    pub problem_solving_duration_ms: Option<f32>,
    pub problem_parsing_duration_ms: Option<f32>,
    pub proof_verification_time_ms: Option<f32>,
}

impl SolverRecord {
    pub fn create_iteration_record(&self, iteration: u32) -> IterationSolverRecord {
        IterationSolverRecord {
            iteration,
            solver_name: self.solver_name.clone(),
            solver_status: self.solver_status,
            found_optimum: self.found_optimum,
            model_verified: self.model_verified,
            proof_verified: self.proof_verified,
            term_signal: self.term_signal,
            solver_conflicts: self.solver_conflicts,
            problem_solving_duration_ms: self.problem_solving_duration_ms,
            problem_parsing_duration_ms: self.problem_parsing_duration_ms,
            proof_verification_time_ms: self.proof_verification_time_ms,
        }
    }

    pub fn create_not_parseable_run(
        solver_name: String,
        solver_status: &'static str,
        term_signal: Option<i32>,
        problem_solving_duration_ms: Option<f32>,
    ) -> Self {
        Self {
            solver_name,
            solver_status,
            term_signal,
            problem_solving_duration_ms,
            found_optimum: None,
            model_verified: None,
            proof_verified: None,
            problem_parsing_duration_ms: None,
            solver_conflicts: None,
            proof_verification_time_ms: None,
        }
    }
}

pub struct WorkRecord {
    pub seed: u64,
    pub generation_method: String,
    pub with_option_fuzzing: bool,
    pub had_errors: bool,
    pub problem_generation_ms: f32,
    pub result: String,
}

impl WorkRecord {
    pub fn new(
        seed: u64,
        generation_method: String,
        problem_generation_ms: f32,
        with_option_fuzzing: bool,
        had_errors: bool,
        result: String,
    ) -> Self {
        Self {
            seed,
            generation_method,
            problem_generation_ms,
            with_option_fuzzing,
            had_errors,
            result,
        }
    }

    pub fn crete_iteration_record(
        &self,
        iteration: u32,
        duration_iteration_ms: f32,
    ) -> IterationWorkRecord {
        IterationWorkRecord {
            iteration,
            seed: self.seed,
            generation_method: self.generation_method.clone(),
            problem_generation_ms: self.problem_generation_ms,
            with_option_fuzzing: self.with_option_fuzzing,
            had_errors: self.had_errors,
            duration_iteration_ms,
            result: self.result.clone(),
        }
    }
}

#[derive(Serialize)]
pub struct IterationWorkRecord {
    iteration: u32,
    seed: u64,
    generation_method: String,
    with_option_fuzzing: bool,
    had_errors: bool,
    problem_generation_ms: f32,
    duration_iteration_ms: f32,
    result: String,
}

#[derive(Serialize)]
pub struct IterationSolverRecord {
    iteration: u32,
    solver_name: String,
    solver_status: &'static str,
    found_optimum: Option<i32>,
    model_verified: Option<bool>,
    proof_verified: Option<bool>,
    term_signal: Option<i32>,
    solver_conflicts: Option<u32>,
    problem_solving_duration_ms: Option<f32>,
    problem_parsing_duration_ms: Option<f32>,
    proof_verification_time_ms: Option<f32>,
}
