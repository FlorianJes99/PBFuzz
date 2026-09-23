use std::collections::HashSet;

use log::{debug, warn};

use crate::{
    fuzzer::domain::LiteralIdent,
    orchestrator::parsing::parser::{OutputParsingError, StatusKind, StatusLine},
};

pub struct DefaultParser;

impl DefaultParser {
    fn parse_solution(
        &self,
        solution_model_line: &str,
    ) -> Result<HashSet<LiteralIdent>, OutputParsingError> {
        solution_model_line
            .split_whitespace()
            .filter(|w| w.starts_with("x"))
            .map(|w| {
                w.parse::<LiteralIdent>()
                    .map_err(|err| OutputParsingError::ParseSolution {
                        line: w.into(),
                        source: err,
                    })
            })
            .collect()
    }

    pub fn parse_line(&self, line: &str) -> StatusLine {
        match line.split_once(' ') {
            Some(("s", rest)) => {
                let status = match rest {
                    "UNSATISFIABLE" => StatusKind::Unsat,
                    "SATISFIABLE" => StatusKind::Sat,
                    "OPTIMUM FOUND" => StatusKind::Optimum,
                    "TIMELIMIT" => StatusKind::TimeLimit,
                    _ => StatusKind::Unknown(rest.to_string()),
                };
                debug!("Parsed statusline {line} to status: {status}");
                StatusLine::Status(status)
            }
            Some(("v", rest)) => StatusLine::Solution(self.parse_solution(rest)),
            Some(("o", rest)) => StatusLine::Objective(rest.parse().map_err(|err| {
                OutputParsingError::ParseObjective {
                    line: rest.into(),
                    source: err,
                }
            })),
            Some(("c", rest)) => match rest.split_once(' ') {
                Some(("conflicts", rest)) => match parse_conflicts(rest) {
                    Ok(c) => StatusLine::Conflicts(c),
                    Err(err) => {
                        warn!("Could not parse conflicts from {} - Error: {err}", rest);
                        StatusLine::Ignored
                    }
                },
                Some(_) => StatusLine::Ignored,
                None => StatusLine::Ignored,
            },
            Some(_) => StatusLine::Ignored,
            None => StatusLine::Ignored,
        }
    }
}

/// Parse conflicts out of a given string
///
/// Handles a special case for "naps" solver
fn parse_conflicts(rest: &str) -> Result<u32, String> {
    match parse_exp_as_u32(rest) {
        Ok(c) => Ok(c),
        Err(e) => {
            // ugly hack to parse conflicts from naps output
            // `c conflicts : num_to_parse (additional info)`
            if let Some((_, rest)) = rest.split_once(':')
                && let Some(word) = rest.split_whitespace().next()
            {
                parse_exp_as_u32(word)
            } else {
                Err(e)
            }
        }
    }
}

/// Helper method to also be able to parse scientific notation like 1.34e5
fn parse_exp_as_u32(num: &str) -> Result<u32, String> {
    let value = num.parse::<f64>().map_err(|e| e.to_string())?;
    if value.fract() != 0.0 || value < 0.0 {
        return Err(format!("value is not a non-negative whole number: {num}"));
    }
    let value = value as u64;
    let value = TryInto::<u32>::try_into(value).map_err(|e| e.to_string())?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_parsing() {
        let rest = " : 42 (XXXXXXXXX)";
        let r = parse_conflicts(rest);
        assert_eq!(42, r.unwrap())
    }

    #[test]
    fn test_exp_notation_parsing() {
        let num = "1.23e5";
        let parsed_num = parse_exp_as_u32(num).unwrap();
        assert_eq!(123000, parsed_num)
    }
}
