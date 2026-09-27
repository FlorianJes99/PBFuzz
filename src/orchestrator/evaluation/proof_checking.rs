use std::path::{Path, PathBuf};

use thiserror::Error;
use veripb::{self, args::Args, run_checker};

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

impl ProofError {
    pub fn short_str(&self) -> &'static str {
        match self {
            ProofError::FileDoesNotExist(_, _) => "no_proof_file",
            ProofError::VeriPB(_) => "proof_invalid",
        }
    }
}
