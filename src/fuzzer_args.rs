use clap::Parser;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct FuzzerArgs {
    #[arg(long)]
    pub seed: u64,
    #[arg(long)]
    pub output_path: String,
    #[arg(
        long,
        default_value_t = 3,
        value_parser = clap::value_parser!(u32).range(1..),
        help = "Number of constraints used (must be > 0)"
    )]
    pub amount_vars: u32,
    #[arg(
        long,
        default_value_t = 10,
        value_parser = clap::value_parser!(u32).range(1..),
        help = "Number of variables used (must be > 0)"
    )]
    pub amount_constraints: u32,
}
