use serde::Serialize;

use crate::fuzzer::pb_settings::GenerationMethod;

#[derive(Serialize)]
pub struct IssueRecord {
    pub iteration: u32,
    pub seed: u64,
    pub generation_method: GenerationMethod,
    pub status: String,
}
