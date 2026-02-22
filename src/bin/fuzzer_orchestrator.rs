use std::{
    fs,
    io::{Error, ErrorKind},
    path::PathBuf,
};

use clap::Parser;
use fuzzer_pseudo_bool::{
    fuzzer::{grammar::PBKind, pbs_generator::GeneratorSettingsBuilder},
    orchestrator::{
        roundingsat::Roundingsat,
        runner::{OrchestratorInput, run_orchestrator},
        scip::Scip,
    },
};

fn main() {
    env_logger::init();
    let args = OrchestratorArgs::parse();
    let rounding_sat = Roundingsat::new("roundingsat".into());
    let scip = Scip::new("software/scip".into());
    let mut settings = match args.settings_file {
        Some(file) => load_settings(file).unwrap_or_else(|e| {
            println!("Error occured while loading settings: {}", e);
            println!("Using default settings...");
            GeneratorSettingsBuilder::default()
        }),
        None => GeneratorSettingsBuilder::default(),
    };

    if let Some(mode) = args.mode {
        settings = settings.pb_kind(mode);
    }
    let res = run_orchestrator(OrchestratorInput::new(
        args.iterations,
        args.ouput_dir,
        settings.build(),
        rounding_sat,
        scip,
    ));
    if res.is_err() {
        println!("{}", res.unwrap_err())
    }
}

fn load_settings(file: PathBuf) -> std::io::Result<GeneratorSettingsBuilder> {
    let data = fs::read_to_string(file)?;
    let settings_builder = ron::from_str(&data)
        .map_err(|e| Error::new(ErrorKind::Other, e))
        .map(|settings| GeneratorSettingsBuilder::from_settings(settings))?;
    Ok(settings_builder)
}

/// Arguments that can be passed to the orchestration pipeline for testing multiple solvers against
/// there outputs.
/// IMPORTANT: if both settings file and parameters are provided, then parameters will override
/// settings specified in the file
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct OrchestratorArgs {
    #[arg(short, long)]
    ouput_dir: PathBuf,
    #[arg(short, long, default_value_t = 1000)]
    iterations: u32,
    /// Choose mode for Fuzzer
    #[arg(long, value_enum)]
    mode: Option<PBKind>,
    /// instead of providing arguments via cli all settings can be loaded from a dedicated settings
    /// file
    #[arg(long)]
    settings_file: Option<PathBuf>,
}
