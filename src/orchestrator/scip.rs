use std::{
    collections::HashSet,
    path::PathBuf,
    process::{Command, Output},
};

use regex::Regex;

use crate::orchestrator::external_program::{
    ErrorKind, ExternalProgramError, PBOValues, PBSValues, SolverResultPBS, SolverStatus,
};
use crate::{
    fuzzer::domain::AtomIdent,
    orchestrator::external_program::{PBOProcessor, PBSProcessor, SolverResultPBO, SolverRunner},
};

/// Struct for running the external program scip and evaluating its output
/// on result and model.
pub struct Scip {
    binary: PathBuf,
}

impl SolverRunner for Scip {
    fn run(&self, file_path: &PathBuf) -> Result<Output, ExternalProgramError> {
        Command::new(self.binary.as_os_str())
            // scip expects -f flag for input file
            .arg("-f")
            .arg(file_path)
            .output()
            .map_err(|e| {
                ExternalProgramError::new(
                    self.get_name(),
                    e.to_string(),
                    ErrorKind::ProgramExecutionFailed,
                )
            })
    }

    fn get_name(&self) -> String {
        "scip".to_string()
    }
}

impl PBSProcessor for Scip {
    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBS, ExternalProgramError> {
        self.extract_pbs_result_from_output(output)
    }
}

impl PBOProcessor for Scip {
    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBO, ExternalProgramError> {
        self.extract_pbo_result_from_output(output)
    }
}

impl Scip {
    pub fn new(path_to_binary: PathBuf) -> Self {
        Scip {
            binary: path_to_binary,
        }
    }
    fn extract_pbo_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBO, ExternalProgramError> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let sat_parser = Regex::new(r"\[optimal solution found\]").unwrap();
        let unsat_parser = Regex::new(r"\[infeasible\]").unwrap();
        // We don't support conflict counting for scip ATM
        let conflict_count = 0;
        let mut min_objective_result: Result<i32, ExternalProgramError> =
            Err(ExternalProgramError::new(
                self.get_name(),
                "Objective not parsed yet".to_string(),
                ErrorKind::OutputParsingError,
            ));
        let mut is_sat = false;
        let mut true_vars: HashSet<AtomIdent> = HashSet::new();
        for line in stdout.lines() {
            let line = line.trim();
            if unsat_parser.is_match(line) {
                return Ok(SolverResultPBO::new(SolverStatus::Unsat, conflict_count));
            }

            if sat_parser.is_match(line) {
                is_sat = true;
            }

            if line.starts_with("objective value:") {
                min_objective_result = line
                    .split_whitespace()
                    .last()
                    .ok_or(ExternalProgramError::new(
                        self.get_name(),
                        format!("Expteced at least one word in line: {line}"),
                        ErrorKind::OutputParsingError,
                    ))
                    .and_then(|w| {
                        w.trim().parse::<i32>().map_err(|e| {
                            ExternalProgramError::new(
                                self.get_name(),
                                format!("Failed to parse integer: {e}"),
                                ErrorKind::OutputParsingError,
                            )
                        })
                    });
            }
            if line.starts_with("x") {
                if let Some(ident) = line.split_whitespace().next()
                    && let Ok(parsed_ident) = ident.parse::<AtomIdent>()
                {
                    true_vars.insert(parsed_ident);
                }
            }
        }

        if is_sat {
            match min_objective_result {
                Ok(objective_result) => Ok(SolverResultPBO::new(
                    SolverStatus::Sat(PBOValues::new(
                        PBSValues::new(true_vars),
                        objective_result.into(),
                    )),
                    conflict_count,
                )),
                Err(e) => Err(e),
            }
        } else {
            Ok(SolverResultPBO::new(SolverStatus::Unknown, 0))
        }
    }

    fn extract_pbs_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBS, ExternalProgramError> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let sat_parser = Regex::new(r"\[optimal solution found\]").unwrap();
        let unsat_parser = Regex::new(r"\[infeasible\]").unwrap();
        // We don't support conflict counting for scip ATM
        let conflict_count = 0;
        let mut is_sat = false;
        let mut true_vars: HashSet<AtomIdent> = HashSet::new();
        for line in stdout.lines() {
            let line = line.trim();
            if unsat_parser.is_match(line) {
                return Ok(SolverResultPBS::new(SolverStatus::Unsat, conflict_count));
            }

            if sat_parser.is_match(line) {
                is_sat = true;
            }

            if line.starts_with("x") {
                if let Some(ident) = line.split_whitespace().next()
                    && let Ok(parsed_ident) = ident.parse::<AtomIdent>()
                {
                    true_vars.insert(parsed_ident);
                }
            }
        }

        if is_sat {
            Ok(SolverResultPBS::new(
                SolverStatus::Sat(PBSValues::new(true_vars)),
                conflict_count,
            ))
        } else {
            Ok(SolverResultPBS::new(SolverStatus::Unknown, 0))
        }
    }
}
