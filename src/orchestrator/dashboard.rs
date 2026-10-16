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
    solvers_pb: HashMap<String, ProgressBar>,
    total_pb: ProgressBar,
}

pub struct IterationInfo {
    pub total: u32,
    pub amount_random: u32,
    pub amount_planted: u32,
}

impl Dashboard {
    pub fn new(mp: MultiProgress, meta_data: MetaData, solver_names: &[&str]) -> Self {
        let create_pb = || {
            let pb = mp.add(ProgressBar::new_spinner());
            pb.set_style(ProgressStyle::with_template("{msg}").expect("invalid template provided"));
            pb
        };
        let mut seps = vec![];
        let program_info_pb = create_pb();
        program_info_pb.finish_with_message(format!("PBFuzz - {}", meta_data));
        seps.push(program_info_pb);
        let status_pb = create_pb();
        let iterations_pb = create_pb();
        seps.push(create_empty_line(&mp));
        let solver_header = create_pb();
        solver_header.finish_with_message(create_column("solver", "bug count", "timeouts"));
        let sep = create_pb();
        sep.finish_with_message("-".repeat(TABLE_W));
        seps.push(sep);
        let solvers_pb = solver_names
            .iter()
            .map(|s| (s.to_string(), create_pb()))
            .collect();
        let sep = create_pb();
        sep.finish_with_message("-".repeat(TABLE_W));
        seps.push(sep);
        let total_pb = create_pb();
        Self {
            _mp: mp,
            _seperators: seps,
            iterations_pb,
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
        let total = iterations.total;
        let throughput = total as f64 / elapsed.as_secs_f64().max(0.001);
        let pct = |n: u32| {
            if total > 0 {
                n as f64 * 100.0 / total as f64
            } else {
                0.0
            }
        };
        self.iterations_pb.set_message(format!(
            "Iterations {total}  ({throughput:.2}/s)   random {:.0}%  planted {:.0}%",
            pct(iterations.amount_random),
            pct(iterations.amount_planted)
        ));
        let state = if user_interrupted {
            "User interrupted! Terminating gracefully...".to_string()
        } else if workers_active > 0 {
            "Fuzzing active".to_string()
        } else {
            "Workers done! Aggregating results...".to_string()
        };
        let (h, m, s) = get_hms_parts(elapsed);
        self.status_pb.set_message(format!(
            "{h:02}:{m:02}:{s:02} | {workers_active} worker threads | {state}"
        ));

        let (mut failures_total, mut timeouts_total) = (0u32, 0u32);
        for (name, stats) in solver_stats {
            let SolverStats {
                expected: _,
                timeouts,
                failures,
            } = stats;
            failures_total += failures;
            timeouts_total += timeouts;
            match self.solvers_pb.get(name) {
                Some(pb) => pb.set_message(create_column(name, failures, timeouts)),
                None => warn!("Tried to update non existing solver: {name}"),
            }
        }
        self.total_pb
            .set_message(create_column("total", failures_total, timeouts_total));
    }

    pub fn finish(&self, elapsed: &Duration) {
        let Dashboard {
            _mp,
            _seperators,
            iterations_pb,
            status_pb,
            solvers_pb,
            total_pb,
        } = self;
        total_pb.finish();
        let (h, m, s) = get_hms_parts(elapsed);
        status_pb.finish_with_message(format!("{h:02}:{m:02}:{s:02} | Done!"));
        iterations_pb.finish();
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

fn create_column(solver_name: &str, bug_count: impl Display, timeouts: impl Display) -> String {
    format!("{:<20}{:>10}{:>10}", solver_name, bug_count, timeouts)
}
