use std::path::Path;

use log::error;
use veripb::{self, args::Args, run_checker};

pub fn check(formula: &Path, derivation: &Path) -> bool {
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
