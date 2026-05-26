use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use fuzzer_pseudo_bool::fuzzer::fuzzer_args::FuzzerArgs;
use fuzzer_pseudo_bool::fuzzer::helpers::current_time_as_timestamp;
use fuzzer_pseudo_bool::fuzzer::helpers::print_block;
use fuzzer_pseudo_bool::fuzzer::pbs_generator::GeneratorSettingsBuilder;
use fuzzer_pseudo_bool::fuzzer::pbs_generator::PBSGenerator;
use log::Level;
use log::error;
use log::info;

fn main() {
    // initialize the logger
    if let Err(e) = init_logger() {
        println!("Logger initialization failed\n Error: {}", e);
        return;
    }

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
    let grammar = PBSGenerator::create_grammar(args.seed, settings.build());
    if args.print_statistics {
        print_block("Grammar Statistics", grammar.get_statistics());
    }
    let output_path = &args.output_path;
    let write_result = File::create(output_path)
        .map(BufWriter::new)
        .map(|mut buf_writer| grammar.produce(&mut buf_writer));
    match write_result {
        Ok(_) => info!("Wrote generated grammar into file: {}", output_path),
        Err(e) => error!("Could not write into file: {}\n Error: {}", output_path, e),
    }
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
