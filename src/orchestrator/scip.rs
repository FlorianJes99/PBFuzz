use std::{
    collections::HashSet,
    path::{Path, PathBuf},
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
    fn run(&self, file_path: &Path) -> Result<Output, ExternalProgramError> {
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
                return Ok(SolverStatus::Unsat {
                    conflicts: conflict_count,
                });
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
                    .and_then(|w| self.parse_as_i32(w.trim()));
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
            min_objective_result.map(|objective_result| SolverStatus::Sat {
                conflicts: conflict_count,
                solution: PBOValues::new(PBSValues::new(true_vars), objective_result.into()),
            })
        } else {
            Ok(SolverStatus::Unknown)
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
                return Ok(SolverStatus::Unsat {
                    conflicts: conflict_count,
                });
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
            Ok(SolverStatus::Sat {
                conflicts: conflict_count,
                solution: PBSValues::new(true_vars),
            })
        } else {
            Ok(SolverStatus::Unknown)
        }
    }
    /// Scip sometime returns floats, which actually should be integers. This method will check for
    /// this type of inconsistency in Scip and will still parse the value correctly.
    fn parse_as_i32(&self, s: &str) -> Result<i32, ExternalProgramError> {
        let parsed: f64 = s.parse().map_err(|e| {
            ExternalProgramError::new(
                self.get_name(),
                format!("Failed to parse integer: {e}"),
                ErrorKind::OutputParsingError,
            )
        })?;
        let rounded = parsed.round();

        // Only convert if it's close to an integer and non-negative
        let epsilon = 1e-5;
        if (parsed - rounded).abs() < epsilon
            && rounded >= i32::MIN as f64
            && rounded <= i32::MAX as f64
        {
            Ok(rounded as i32)
        } else {
            Err(ExternalProgramError::new(
                self.get_name(),
                format!(
                    "Floating point number is not close enough to the expected integer.\nMax derivation: {epsilon}, Exptected: {rounded}, Actual: {parsed}"
                ),
                ErrorKind::OutputParsingError,
            ))
        }
    }
}
