use std::path::Path;

use log::error;
use veripb::{self, args::Args, run_checker};

pub fn check_proof(formula: &Path, derivation: &Path) -> bool {
    assert!(
        formula.exists(),
        "Expected formula at path {} to exist",
        formula.display()
    );
    assert!(
        derivation.exists(),
        "Expected proof derivation at path {} to exist",
        derivation.display()
    );
    let args = Args {
        formula: formula.to_path_buf(),
        derivation: derivation.to_path_buf(),
        ..Default::default()
    };
    match run_checker(args) {
        Ok(_) => true,
        Err(e) => {
            error!("{e}");
            false
        }
    }
}
