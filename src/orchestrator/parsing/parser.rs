use std::{collections::HashSet, fmt::Display, num::ParseIntError, process::Output};

use log::debug;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    fuzzer::domain::{LiteralIdent, ParseLiteralError},
    orchestrator::{
        parsing::{
            default_parser::DefaultParser,
            scip_parser::{ScipParser, ScipParsingError},
        },
        solving::solver::{SolverSolution, SolverStatus},
    },
};

#[derive(Default, Serialize, Deserialize)]
pub enum ParserKind {
    #[default]
    Default,
    Scip,
}

impl ParserKind {
    pub fn create_parser(&self, name: String) -> Box<dyn Parser> {
        match self {
            ParserKind::Default => Box::new(DefaultParser::new(name)),
            ParserKind::Scip => Box::new(ScipParser::new(name)),
        }
    }
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
                StatusKind::Optimum => Ok(SolverStatus::Sat {
                    conflicts: result.conflicts.unwrap_or(0),
                    solution: SolverSolution::PBS(result.solution.into()),
                }),
                StatusKind::Unsat => Ok(SolverStatus::Unsat {
                    conflicts: result.conflicts.unwrap_or(0),
                }),
                // only sat means we could not find optimum in given time limit
                StatusKind::Sat => Ok(SolverStatus::TimeLimit),
                StatusKind::TimeLimit => Ok(SolverStatus::TimeLimit),
                StatusKind::Unknown(s) => Ok(SolverStatus::Unknown {
                    solver_name: self.name().to_string(),
                    status: s,
                }),
            },
            None => Err(OutputParsingError::StatusMissing),
        }
    }
    fn parse_pbo(&self, output: &Output) -> Result<SolverStatus, OutputParsingError> {
        let result = self.scan(output)?;
        match result.status {
            Some(s) => match s {
                StatusKind::Optimum => result
                    .objective
                    .ok_or(OutputParsingError::ObjectiveMissing)
                    .map(|obj| SolverStatus::Sat {
                        conflicts: result.conflicts.unwrap_or(0),
                        solution: SolverSolution::PBO(result.solution.into(), obj.into()),
                    }),
                StatusKind::Unsat => Ok(SolverStatus::Unsat {
                    conflicts: result.conflicts.unwrap_or(0),
                }),
                // only sat means we could not find optimum in given time limit
                StatusKind::Sat => Ok(SolverStatus::TimeLimit),
                StatusKind::TimeLimit => Ok(SolverStatus::TimeLimit),
                StatusKind::Unknown(s) => Ok(SolverStatus::Unknown {
                    solver_name: self.name().to_string(),
                    status: s,
                }),
            },
            None => Err(OutputParsingError::StatusMissing),
        }
    }
}

#[derive(Default, Debug)]
pub struct ScannedResult {
    status: Option<StatusKind>,
    solution: HashSet<LiteralIdent>,
    objective: Option<i32>,
    conflicts: Option<u32>,
}

#[derive(Debug)]
pub enum StatusKind {
    Sat,
    Optimum,
    Unsat,
    TimeLimit,
    Unknown(String),
}

impl Display for StatusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatusKind::Sat => f.write_str("sat"),
            StatusKind::Optimum => f.write_str("optimum found"),
            StatusKind::Unsat => f.write_str("unsat"),
            StatusKind::TimeLimit => f.write_str("timelimit"),
            StatusKind::Unknown(s) => write!(f, "unknown ({s})"),
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
    #[error("Objective minimum wasn't parsed, but problem is of kind PBO")]
    ObjectiveMissing,
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
