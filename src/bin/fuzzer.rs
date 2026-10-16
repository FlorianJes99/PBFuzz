use std::error::Error;
use std::fs::File;
use std::io::BufWriter;

use clap::Parser;
use fuzzer_pseudo_bool::fuzzer::app_info::MetaData;
use fuzzer_pseudo_bool::fuzzer::fuzzer_args::FuzzerArgs;
use fuzzer_pseudo_bool::fuzzer::helpers::current_time_as_timestamp;
use fuzzer_pseudo_bool::fuzzer::helpers::print_block;
use fuzzer_pseudo_bool::fuzzer::pb_generator::PBGenerator;
use fuzzer_pseudo_bool::fuzzer::pb_settings::PbConfigFile;
use log::Level;
use log::info;

fn main() -> Result<(), Box<dyn Error>> {
    let program_version = env!("CARGO_PKG_VERSION");
    let git_hash = option_env!("GIT_HASH");
    println!("Program version: {program_version}");
    if let Some(hash) = git_hash {
        println!("Program commit: {hash}");
    }

    // initialize the logger
    init_logger()?;
    // start of actual program
    let args = FuzzerArgs::parse();

    let meta_data = MetaData {
        program_version: program_version.into(),
        commit_hash: git_hash.map(Into::into),
    };
    let config = PbConfigFile::load_from_file(&args.settings_file)?;
    if config.generation_method.is_none() {
        return Err(
            "When running the fuzzer standalone, generation method must be set in config file!"
                .into(),
        );
    }
    let grammar = PBGenerator::create_grammar(args.seed, config.generate_settings(), meta_data)?;
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
