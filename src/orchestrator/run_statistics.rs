use crate::fuzzer::helpers::OFFSET;
use std::{collections::HashMap, fmt::Display, time::Duration};

use serde::Serialize;

use crate::fuzzer::helpers::{RunningAverage, RunningAverageDuration};

#[derive(Serialize)]
pub struct RunStatistics {
    amount_runs: u32,
    amount_sat: u32,
    amount_unsat: u32,
    amount_unknown: u32,
    average_conflicts: RunningAverage,
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
    pub failures: u32,
}

pub enum SolverStateAfterIteration {
    Failure,
    Timeout,
    Expected,
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
                        SolverStateAfterIteration::Failure => solver_stat.failures += 1,
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
                name, stats.failures, stats.timeouts
            )?;
        }
        write!(
            f,
            "{:<OFFSET$}: {}",
            "Runs with large solving time difference", large_solving_difference
        )
    }
}
