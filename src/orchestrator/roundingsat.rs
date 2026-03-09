use std::{
    collections::HashSet,
    path::PathBuf,
    process::{Command, Output},
};

use crate::orchestrator::external_program::{
    ErrorKind, ExternalProgramError, PBOProcessor, PBOValues, PBSValues, SolverResultPBO,
    SolverResultPBS, SolverStatus,
};
use crate::{
    fuzzer::domain::AtomIdent,
    orchestrator::external_program::{PBSProcessor, SolverRunner},
};

/// Struct for running the external program roundingsat and evaluating its output
/// on result and model.
pub struct Roundingsat {
    binary: PathBuf,
}

impl SolverRunner for Roundingsat {
    fn run(&self, file_path: &PathBuf) -> Result<Output, ExternalProgramError> {
        Command::new(self.binary.as_os_str())
            // If solution is SAT print found model
            // needed for verification of the found result
            .arg("--print-sol=1")
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
        "roundingsat".to_string()
    }
}

impl PBSProcessor for Roundingsat {
    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBS, ExternalProgramError> {
        self.extract_pbs_result_from_output(output)
    }
}

impl PBOProcessor for Roundingsat {
    fn extract_result_from_output(
        &self,
        output: Output,
    ) -> Result<super::external_program::SolverResultPBO, ExternalProgramError> {
        self.extract_pbo_result_from_output(output)
    }
}

impl Roundingsat {
    pub fn new(path_to_binary: PathBuf) -> Self {
        Roundingsat {
            binary: path_to_binary,
        }
    }

    fn extract_pbo_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBO, ExternalProgramError> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut conflicts = 0;
        let mut objective_min_result: Option<i32> = Option::None;
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
            } else if line.starts_with("o ") {
                objective_min_result = line.split_whitespace().last().and_then(|n| n.parse().ok())
            }
        }
        let status = match status_line {
            "s UNSATISFIABLE" => SolverStatus::Unsat {
                conflicts: conflicts,
            },
            "s SATISFIABLE" | "s OPTIMUM FOUND" => SolverStatus::Sat {
                conflicts: conflicts,
                solution: PBOValues::new(
                    PBSValues::new(self.parse_solution(solution_model_line)),
                    objective_min_result
                        .ok_or_else(|| {
                            ExternalProgramError::new(
                                self.get_name(),
                                "Error while parsing minimum objective value".to_string(),
                                ErrorKind::OutputParsingError,
                            )
                        })?
                        .into(),
                ),
            },
            _ => SolverStatus::Unknown,
        };
        Ok(status)
    }

    fn extract_pbs_result_from_output(
        &self,
        output: Output,
    ) -> Result<SolverResultPBS, ExternalProgramError> {
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
        let status = match status_line {
            "s UNSATISFIABLE" => SolverStatus::Unsat {
                conflicts: conflicts,
            },
            "s SATISFIABLE" | "s OPTIMUM FOUND" => SolverStatus::Sat {
                conflicts: conflicts,
                solution: PBSValues::new(self.parse_solution(solution_model_line)),
            },
            _ => SolverStatus::Unknown,
        };
        Ok(status)
    }

    fn parse_solution(&self, solution_model_line: &str) -> HashSet<AtomIdent> {
        solution_model_line
            .split_whitespace()
            .filter(|w| w.starts_with("x"))
            .map(|w| {
                w.parse::<AtomIdent>()
                    .expect("Roundingsat should output valid model. Probable parsing error!")
            })
            .collect()
    }
}
