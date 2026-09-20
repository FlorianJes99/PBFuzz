use std::fs::File;

use csv::Writer;

pub const ORCHESTRATOR_DATA_FILE_PREFIX: &str = "orchestrator_data";

pub struct CsvRecordsSingleIteration {
    records: Vec<SolverRecord>,
}

impl CsvRecordsSingleIteration {
    pub fn new(records: Vec<SolverRecord>) -> Self {
        Self { records }
    }

    pub fn get_records(&self) -> &[SolverRecord] {
        &self.records
    }

    pub fn serialize(
        &self,
        iteration: u32,
        duration_iteration_ms: f32,
        writer: &mut Writer<File>,
    ) -> Result<(), csv::Error> {
        for cr in &self.records {
            let r = cr.create_iteration_record(iteration, duration_iteration_ms);
            writer.serialize(r)?;
        }
        Ok(())
    }
}

pub struct SolverRecord {
    pub result: &'static str,
    pub problem_generation_ms: f32,
    pub solver_name: String,
    pub problem_solving_duration_solver_ms: f32,
    pub problem_parsing_duration_ms: f32,
    pub proof_verification_time_ms: Option<f32>,
    pub conflicts: Option<u32>,
    pub seed: u64,
}

impl SolverRecord {
    pub fn create_iteration_record(
        &self,
        iteration: u32,
        duration_iteration_ms: f32,
    ) -> IterationSolverRecord {
        IterationSolverRecord {
            iteration,
            result: self.result,
            duration_iteration_ms,
            problem_generation_ms: self.problem_generation_ms,
            solver_name: self.solver_name.clone(),
            problem_solving_duration_solver_ms: self.problem_solving_duration_solver_ms,
            problem_parsing_duration_ms: self.problem_parsing_duration_ms,
            conflicts_solver: self.conflicts,
            seed: self.seed,
            proof_verification_time_ms: self.proof_verification_time_ms,
        }
    }
}

#[derive(serde::Serialize)]
pub struct IterationSolverRecord {
    pub iteration: u32,
    pub solver_name: String,
    pub result: &'static str,
    pub duration_iteration_ms: f32,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_solver_ms: f32,
    pub problem_parsing_duration_ms: f32,
    pub proof_verification_time_ms: Option<f32>,
    pub conflicts_solver: Option<u32>,
    pub seed: u64,
}
