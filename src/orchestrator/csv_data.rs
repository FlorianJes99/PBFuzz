pub const ORCHESTRATOR_DATA_FILE_PREFIX: &str = "orchestrator_data";

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
