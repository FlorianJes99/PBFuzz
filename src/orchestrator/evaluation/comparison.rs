use crate::{
    fuzzer::grammar::PBGrammar,
    orchestrator::{
        evaluation::common::{BugReason, VerificationOutcome, wrap_objective, wrap_violation},
        parsing::parser::{ParsedStatus, ParserRun},
    },
};

pub struct ComparisonRun {
    pub parser_run1: ParserRun,
    pub parser_run2: ParserRun,
    pub outcome: VerificationOutcome,
}

pub fn compare_runs(
    parser_run1: ParserRun,
    parser_run2: ParserRun,
    grammar: &PBGrammar,
) -> ComparisonRun {
    let outcome = compare_status(&parser_run1, &parser_run2, grammar);
    ComparisonRun {
        parser_run1,
        parser_run2,
        outcome,
    }
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
            if let Err(e) = grammar.verify_solution(s1) {
                return VerificationOutcome::Bug(wrap_violation(e, r1.record.name.clone()));
            }

            if let Err(e) = grammar.verify_solution(s2) {
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
            if let Err(e) = grammar.verify_solution(s1) {
                return VerificationOutcome::Bug(wrap_violation(e, r1.record.name.clone()));
            }

            if let Err(e) = grammar.verify_solution(s2) {
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
