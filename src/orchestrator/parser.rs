use std::{collections::HashSet, fmt::Display, num::ParseIntError, process::Output};

use log::debug;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    fuzzer::domain::{LiteralIdent, ParseLiteralError},
    orchestrator::{
        external_program::{SolverSolution, SolverStatus},
        scip_parser::ScipParsingError,
    },
};

#[derive(Default, Serialize, Deserialize)]
pub enum ParserKind {
    #[default]
    Default,
    Scip,
}

pub trait Parser: Sync {
    /// return the name of the parser
    fn name(&self) -> &str;

    /// Parse a single line of a solver output into one of the expected representations
    fn parse_line(&self, line: &str) -> StatusLine;

    /// Folds all the collected information into an aggregated result
    fn scan(&self, output: &Output) -> Result<ScannedResult, OutputParsingError> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Output of program:\n {stdout}");
        stdout
            .lines()
            .map(str::trim)
            .map(|l| self.parse_line(l))
            .try_fold(ScannedResult::default(), |mut acc, line| {
                match line {
                    StatusLine::Status(s) => acc.status = Some(s),
                    StatusLine::Solution(s) => acc.solution.extend(s?),
                    StatusLine::Objective(o) => acc.objective = Some(o?),
                    StatusLine::Conflicts(c) => acc.conflicts = Some(c),
                    StatusLine::Ignored => {}
                };
                Ok(acc)
            })
    }

    fn parse_pbs(&self, output: &Output) -> Result<SolverStatus, OutputParsingError> {
        let result = self.scan(output)?;
        match result.status {
            Some(s) => match s {
                StatusKind::Sat => Ok(SolverStatus::Sat {
                    conflicts: result.conflicts.unwrap_or(0),
                    solution: SolverSolution::PBS(result.solution.into()),
                }),
                StatusKind::Unsat => Ok(SolverStatus::Unsat {
                    conflicts: result.conflicts.unwrap_or(0),
                }),
                StatusKind::TimeLimit => Ok(SolverStatus::TimeLimit),
                StatusKind::Unknown => Ok(SolverStatus::Unknown),
            },
            None => Err(OutputParsingError::StatusMissing),
        }
    }
    fn parse_pbo(&self, output: &Output) -> Result<SolverStatus, OutputParsingError> {
        let result = self.scan(output)?;
        match result.status {
            Some(s) => match s {
                StatusKind::Sat => match result.objective {
                    Some(o) => Ok(SolverStatus::Sat {
                        conflicts: result.conflicts.unwrap_or(0),
                        solution: SolverSolution::PBO(result.solution.into(), o.into()),
                    }),
                    None => Ok(SolverStatus::TimeLimit),
                },
                StatusKind::Unsat => Ok(SolverStatus::Unsat {
                    conflicts: result.conflicts.unwrap_or(0),
                }),
                StatusKind::TimeLimit => Ok(SolverStatus::TimeLimit),
                StatusKind::Unknown => Ok(SolverStatus::Unknown),
            },
            None => Err(OutputParsingError::StatusMissing),
        }
    }
}

#[derive(Default)]
pub struct ScannedResult {
    status: Option<StatusKind>,
    solution: HashSet<LiteralIdent>,
    objective: Option<i32>,
    conflicts: Option<u32>,
}

pub enum StatusKind {
    Sat,
    Unsat,
    TimeLimit,
    Unknown,
}

impl Display for StatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusKind::Sat => write!(f, "sat"),
            StatusKind::Unsat => write!(f, "unsat"),
            StatusKind::TimeLimit => write!(f, "timelimit"),
            StatusKind::Unknown => write!(f, "unknown"),
        }
    }
}

pub enum StatusLine {
    Status(StatusKind),
    Solution(Result<HashSet<LiteralIdent>, OutputParsingError>),
    Objective(Result<i32, OutputParsingError>),
    Conflicts(u32),
    Ignored,
}

#[derive(Debug, Error)]
pub enum OutputParsingError {
    #[error("Status was not parsed from solver output")]
    StatusMissing,
    #[error("Failed to parse line {line} into solution: source: {source}")]
    ParseSolution {
        line: String,
        #[source]
        source: ParseLiteralError,
    },
    #[error("Faile to parse line {line} into valid objective: source: {source}")]
    ParseObjective {
        line: String,
        #[source]
        source: ParseIntError,
    },
    // TODO: get rid of circular dependency
    #[error("Parsing scip output failed")]
    Scip(#[from] ScipParsingError),
}
