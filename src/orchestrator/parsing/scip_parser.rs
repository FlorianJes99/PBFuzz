use thiserror::Error;

use crate::fuzzer::domain::LiteralIdent;
use crate::orchestrator::parsing::parser::OutputParsingError;
use crate::orchestrator::parsing::parser::StatusKind;
use crate::orchestrator::parsing::parser::StatusLine;
use std::collections::HashSet;
use std::num::ParseFloatError;

pub struct ScipParser;

#[derive(Debug, Error)]
pub enum ScipParsingError {
    #[error(transparent)]
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
    #[error("Could not parse a solution literal from: {0}")]
    ParseSolutionValue(String),
}

impl ScipParser {
    pub fn parse_line(&self, line: &str) -> StatusLine {
        let line = line.trim();

        if line.contains("[infeasible]") {
            return StatusLine::Status(StatusKind::Unsat);
        }
        if line.contains("[time limit reached]") {
            return StatusLine::Status(StatusKind::TimeLimit);
        }
        if line.contains("[optimal solution found]") {
            return StatusLine::Status(StatusKind::Optimum);
        }

        if let Some(rest) = line.strip_prefix("objective value:") {
            let result = rest
                .split_whitespace()
                .last()
                .ok_or_else(|| ScipParsingError::NoObjectiveFound(line.into()))
                .and_then(|v| parse_as_i32(v.trim()));
            return StatusLine::Objective(result.map_err(Into::into));
        }

        if line.starts_with("x") {
            return match parse_solution_value(line) {
                Ok(ok) => match ok {
                    Some(v) => StatusLine::Solution(Ok(HashSet::from([v]))),
                    None => StatusLine::Ignored,
                },
                Err(e) => StatusLine::Solution(Err(e)),
            };
        }
        StatusLine::Ignored
    }
}

fn parse_solution_value(line: &str) -> Result<Option<LiteralIdent>, OutputParsingError> {
    let mut parts = line.split_whitespace();
    if let Some(ident) = parts.next()
        && let Some(value_str) = parts.next()
    {
        let value = parse_as_i32(value_str)?;
        // scip sometimes prints variables in the solution which are close to zero. Those have to be filtered out.
        if value == 0 {
            Ok(None)
        } else {
            ident
                .parse()
                .map(Some)
                .map_err(|err| OutputParsingError::ParseSolution {
                    line: line.into(),
                    source: err,
                })
        }
    } else {
        Err(ScipParsingError::ParseSolutionValue(line.into()).into())
    }
}

/// Scip sometimes returns floats, which actually should be integers. This method will check for
/// this type of inconsistency in Scip and will still parse the value correctly.
fn parse_as_i32(s: &str) -> Result<i32, ScipParsingError> {
    let parsed: f64 = s.parse().map_err(ScipParsingError::ParseFloat)?;
    let rounded = parsed.round();

    // Only convert if it's close to an integer and non-negative
    let epsilon = 1e-5;
    if (parsed - rounded).abs() < epsilon && rounded <= i32::MAX as f64 {
        Ok(rounded as i32)
    } else {
        Err(ScipParsingError::FloatNotCloseEnough {
            parsed,
            expected: rounded as i32,
            epsilon,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::parse_solution_value;

    #[test]
    fn test_parse_solution() {
        let line = "x15                                 0.999999590181784   (obj:0)";
        let res = parse_solution_value(line).unwrap().unwrap();
        assert_eq!(15, res.literal_id());
        let line = "x63                              4.09818216140501e-07   (obj:0)";
        let res = parse_solution_value(line).unwrap();
        assert!(res.is_none())
    }
}
