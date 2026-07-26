use std::error::Error;
use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use fuzzer_pseudo_bool::fuzzer::app_info::MetaData;
use fuzzer_pseudo_bool::fuzzer::fuzzer_args::FuzzerArgs;
use fuzzer_pseudo_bool::fuzzer::helpers::current_time_as_timestamp;
use fuzzer_pseudo_bool::fuzzer::helpers::print_block;
use fuzzer_pseudo_bool::fuzzer::pb_generator::PBGenerator;
use fuzzer_pseudo_bool::fuzzer::pb_settings::GeneratorSettingsBuilder;
use log::info;
use log::Level;

fn main() -> Result<(), Box<dyn Error>> {
    let program_version = env!("CARGO_PKG_VERSION");
    let git_hash = env!("GIT_HASH");
    println!("Program version: {program_version}");
    println!("Program commit: {git_hash}");

    // initialize the logger
    init_logger()?;
    // start of actual program
    let args = FuzzerArgs::parse();

    let mut settings = match args.settings_file {
        Some(file) => GeneratorSettingsBuilder::load_from_file(&file).unwrap_or_else(|e| {
            println!("Error occured while loading settings: {}", e);
            println!("Using default settings...");
            GeneratorSettingsBuilder::default()
        }),
        None => GeneratorSettingsBuilder::default(),
    };

    if let Some(mode) = args.mode {
        settings = settings.pb_kind(mode);
    }
    let meta_data = MetaData {
        program_version: program_version.into(),
        commit_hash: git_hash.into(),
    };
    let grammar = PBGenerator::create_grammar(args.seed, settings.build(), meta_data)?;
    if args.print_statistics {
        print_block("Grammar Statistics", grammar.get_statistics());
    }
    let output_path = &args.output_path;
    let write_duration = File::create(output_path)
        .map(BufWriter::new)
        .and_then(|mut buf_writer| grammar.produce(&mut buf_writer))?;
    info!(
        "Wrote generated grammar in {}s into file: {}",
        write_duration.as_secs_f32(),
        output_path
    );
    Ok(())
}

fn init_logger() -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                current_time_as_timestamp(),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(Level::Info.to_level_filter())
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}
