use std::{collections::HashSet, fmt::Display, num::ParseIntError, process::Output};

use log::debug;
use thiserror::Error;

use crate::{
    fuzzer::{
        domain::{LiteralIdent, MinObjectiveValue, ParseLiteralError, SolutionModel},
        grammar::PBKind,
    },
    orchestrator::{
        parsing::{
            default_parser::DefaultParser,
            scip_parser::{ScipParser, ScipParsingError},
        },
        solving::{
            solver::{SolverExecutionRecord, SolverRun, SolverState},
            solver_config::ParserKind,
        },
    },
};

pub enum ParserImpl {
    Default(DefaultParser),
    Scip(ScipParser),
}

impl ParserImpl {
    pub fn from_kind(kind: &ParserKind) -> Self {
        match kind {
            ParserKind::Default => Self::Default(DefaultParser),
            ParserKind::Scip => Self::Scip(ScipParser),
        }
    }

    pub fn parse(&self, run: SolverRun, pb_kind: &PBKind) -> Result<ParserRun, OutputParsingError> {
        let SolverRun { record, state } = run;

        let parsed_status = match state {
            SolverState::Terminated(output) => match pb_kind {
                PBKind::PBS => self.parse_pbs(&output)?,
                PBKind::PBO => self.parse_pbo(&output)?,
            },
            SolverState::Timeout => ParsedStatus::TimeLimit,
        };

        Ok(ParserRun {
            record,
            parsed_status,
        })
    }
    fn parse_pbs(&self, output: &Output) -> Result<ParsedStatus, OutputParsingError> {
        let result = self.scan(output)?;
        match result.status {
            Some(s) => match s {
                StatusKind::Optimum | StatusKind::Sat => Ok(ParsedStatus::Sat {
                    conflicts: result.conflicts,
                    solution: result.solution.into(),
                }),
                StatusKind::Unsat => Ok(ParsedStatus::Unsat {
                    conflicts: result.conflicts,
                }),
                StatusKind::TimeLimit => Ok(ParsedStatus::TimeLimit),
                StatusKind::Unknown(s) => Ok(ParsedStatus::Unknown { status: s }),
            },
            None => Err(OutputParsingError::StatusMissing),
        }
    }

    fn parse_pbo(&self, output: &Output) -> Result<ParsedStatus, OutputParsingError> {
        let result = self.scan(output)?;
        match result.status {
            Some(s) => match s {
                StatusKind::Optimum | StatusKind::Sat => result
                    .objective
                    .ok_or(OutputParsingError::ObjectiveMissing)
                    .map(|obj| ParsedStatus::Optimum {
                        conflicts: result.conflicts,
                        solution: result.solution.into(),
                        minimum: obj.into(),
                    }),
                StatusKind::Unsat => Ok(ParsedStatus::Unsat {
                    conflicts: result.conflicts,
                }),
                StatusKind::TimeLimit => Ok(ParsedStatus::TimeLimit),
                StatusKind::Unknown(s) => Ok(ParsedStatus::Unknown { status: s }),
            },
            None => Err(OutputParsingError::StatusMissing),
        }
    }

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

    fn parse_line(&self, line: &str) -> StatusLine {
        match self {
            ParserImpl::Default(default_parser) => default_parser.parse_line(line),
            ParserImpl::Scip(scip_parser) => scip_parser.parse_line(line),
        }
    }
}

pub struct ParserRun {
    pub record: SolverExecutionRecord,
    pub parsed_status: ParsedStatus,
}

#[derive(Debug, Clone)]
pub enum ParsedStatus {
    Sat {
        conflicts: Option<u32>,
        solution: SolutionModel,
    },
    Optimum {
        conflicts: Option<u32>,
        solution: SolutionModel,
        minimum: MinObjectiveValue,
    },

    Unsat {
        conflicts: Option<u32>,
    },

    TimeLimit,
    Unknown {
        status: String,
    },
}

impl Display for ParsedStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParsedStatus::Sat {
                conflicts,
                solution,
            } => {
                writeln!(f, "Status: SAT")?;
                match conflicts {
                    Some(c) => writeln!(f, "Conflicts: {c}")?,
                    None => writeln!(f, "Conflicts: NULL")?,
                }
                writeln!(f, "Solution: {solution}")
            }
            ParsedStatus::Optimum {
                conflicts,
                solution,
                minimum,
            } => {
                writeln!(f, "Status: Optimum found")?;
                match conflicts {
                    Some(c) => writeln!(f, "Conflicts: {c}")?,
                    None => writeln!(f, "Conflicts: NULL")?,
                }
                writeln!(f, "Solution: {solution}")?;
                writeln!(f, "Minimum: {minimum}")
            }
            ParsedStatus::Unsat { conflicts } => {
                writeln!(f, "Status: UNSAT")?;
                match conflicts {
                    Some(c) => writeln!(f, "Conflicts: {c}")?,
                    None => writeln!(f, "Conflicts: NULL")?,
                }
                Ok(())
            }
            ParsedStatus::TimeLimit => writeln!(f, "Status: TIMELIMIT"),
            ParsedStatus::Unknown { status } => {
                writeln!(f, "Status: Unknown")?;
                writeln!(f, "Parsed: {status}")
            }
        }
    }
}

impl ParsedStatus {
    pub fn get_conflicts(&self) -> Option<u32> {
        match self {
            ParsedStatus::Sat { conflicts, .. } => *conflicts,
            ParsedStatus::Optimum { conflicts, .. } => *conflicts,
            ParsedStatus::Unsat { conflicts } => *conflicts,
            ParsedStatus::TimeLimit => None,
            ParsedStatus::Unknown { .. } => None,
        }
    }

    pub fn get_min_objective(&self) -> Option<&MinObjectiveValue> {
        match &self {
            ParsedStatus::Optimum {
                conflicts: _,
                solution: _,
                minimum,
            } => Some(minimum),
            _ => None,
        }
    }

    pub fn get_status_str(&self) -> &'static str {
        match self {
            ParsedStatus::Sat { .. } => "sat",
            ParsedStatus::Optimum { .. } => "optimum",
            ParsedStatus::Unsat { .. } => "unsat",
            ParsedStatus::TimeLimit => "timelimit",
            ParsedStatus::Unknown { .. } => "unknown",
        }
    }

    pub fn did_not_timeout(&self) -> bool {
        !matches!(self, ParsedStatus::TimeLimit)
    }

    pub fn has_model(&self) -> bool {
        matches!(
            self,
            ParsedStatus::Sat { .. } | ParsedStatus::Optimum { .. },
        )
    }
}

#[derive(Default, Debug)]
struct ScannedResult {
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
    #[error("Failed to parse line {line} into valid objective: source: {source}")]
    ParseObjective {
        line: String,
        #[source]
        source: ParseIntError,
    },
    #[error("Parsing scip output failed: {0}")]
    Scip(#[from] ScipParsingError),
}
