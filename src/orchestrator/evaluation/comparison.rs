use crate::orchestrator::{
    evaluation::common::{BugReason, ObjectiveInfo, wrap_objective},
    parsing::parser::{ParsedStatus, ParserRun},
};

pub fn compare_status(runs: &[ParserRun]) -> Result<(), BugReason> {
    let useful_runs: Vec<_> = runs
        .iter()
        .filter(|r| !matches!(r.parsed_status, ParsedStatus::TimeLimit))
        .collect();
    if useful_runs.len() <= 1 {
        // We need at least two finished solvers in order to compare them
        return Ok(());
    }
    let status = &useful_runs[0].parsed_status;
    let status_as_str = status.get_status_str();
    let stati: Vec<_> = useful_runs
        .iter()
        .map(|r| r.parsed_status.get_status_str().to_string())
        .collect();
    if stati.iter().any(|s| s != status_as_str) {
        return Err(BugReason::OutputStatusDiffers(stati));
    };
    let objs: Vec<_> = useful_runs
        .iter()
        .filter_map(|r| extract_objective(&r.parsed_status, r.record.name.to_string()))
        .collect();
    if !objs.is_empty() {
        let obj = objs[0].get_obj();
        if !objs.iter().all(|o| o.get_obj() == obj) {
            return Err(BugReason::DifferentObjectives(objs));
        }
    }
    Ok(())
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
