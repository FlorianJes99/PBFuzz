use std::fmt::Display;

use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
pub struct MetaData {
    pub program_version: ProgramVersion,
    pub commit_hash: Option<CommitHash>,
}

impl Display for MetaData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let MetaData {
            program_version,
            commit_hash,
        } = self;
        match commit_hash {
            Some(c) => write!(f, "{program_version} (Commit {c})"),
            None => write!(f, "{program_version}"),
        }
    }
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
