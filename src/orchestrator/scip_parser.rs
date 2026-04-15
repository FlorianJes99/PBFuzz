use crate::orchestrator::parser::StatusKind;
use crate::orchestrator::parser::StatusLine;
use std::collections::HashSet;

use crate::orchestrator::{external_program::ExternalProgramError, parser::Parser};

pub struct ScipParser {
    name: String,
}

impl ScipParser {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    /// Scip sometime returns floats, which actually should be integers. This method will check for
    /// this type of inconsistency in Scip and will still parse the value correctly.
    fn parse_as_i32(&self, s: &str) -> Result<i32, ExternalProgramError> {
        let parsed: f64 = s
            .parse()
            .map_err(|e| self.error(format!("Failed to parse integer: {e}")))?;
        let rounded = parsed.round();

        // Only convert if it's close to an integer and non-negative
        let epsilon = 1e-5;
        if (parsed - rounded).abs() < epsilon
            && rounded >= i32::MIN as f64
            && rounded <= i32::MAX as f64
        {
            Ok(rounded as i32)
        } else {
            Err(self.error(
                    format!(
                        "Floating point number is not close enough to the expected integer.\nMax derivation: {}, Exptected: {}, Actual: {}",
                         epsilon,
                         rounded,
                         parsed
                     )))
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
                .ok_or_else(|| {
                    self.error(format!(
                        "Expected objective value but could not find in line. Line: {line}"
                    ))
                })
                .and_then(|v| self.parse_as_i32(v.trim()));
            return StatusLine::Objective(result);
        }

        if line.starts_with("x")
            && let Some(ident) = line.split_whitespace().next()
        {
            return StatusLine::Solution(
                ident
                    .parse()
                    .map(|v| HashSet::from([v]))
                    .map_err(|err| self.error(format!("Could not parse LiteralIdent: {err}"))),
            );
        }
        StatusLine::Ignored
    }
}
