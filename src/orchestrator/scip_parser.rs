use thiserror::Error;

use crate::orchestrator::parser::OutputParsingError;
use crate::orchestrator::parser::StatusKind;
use crate::orchestrator::parser::StatusLine;
use std::collections::HashSet;
use std::num::ParseFloatError;

use crate::orchestrator::parser::Parser;

pub struct ScipParser {
    name: String,
}

#[derive(Debug, Error)]
pub enum ScipParsingError {
    #[error("Parsing float failed")]
    ParseFloat(#[from] ParseFloatError),
    #[error(
        "Parsed number ({parsed}) is not close enough to expected integer ({expected}), max epsilon: {epsilon}"
    )]
    FloatNotCloseEnough {
        parsed: f64,
        expected: i32,
        epsilon: f64,
    },
    #[error("Expected objective as last word in line, but got: {0}")]
    NoObjectiveFound(String),
}

impl ScipParser {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Scip sometimes returns floats, which actually should be integers. This method will check for
    /// this type of inconsistency in Scip and will still parse the value correctly.
    fn parse_as_i32(&self, s: &str) -> Result<i32, ScipParsingError> {
        let parsed: f64 = s.parse().map_err(ScipParsingError::ParseFloat)?;
        let rounded = parsed.round();

        // Only convert if it's close to an integer and non-negative
        let epsilon = 1e-5;
        if (parsed - rounded).abs() < epsilon && rounded >= 0.0 && rounded <= i32::MAX as f64 {
            Ok(rounded as i32)
        } else {
            Err(ScipParsingError::FloatNotCloseEnough {
                parsed,
                expected: rounded as i32,
                epsilon,
            })
        }
    }
}

impl Parser for ScipParser {
    fn name(&self) -> &str {
        &self.name
    }

    fn parse_line(&self, line: &str) -> StatusLine {
        let line = line.trim();

        if line.contains("[infeasible]") {
            return StatusLine::Status(StatusKind::Unsat);
        }
        if line.contains("[time limit reached]") {
            return StatusLine::Status(StatusKind::TimeLimit);
        }
        if line.contains("[optimal solution found]") {
            return StatusLine::Status(StatusKind::Sat);
        }

        if let Some(rest) = line.strip_prefix("objective value:") {
            let result = rest
                .split_whitespace()
                .last()
                .ok_or_else(|| ScipParsingError::NoObjectiveFound(line.into()))
                .and_then(|v| self.parse_as_i32(v.trim()));
            return StatusLine::Objective(result.map_err(Into::into));
        }

        if line.starts_with("x")
            && let Some(ident) = line.split_whitespace().next()
        {
            return StatusLine::Solution(ident.parse().map(|v| HashSet::from([v])).map_err(
                |err| OutputParsingError::ParseSolution {
                    line: line.into(),
                    source: err,
                },
            ));
        }
        StatusLine::Ignored
    }
}
