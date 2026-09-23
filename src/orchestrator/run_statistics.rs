use crate::fuzzer::helpers::OFFSET;
use std::{fmt::Display, time::Duration};

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
    bugs_found: u32,
    errors: u32,
    timeouts: u32,
    large_solving_difference: u32,
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
    Bug,
    Error,
    BugError,
    Unknown,
    Other,
}

pub struct SingleRunRecord {
    result: SimpleResult,
    conflicts: Vec<u32>,
    iteration_duration: Duration,
    large_solving_time_difference: bool,
}

impl SingleRunRecord {
    pub fn new(
        result: SimpleResult,
        conflicts: Vec<u32>,
        iteration_duration: Duration,
        large_solving_time_difference: bool,
    ) -> Self {
        Self {
            result,
            conflicts,
            iteration_duration,
            large_solving_time_difference,
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
            bugs_found: 0,
            errors: 0,
            timeouts: 0,
            large_solving_difference: 0,
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
                } = record;
                match result {
                    SimpleResult::Sat => self.amount_sat += 1,
                    SimpleResult::Unsat => self.amount_unsat += 1,
                    SimpleResult::Unknown => self.amount_unknown += 1,
                    SimpleResult::Timeout => self.timeouts += 1,
                    SimpleResult::Bug => self.bugs_found += 1,
                    SimpleResult::Optimum => self.amount_sat += 1,
                    SimpleResult::Error => self.errors += 1,
                    SimpleResult::BugError => {
                        self.errors += 1;
                        self.bugs_found += 1;
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
            }
            SingleRunStats::Error => self.errors += 1,
        };
        self.amount_runs += 1;
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
            bugs_found,
            errors,
            timeouts,
            large_solving_difference,
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
        writeln!(f, "{:<OFFSET$}: {}", "Bugs found", bugs_found)?;
        writeln!(f, "{:<OFFSET$}: {}", "Errors occured", errors)?;
        writeln!(f, "{:<OFFSET$}: {}", "Timeouts", timeouts)?;
        writeln!(
            f,
            "{:<OFFSET$}: {:.3}ms",
            "Average Time per Instance",
            average_iteration_duration.get_average().as_secs_f32() * 1_000.0,
        )?;
        write!(
            f,
            "{:<OFFSET$}: {}",
            "Runs with large solving time difference", large_solving_difference
        )
    }
}
