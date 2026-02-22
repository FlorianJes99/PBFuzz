use std::{
    collections::HashSet,
    path::PathBuf,
    process::{Command, Output},
};

use regex::Regex;

use crate::fuzzer::domain::AtomIdent;
use crate::orchestrator::external_program::{
    ErrorKind, ExternalProgramError, Solver, SolverResult, SolverStatus,
};

/// Struct for running the external program scip and evaluating its output
/// on result and model.
pub struct Scip {
    binary: PathBuf,
}

impl Solver for Scip {
    fn execute_solver(&self, file_path: &PathBuf) -> Result<SolverResult, ExternalProgramError> {
        self.execute_program(file_path)
    }

    fn get_name(&self) -> String {
        "scip".to_string()
    }
}

impl Scip {
    pub fn new(path_to_binary: PathBuf) -> Self {
        Scip {
            binary: path_to_binary,
        }
    }

    fn execute_program(&self, input_file: &PathBuf) -> Result<SolverResult, ExternalProgramError> {
        Command::new(self.binary.as_os_str())
            // scip expects -f flag for input file
            .arg("-f")
            .arg(input_file)
            .output()
            .map_err(|e| {
                ExternalProgramError::new(
                    self.get_name(),
                    e.to_string(),
                    ErrorKind::ProgramExecutionFailed,
                )
            })
            .and_then(|output| self.extract_result_from_output(output))
    }

    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResult, ExternalProgramError> {
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
                return Ok(SolverResult::new(SolverStatus::Unsat, conflict_count));
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
            Ok(SolverResult::new(
                SolverStatus::Sat {
                    found_model: true_vars,
                },
                conflict_count,
            ))
        } else {
            Ok(SolverResult::new(SolverStatus::Unknown, 0))
        }
    }
}
