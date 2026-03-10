pub const ORCHESTRATOR_DATA_FILE_CSV: &str = "orchestrator_data.csv";

#[derive(serde::Serialize)]
pub struct IterationRecord {
    pub iteration: u32,
    pub result: &'static str,
    pub duration_ms: f32,
    pub seed: u64,
}
