use std::fs::File;

use csv::Writer;

pub const ORCHESTRATOR_DATA_FILE_PREFIX: &str = "orchestrator_data";

pub enum CsvRecord {
    Comparison(ComparisonRecord),
    Proof(ProofRecord),
}

impl CsvRecord {
    pub fn serialize(
        self,
        iteration: u32,
        duration_iteration_ms: f32,
        writer: &mut Writer<File>,
    ) -> Result<(), csv::Error> {
        match self {
            CsvRecord::Comparison(comparison_record) => {
                let r = comparison_record.create_iteration_record(iteration, duration_iteration_ms);
                writer.serialize(r)
            }
            CsvRecord::Proof(proof_record) => {
                let r = proof_record.create_iteration_record(iteration, duration_iteration_ms);
                writer.serialize(r)
            }
        }
    }

    pub fn get_seed(&self) -> u64 {
        match self {
            CsvRecord::Comparison(comparison_record) => comparison_record.seed,
            CsvRecord::Proof(proof_record) => proof_record.seed,
        }
    }
}

pub struct ComparisonRecord {
    pub result: &'static str,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_solver1_ms: f32,
    pub problem_solving_duration_solver2_ms: f32,
    pub problem_parsing_duration_ms: f32,
    pub conflicts1: Option<u32>,
    pub conflicts2: Option<u32>,
    pub seed: u64,
}

impl ComparisonRecord {
    pub fn create_iteration_record(
        self,
        iteration: u32,
        duration_iteration_ms: f32,
    ) -> IterationComparisonRecord {
        IterationComparisonRecord {
            iteration,
            result: self.result,
            duration_iteration_ms,
            problem_generation_ms: self.problem_generation_ms,
            problem_solving_duration_solver1_ms: self.problem_solving_duration_solver1_ms,
            problem_solving_duration_solver2_ms: self.problem_solving_duration_solver2_ms,
            problem_parsing_duration_ms: self.problem_parsing_duration_ms,
            conflicts1: self.conflicts1,
            conflicts2: self.conflicts2,
            seed: self.seed,
        }
    }
}

#[derive(serde::Serialize)]
pub struct IterationComparisonRecord {
    pub iteration: u32,
    pub result: &'static str,
    pub duration_iteration_ms: f32,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_solver1_ms: f32,
    pub problem_solving_duration_solver2_ms: f32,
    pub problem_parsing_duration_ms: f32,
    pub conflicts1: Option<u32>,
    pub conflicts2: Option<u32>,
    pub seed: u64,
}

pub struct ProofRecord {
    pub result: &'static str,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_program: f32,
    pub problem_parsing_duration_ms: f32,
    pub problem_checking_duration_ms: f32,
    pub conflicts: Option<u32>,
    pub seed: u64,
}

impl ProofRecord {
    pub fn create_iteration_record(
        self,
        iteration: u32,
        duration_iteration_ms: f32,
    ) -> IterationProofRecord {
        IterationProofRecord {
            iteration,
            result: self.result,
            duration_iteration_ms,
            problem_generation_ms: self.problem_generation_ms,
            problem_solving_duration_program: self.problem_solving_duration_program,
            problem_parsing_duration_ms: self.problem_parsing_duration_ms,
            problem_checking_duration_ms: self.problem_checking_duration_ms,
            conflicts: self.conflicts,
            seed: self.seed,
        }
    }
}

#[derive(serde::Serialize)]
pub struct IterationProofRecord {
    pub iteration: u32,
    pub result: &'static str,
    pub duration_iteration_ms: f32,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_program: f32,
    pub problem_parsing_duration_ms: f32,
    pub problem_checking_duration_ms: f32,
    pub conflicts: Option<u32>,
    pub seed: u64,
}
