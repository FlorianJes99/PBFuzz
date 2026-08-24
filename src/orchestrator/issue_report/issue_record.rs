use serde::Serialize;

use crate::fuzzer::pb_settings::GenerationMethod;

#[derive(Serialize)]
pub struct IssueRecord {
    pub iteration: u32,
    pub seed: u64,
    pub status: String,
    pub generation_method: GenerationMethod,
    pub with_option_fuzzing: bool,
    pub solver_infos: Vec<SolverInfo>,
}

#[derive(Serialize)]
pub struct SolverInfo {
    pub binary: String,
    pub used_args: Vec<String>,
}
