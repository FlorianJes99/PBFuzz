use std::fmt::Display;

use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
pub struct MetaData {
    pub program_version: ProgramVersion,
    pub commit_hash: CommitHash,
}

#[derive(Clone, Copy, Serialize)]
pub struct ProgramVersion(&'static str);

impl Display for ProgramVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for ProgramVersion {
    fn from(value: &'static str) -> Self {
        ProgramVersion(value)
    }
}

#[derive(Clone, Copy, Serialize)]
pub struct CommitHash(&'static str);

impl Display for CommitHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for CommitHash {
    fn from(value: &'static str) -> Self {
        CommitHash(value)
    }
}
