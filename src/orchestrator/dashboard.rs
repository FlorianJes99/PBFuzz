use crate::{fuzzer::app_info::MetaData, orchestrator::run_statistics::SolverStats};
use std::{collections::HashMap, fmt::Display, time::Duration};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::warn;

use crate::fuzzer::helpers::get_hms_parts;

const NAME_W: usize = 20;
const COL_W: usize = 10;
const TABLE_W: usize = NAME_W + 2 * COL_W;

pub struct Dashboard {
    _mp: MultiProgress,
    _seperators: Vec<ProgressBar>,
    status_pb: ProgressBar,
    iterations_pb: ProgressBar,
    sat_unsat_pb: ProgressBar,
    solvers_pb: HashMap<String, ProgressBar>,
    total_pb: ProgressBar,
}

pub struct IterationInfo {
    pub total: u32,
    pub amount_random: u32,
    pub amount_planted: u32,
    pub amount_sat: u32,
    pub amount_unsat: u32,
    pub average_conflicts: f32,
}

impl Dashboard {
    pub fn new(mp: MultiProgress, meta_data: MetaData, solver_names: &[&str]) -> Self {
        let create_pb = |templ: &str| {
            let pb = mp.add(ProgressBar::new_spinner());
            pb.set_style(ProgressStyle::with_template(templ).expect("invalid template provided"));
            pb
        };
        let mut seps = vec![];
        let program_info_pb = create_pb("{msg:.red}");
        program_info_pb.finish_with_message(format!("PBFuzz - {}", meta_data));
        seps.push(program_info_pb);
        let status_pb = create_pb("{msg:.green}");
        let iterations_pb = create_pb("{msg}");
        let sat_unsat_pb = create_pb("{msg}");
        seps.push(create_empty_line(&mp));
        let solver_header = create_pb("{msg}");
        solver_header.finish_with_message(create_column("solver", "failures", "timeouts"));
        let sep = create_pb("{msg}");
        sep.finish_with_message("-".repeat(TABLE_W));
        seps.push(sep);
        let solvers_pb = solver_names
            .iter()
            .map(|s| (s.to_string(), create_pb("{msg}")))
            .collect();
        let sep = create_pb("{msg}");
        sep.finish_with_message("-".repeat(TABLE_W));
        seps.push(sep);
        let total_pb = create_pb("{msg:.bold}");
        Self {
            _mp: mp,
            _seperators: seps,
            iterations_pb,
            sat_unsat_pb,
            status_pb,
            solvers_pb,
            total_pb,
        }
    }

    pub fn update<'a>(
        &self,
        elapsed: &Duration,
        iterations: &IterationInfo,
        workers_active: u8,
        user_interrupted: bool,
        solver_stats: impl Iterator<Item = (&'a String, &'a SolverStats)>,
    ) {
        let Dashboard {
            _mp,
            _seperators,
            status_pb,
            iterations_pb,
            sat_unsat_pb,
            solvers_pb,
            total_pb,
        } = self;
        let state = if user_interrupted {
            format!(
                "User interrupted! Terminating gracefully... ({workers_active} Workers still active)"
            )
        } else if workers_active > 0 {
            format!("Fuzzing ({workers_active} workers)")
        } else {
            "Workers done, processing results!".to_string()
        };
        let (h, m, s) = get_hms_parts(elapsed);
        status_pb.set_message(format!("[{h:02}:{m:02}:{s:02}] {state}"));
        let total = iterations.total;
        let throughput = total as f64 / elapsed.as_secs_f64().max(0.001);
        let pct = |n: u32| {
            if total > 0 {
                n as f64 * 100.0 / total as f64
            } else {
                0.0
            }
        };
        iterations_pb.set_message(format!(
            "Iterations {total}  ({throughput:.2}/s)   random {:.0}%  planted {:.0}%",
            pct(iterations.amount_random),
            pct(iterations.amount_planted)
        ));

        let sat = iterations.amount_sat;
        let unsat = iterations.amount_unsat;
        let total = sat + unsat;
        let sat_percentage = if total == 0 {
            0.0
        } else {
            (sat as f32 / total as f32) * 100.0
        };
        sat_unsat_pb.set_message(format!(
            "SAT: {}   UNSAT: {}   ({:.2}% SAT)   Average conflicts: {:.2}",
            sat, unsat, sat_percentage, iterations.average_conflicts
        ));

        let (mut failures_total, mut timeouts_total) = (0u32, 0u32);
        for (name, stats) in solver_stats {
            let SolverStats {
                expected: _,
                timeouts,
                failures,
            } = stats;
            failures_total += failures.total_failures();
            timeouts_total += timeouts;
            match solvers_pb.get(name) {
                Some(pb) => {
                    pb.set_message(create_column(name, failures.total_failures(), timeouts))
                }
                None => warn!("Tried to update non existing solver: {name}"),
            }
        }
        total_pb.set_message(create_column("total", failures_total, timeouts_total));
    }

    pub fn finish(&self, elapsed: &Duration) {
        let Dashboard {
            _mp,
            _seperators,
            iterations_pb,
            sat_unsat_pb,
            status_pb,
            solvers_pb,
            total_pb,
        } = self;
        total_pb.finish();
        let (h, m, s) = get_hms_parts(elapsed);
        status_pb.finish_with_message(format!("{h:02}:{m:02}:{s:02} | Done!"));
        iterations_pb.finish();
        sat_unsat_pb.finish();
        for pb in solvers_pb.values() {
            pb.finish();
        }
    }
}

fn create_empty_line(mp: &MultiProgress) -> ProgressBar {
    let pb = mp.add(ProgressBar::new_spinner());
    pb.set_style(ProgressStyle::with_template("{msg}").expect("invalid template provided"));
    pb.finish_with_message(" ");
    pb
}

fn create_column(solver_name: &str, failures: impl Display, timeouts: impl Display) -> String {
    format!("{:<20}{:>10}{:>10}", solver_name, failures, timeouts)
}
