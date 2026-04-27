pub const ORCHESTRATOR_DATA_FILE_PREFIX: &str = "orchestrator_data";

pub struct ComparisonRecord {
    pub result: &'static str,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_roundingsat_ms: f32,
    pub problem_solving_duration_scip_ms: f32,
    pub problem_parsing_duration_ms: f32,
    pub conflicts: u32,
    pub seed: u64,
}

impl ComparisonRecord {
    pub fn create_iteration_record(
        self,
        iteration: u32,
        duration_iteration_ms: f32,
    ) -> IterationRecord {
        IterationRecord {
            iteration,
            result: self.result,
            duration_iteration_ms,
            problem_generation_ms: self.problem_generation_ms,
            problem_solving_duration_roundingsat_ms: self.problem_solving_duration_roundingsat_ms,
            problem_solving_duration_scip_ms: self.problem_solving_duration_scip_ms,
            problem_parsing_duration_ms: self.problem_parsing_duration_ms,
            conflicts: self.conflicts,
            seed: self.seed,
        }
    }
}

#[derive(serde::Serialize)]
pub struct IterationRecord {
    pub iteration: u32,
    pub result: &'static str,
    pub duration_iteration_ms: f32,
    pub problem_generation_ms: f32,
    pub problem_solving_duration_roundingsat_ms: f32,
    pub problem_solving_duration_scip_ms: f32,
    pub problem_parsing_duration_ms: f32,
    pub conflicts: u32,
    pub seed: u64,
}
