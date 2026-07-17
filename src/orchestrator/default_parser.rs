use std::collections::HashSet;

use log::{debug, error};

use crate::{
    fuzzer::domain::LiteralIdent,
    orchestrator::parser::{OutputParsingError, Parser, StatusKind, StatusLine},
};

pub struct DefaultParser {
    name: String,
}

impl DefaultParser {
    pub fn new(name: String) -> Self {
        Self { name }
    }

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
}

impl Parser for DefaultParser {
    fn name(&self) -> &str {
        &self.name
    }

    fn parse_line(&self, line: &str) -> StatusLine {
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
                Some(("conflicts", rest)) => match rest.parse() {
                    Ok(c) => StatusLine::Conflicts(c),
                    Err(err) => {
                        error!("{err}");
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
