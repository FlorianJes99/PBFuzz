use crate::orchestrator::{
    evaluation::common::{BugReason, ObjectiveInfo, wrap_objective},
    parsing::parser::{ParsedStatus, ParserRun},
};

pub fn compare_status<'a>(runs: &[&'a ParserRun]) -> Vec<(&'a ParserRun, BugReason)> {
    if runs.len() <= 1 {
        // We need at least two finished solvers in order to compare them
        return vec![];
    }
    let mut comparison_failures = vec![];

    let any_sat = runs.iter().any(|r| r.parsed_status.is_sat());
    let any_unsat = runs.iter().any(|r| r.parsed_status.is_unsat());
    let any_decided = any_sat || any_unsat;
    let best_objective = runs
        .iter()
        .filter_map(|r| {
            extract_objective(&r.parsed_status, r.record.name.to_string()).map(|o| o.get_obj())
        })
        .min();
    for run in runs {
        // compare status
        match run.parsed_status {
            ParsedStatus::Unsat { .. } if any_sat => {
                comparison_failures.push((
                    *run,
                    BugReason::WrongUnsat {
                        unsat_solver: run.record.name.to_string(),
                    },
                ));
            }
            ParsedStatus::Unknown { .. } if any_decided => comparison_failures.push((
                *run,
                BugReason::UnknownWhileDecidable {
                    unknown_solver: run.record.name.to_string(),
                },
            )),
            _ => {}
        }

        // compare objectives
        if let Some(best_obj) = best_objective
            && let Some(r_obj) = extract_objective(&run.parsed_status, run.record.name.to_string())
            && r_obj.get_obj() > best_obj
        {
            comparison_failures.push((
                *run,
                BugReason::DifferentObjectives {
                    claimed: r_obj,
                    better: best_obj,
                },
            ));
        }
    }
    comparison_failures
}

fn extract_objective(status: &ParsedStatus, solver_name: String) -> Option<ObjectiveInfo> {
    match status {
        ParsedStatus::Optimum {
            conflicts: _,
            solution: _,
            minimum,
        } => Some(wrap_objective(*minimum, solver_name)),
        _ => None,
    }
}
