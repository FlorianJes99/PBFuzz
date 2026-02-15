use std::{
    collections::HashSet,
    path::PathBuf,
    process::{Command, Output},
};

use crate::grammar::domain::AtomIdent;
use crate::orchestrator::external_program::{
    ErrorKind, ExternalProgramError, SolverResult, SolverStatus, Solver
};

/// Struct for running the external program roundingsat and evaluating its output
/// on result and model.
pub struct Roundingsat {
    binary: PathBuf,
}

impl Solver for Roundingsat {
    fn execute_solver(&self, file_path: &PathBuf) -> Result<SolverResult, ExternalProgramError> {
        self.execute_program(file_path)
    }

    fn get_name(&self) -> String {
        "roundingsat".to_string()
    }
}

impl Roundingsat {
    pub fn new(path_to_binary: PathBuf) -> Self {
        Roundingsat {
            binary: path_to_binary,
        }
    }

    fn execute_program(
        &self,
        input_file: &PathBuf,
    ) -> Result<SolverResult, ExternalProgramError> {
        Command::new(self.binary.as_os_str())
            // If solution is SAT print found model
            // needed for verification of the found result
            .arg("--print-sol=1")
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
        let mut conflicts = 0;
        let mut status_line = "";
        let mut solution_model_line = "";
        for line in stdout.lines() {
            let line = line.trim();
            // Hacky solution to look for conflicts of roundingsat output
            if line.starts_with("c conflicts") {
                conflicts = line
                    .split_whitespace()
                    .last()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("s ") {
                status_line = line
            } else if line.starts_with("v ") {
                solution_model_line = line
            }
        }
        let status = status_line
            .split_whitespace()
            .last()
            .map(|word| {
                if word.starts_with("UNSAT") {
                    SolverStatus::Unsat
                } else if word.starts_with("SAT") {
                    SolverStatus::Sat {
                        found_model: self.parse_solution(solution_model_line),
                    }
                } else {
                    SolverStatus::Unknown
                }
            })
            .unwrap_or(SolverStatus::Unknown);
        Ok(SolverResult::new(status, conflicts))
    }

    fn parse_solution(&self, solution_model_line: &str) -> HashSet<AtomIdent> {
        solution_model_line
            .split_whitespace()
            .filter(|w| w.starts_with("x"))
            .map(|w| w.parse::<AtomIdent>())
            .map(|r| r.expect("Roundingsat should output valid model. Probable parsing error!"))
            .collect()
    }
}
