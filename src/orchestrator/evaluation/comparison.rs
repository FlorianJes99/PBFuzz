use crate::{
    fuzzer::grammar::PBGrammar,
    orchestrator::{
        evaluation::common::{
            wrap_objective, wrap_violation, BugReason, VerificationData, VerificationOutcome,
        },
        parsing::parser::{ParsedStatus, ParserRun},
    },
};

pub struct ComparisonRun {
    pub v1: VerificationData,
    pub v2: VerificationData,
    pub outcome: VerificationOutcome,
}

pub fn compare_runs(r1: ParserRun, r2: ParserRun, grammar: &PBGrammar) -> ComparisonRun {
    let outcome = compare_status(&r1, &r2, grammar);
    let v1 = VerificationData {
        solver_data: r1.record,
        parser_data: r1.parsed_status,
    };
    let v2 = VerificationData {
        solver_data: r2.record,
        parser_data: r2.parsed_status,
    };
    ComparisonRun { v1, v2, outcome }
}

fn compare_status(r1: &ParserRun, r2: &ParserRun, grammar: &PBGrammar) -> VerificationOutcome {
    match (&r1.parsed_status, &r2.parsed_status) {
        (
            ParsedStatus::Sat {
                conflicts: _,
                solution: s1,
            },
            ParsedStatus::Sat {
                conflicts: _,
                solution: s2,
            },
        ) => {
            if let Err(e) = grammar.verify_solution(&s1) {
                return VerificationOutcome::Bug(wrap_violation(e, r1.record.name.clone()));
            }

            if let Err(e) = grammar.verify_solution(&s2) {
                return VerificationOutcome::Bug(wrap_violation(e, r2.record.name.clone()));
            }
            VerificationOutcome::Sat
        }
        (
            ParsedStatus::Optimum {
                conflicts: _,
                solution: s1,
                minimum: m1,
            },
            ParsedStatus::Optimum {
                conflicts: _,
                solution: s2,
                minimum: m2,
            },
        ) => {
            if let Err(e) = grammar.verify_solution(&s1) {
                return VerificationOutcome::Bug(wrap_violation(e, r1.record.name.clone()));
            }

            if let Err(e) = grammar.verify_solution(&s2) {
                return VerificationOutcome::Bug(wrap_violation(e, r2.record.name.clone()));
            }
            if m1 != m2 {
                return VerificationOutcome::Bug(BugReason::DifferentObjectives(
                    wrap_objective(*m1, r1.record.name.clone()),
                    wrap_objective(*m2, r2.record.name.clone()),
                ));
            }
            VerificationOutcome::Sat
        }
        (ParsedStatus::Unsat { .. }, ParsedStatus::Unsat { .. }) => VerificationOutcome::Unsat,
        (ParsedStatus::Unknown { .. }, ParsedStatus::Unknown { .. }) => {
            VerificationOutcome::Unknown
        }
        (ParsedStatus::TimeLimit, _) => VerificationOutcome::Timelimit,
        (_, ParsedStatus::TimeLimit) => VerificationOutcome::Timelimit,
        _ => VerificationOutcome::Bug(BugReason::OutputStatusDiffers(
            get_output_status_str(&r1.parsed_status),
            get_output_status_str(&r2.parsed_status),
        )),
    }
}

fn get_output_status_str(status: &ParsedStatus) -> String {
    match status {
        ParsedStatus::Sat { .. } => "SAT",
        ParsedStatus::Optimum { .. } => "OPTIMUM FOUND",
        ParsedStatus::Unsat { .. } => "UNSAT",
        ParsedStatus::TimeLimit => "TIMELIMIT",
        ParsedStatus::Unknown { .. } => "UNKNOWN",
    }
    .into()
}

pub enum ExpectedResult {
    Sat,
    Unsat,
    Timelimit,
}
