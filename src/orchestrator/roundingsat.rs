use std::{
    collections::HashSet,
    path::{Path, PathBuf},
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
    fn run(&self, file_path: &Path) -> Result<Output, ExternalProgramError> {
        Command::new(self.binary.as_os_str())
            // If solution is SAT print found model
            // needed for verification of the found result
            .arg("--print-sol=1")
            .arg("--time-limit=60")
            .arg(file_path)
            .output()
            .map_err(|e| {
                ExternalProgramError::new(
                    self.get_name().to_string(),
                    e.to_string(),
                    ErrorKind::ProgramExecutionFailed,
                )
            })
    }

    fn get_name(&self) -> &str {
        "roundingsat"
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
        let mut objective_min_result: Result<i32, ExternalProgramError> =
            Err(ExternalProgramError::new(
                self.get_name().to_string(),
                "Field not parsed yet".to_string(),
                ErrorKind::OutputParsingError,
            ));
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
                objective_min_result = match line.split_whitespace().last() {
                    Some(n) => n.parse().map_err(|err| {
                        ExternalProgramError::new(
                            self.get_name().to_string(),
                            format!(
                                "Error while parsing minimum objective value from: {}\n Err: {}",
                                solution_model_line, err
                            ),
                            ErrorKind::OutputParsingError,
                        )
                    }),
                    None => Err(ExternalProgramError::new(
                        self.get_name().to_string(),
                        format!("Invalid line: {}", line),
                        ErrorKind::OutputParsingError,
                    )),
                }
            }
        }
        let status = match status_line {
            "s UNSATISFIABLE" => SolverStatus::Unsat {
                conflicts: conflicts,
            },
            "s OPTIMUM FOUND" => SolverStatus::Sat {
                conflicts: conflicts,
                solution: PBOValues::new(
                    PBSValues::new(self.parse_solution(solution_model_line)?),
                    objective_min_result?.into(),
                ),
            },
            // For PBO this means we run into time limit to find the BEST solution
            // TODO: maybe some better representation for this case
            "s SATISFIABLE" => SolverStatus::TimeLimit,
            "s TIMELIMIT" => SolverStatus::TimeLimit,
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
                solution: PBSValues::new(self.parse_solution(solution_model_line)?),
            },
            "s TIMELIMIT" => SolverStatus::TimeLimit,
            _ => SolverStatus::Unknown,
        };
        Ok(status)
    }

    fn parse_solution(
        &self,
        solution_model_line: &str,
    ) -> Result<HashSet<AtomIdent>, ExternalProgramError> {
        solution_model_line
            .split_whitespace()
            .filter(|w| w.starts_with("x"))
            .map(|w| {
                w.parse::<AtomIdent>().map_err(|err| ExternalProgramError::new(
                        self.get_name().to_string(),
                        format!("Could not parse provided rounding sat solution into atom ident: {}\nError: {}", w, err),
                        ErrorKind::OutputParsingError
                ))
            })
            .collect()
    }
}
