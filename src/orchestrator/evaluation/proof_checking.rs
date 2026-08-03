use crate::{
    fuzzer::grammar::PBGrammar,
    orchestrator::{
        evaluation::common::{wrap_violation, BugReason, VerificationData, VerificationOutcome},
        parsing::parser::{ParsedStatus, ParserRun},
    },
};

use std::path::{Path, PathBuf};

use thiserror::Error;
use veripb::{self, args::Args, run_checker};

pub struct ProofRun {
    pub v: VerificationData,
    pub outcome: VerificationOutcome,
}

pub fn verify_with_proof(
    r: ParserRun,
    grammar: &PBGrammar,
    formula: &Path,
    derivation: &Path,
) -> ProofRun {
    let outcome = match &r.parsed_status {
        ParsedStatus::Sat {
            conflicts: _,
            solution,
        }
        | ParsedStatus::Optimum {
            conflicts: _,
            solution,
            ..
        } => match grammar.verify_solution(&solution) {
            Err(e) => VerificationOutcome::Bug(wrap_violation(e, r.record.name.clone())),
            Ok(()) => match check_proof(formula, derivation) {
                Err(e) => VerificationOutcome::Bug(BugReason::ProofCheckerFailed(e)),
                Ok(()) => VerificationOutcome::Sat,
            },
        },
        ParsedStatus::Unsat { conflicts: _ } => match check_proof(formula, derivation) {
            Err(e) => VerificationOutcome::Bug(BugReason::ProofCheckerFailed(e)),
            Ok(()) => VerificationOutcome::Unsat,
        },
        ParsedStatus::TimeLimit => VerificationOutcome::Timelimit,
        ParsedStatus::Unknown { status: _ } => VerificationOutcome::Unknown,
    };
    let v = VerificationData {
        solver_data: r.record,
        parser_data: r.parsed_status,
    };
    ProofRun { v, outcome }
}

pub fn check_proof(formula: &Path, derivation: &Path) -> Result<(), ProofError> {
    let formula = formula.to_path_buf();
    let derivation = derivation.to_path_buf();
    if !formula.exists() {
        return Err(ProofError::FileDoesNotExist("formula".into(), formula));
    }
    if !derivation.exists() {
        return Err(ProofError::FileDoesNotExist("proof".into(), derivation));
    }
    let args = Args {
        formula,
        derivation,
        ..Default::default()
    };
    run_checker(args).map_err(|e| ProofError::VeriPB(e.into_boxed_dyn_error()))
}

#[derive(Error, Debug)]
pub enum ProofError {
    #[error("Expected {0} at path {p} to exist", p = (.1).display())]
    FileDoesNotExist(String, PathBuf),
    #[error(transparent)]
    VeriPB(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
}
