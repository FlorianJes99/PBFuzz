use crate::fuzzer::helpers::OFFSET;
use std::{collections::HashMap, fmt::Display, time::Duration};

use serde::Serialize;

use crate::fuzzer::helpers::{RunningAverage, RunningAverageDuration};

#[derive(Serialize)]
pub struct RunStatistics {
    amount_runs: u32,
    pub amount_sat: u32,
    pub amount_unsat: u32,
    amount_unknown: u32,
    pub average_conflicts: RunningAverage,
    average_iteration_duration: RunningAverageDuration,
    errors: u32,
    timeouts: u32,
    large_solving_difference: u32,
    stats_per_solver: HashMap<String, SolverStats>,
}

#[derive(Serialize, Default)]
pub struct SolverStats {
    pub expected: u32,
    pub timeouts: u32,
    pub failures: Failures,
}

#[derive(Serialize, Default)]
pub struct Failures {
    pub crash: u32,
    pub proof_invalid: u32,
    pub wrong_unsat: u32,
    pub wrong_sat: u32,
    pub wrong_optimum: u32,
    pub invalid_model: u32,
    pub unknown: u32,
    pub other: u32,
}

impl Failures {
    fn add_failure(&mut self, failure_type: FailureKind) {
        match failure_type {
            FailureKind::Crash => self.crash += 1,
            FailureKind::InvalidProof => self.proof_invalid += 1,
            FailureKind::WrongUnsat => self.wrong_unsat += 1,
            FailureKind::WrongSat => self.wrong_sat += 1,
            FailureKind::WrongOptimum => self.wrong_optimum += 1,
            FailureKind::InvalidModel => self.invalid_model += 1,
            FailureKind::Unknown => self.unknown += 1,
            FailureKind::Other => self.other += 1,
        }
    }

    pub fn total_failures(&self) -> u32 {
        let Failures {
            crash,
            proof_invalid,
            wrong_unsat,
            wrong_sat,
            wrong_optimum,
            invalid_model,
            unknown,
            other,
        } = self;
        crash
            + proof_invalid
            + wrong_unsat
            + wrong_sat
            + wrong_optimum
            + invalid_model
            + unknown
            + other
    }
}

pub enum SolverStateAfterIteration {
    Failure(FailureKind),
    Timeout,
    Expected,
}

pub enum FailureKind {
    Crash,
    InvalidProof,
    WrongUnsat,
    WrongSat,
    WrongOptimum,
    InvalidModel,
    Unknown,
    Other,
}

pub enum SingleRunStats {
    Successfull(SingleRunRecord),
    Error,
}

pub enum SimpleResult {
    Sat,
    Optimum,
    Unsat,
    Timeout,
    Errors(u32),
    Unknown,
    Other,
}

pub struct SingleRunRecord {
    result: SimpleResult,
    conflicts: Vec<u32>,
    iteration_duration: Duration,
    large_solving_time_difference: bool,
    solver_states: Vec<(String, SolverStateAfterIteration)>,
}

impl SingleRunRecord {
    pub fn new(
        result: SimpleResult,
        conflicts: Vec<u32>,
        iteration_duration: Duration,
        large_solving_time_difference: bool,
        solver_states: Vec<(String, SolverStateAfterIteration)>,
    ) -> Self {
        Self {
            result,
            conflicts,
            iteration_duration,
            large_solving_time_difference,
            solver_states,
        }
    }
}

impl RunStatistics {
    pub fn new() -> Self {
        Self {
            amount_runs: 0,
            amount_sat: 0,
            amount_unsat: 0,
            amount_unknown: 0,
            average_conflicts: RunningAverage::default(),
            average_iteration_duration: RunningAverageDuration::default(),
            errors: 0,
            timeouts: 0,
            large_solving_difference: 0,
            stats_per_solver: HashMap::new(),
        }
    }

    pub fn update(&mut self, single_run: SingleRunStats) {
        match single_run {
            SingleRunStats::Successfull(record) => {
                let SingleRunRecord {
                    result,
                    conflicts,
                    iteration_duration,
                    large_solving_time_difference,
                    solver_states,
                } = record;
                match result {
                    SimpleResult::Sat => self.amount_sat += 1,
                    SimpleResult::Unsat => self.amount_unsat += 1,
                    SimpleResult::Unknown => self.amount_unknown += 1,
                    SimpleResult::Timeout => self.timeouts += 1,
                    SimpleResult::Optimum => self.amount_sat += 1,
                    SimpleResult::Errors(n) => {
                        self.errors += n;
                    }
                    SimpleResult::Other => self.amount_unknown += 1,
                }
                for c in conflicts {
                    self.average_conflicts.update(c);
                }
                self.average_iteration_duration.update(&iteration_duration);
                if large_solving_time_difference {
                    self.large_solving_difference += 1;
                }
                for (name, state) in solver_states {
                    let solver_stat = self.stats_per_solver.entry(name).or_default();
                    match state {
                        SolverStateAfterIteration::Failure(failure_type) => {
                            solver_stat.failures.add_failure(failure_type)
                        }
                        SolverStateAfterIteration::Timeout => solver_stat.timeouts += 1,
                        SolverStateAfterIteration::Expected => solver_stat.expected += 1,
                    }
                }
            }
            SingleRunStats::Error => self.errors += 1,
        };
        self.amount_runs += 1;
    }

    pub fn get_solver_stats(&self) -> impl Iterator<Item = (&String, &SolverStats)> {
        self.stats_per_solver.iter()
    }

    fn percentage_sat(&self) -> f32 {
        let sat_unsat = self.amount_sat + self.amount_unsat;
        if sat_unsat == 0 {
            return 0.0;
        }
        self.amount_sat as f32 / sat_unsat as f32
    }
}

impl Display for RunStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let RunStatistics {
            amount_runs,
            amount_sat,
            amount_unsat,
            amount_unknown,
            average_conflicts,
            average_iteration_duration,
            errors,
            timeouts,
            large_solving_difference,
            stats_per_solver,
        } = &self;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Total amount of instances generated", amount_runs
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Generated amount of SAT instances", amount_sat
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Generated amount of UNSAT instances", amount_unsat
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Generated amount of unknown instances", amount_unknown
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}%",
            "Percentage SAT",
            self.percentage_sat() * 100.0
        )?;
        writeln!(
            f,
            "{:<OFFSET$}: {}",
            "Average conflicts to solve",
            average_conflicts.get_average()
        )?;
        writeln!(f, "{:<OFFSET$}: {}", "Errors occured", errors)?;
        writeln!(f, "{:<OFFSET$}: {}", "Timeouts", timeouts)?;
        writeln!(
            f,
            "{:<OFFSET$}: {:.3}ms",
            "Average Time per Instance",
            average_iteration_duration.get_average().as_secs_f32() * 1_000.0,
        )?;
        writeln!(f, "Stats for each solver:")?;
        for (name, stats) in stats_per_solver {
            writeln!(
                f,
                "{:<OFFSET$}: {} failures, {} timeouts",
                name,
                stats.failures.total_failures(),
                stats.timeouts
            )?;
        }
        write!(
            f,
            "{:<OFFSET$}: {}",
            "Runs with large solving time difference", large_solving_difference
        )
    }
}
