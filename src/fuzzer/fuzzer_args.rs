use std::path::PathBuf;

use clap::Parser;

use crate::fuzzer::grammar::PBKind;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct FuzzerArgs {
    /// Seed used for reproducible runs
    #[arg(long)]
    pub seed: u64,
    /// Destination to write generated grammar into
    #[arg(long)]
    pub output_path: String,
    // Optional arguments
    /// Option to print statistics about a single fuzzer run
    #[arg(long, default_value_t = false)]
    pub print_statistics: bool,
    /// Choose mode for Fuzzer (default: PBS)
    #[arg(long, value_enum)]
    pub mode: Option<PBKind>,
    /// Specify a file with settings for the fuzzer
    #[arg(long)]
    pub settings_file: Option<PathBuf>,
}
