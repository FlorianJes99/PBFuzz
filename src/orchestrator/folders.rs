use std::io;
use std::{
    fs::create_dir,
    io::{Error, ErrorKind},
    path::Path,
};

pub struct FolderConfig {
    pub name: &'static str,
}

#[rustfmt::skip]
mod folders_config {
    use super::FolderConfig;
    pub const CASES_FOLDER: FolderConfig   = FolderConfig { name: "cases",   };
    pub const DATA_FOLDER: FolderConfig    = FolderConfig { name: "data",    };
    pub(super) const FOLDERS: [FolderConfig; 2] = [CASES_FOLDER, DATA_FOLDER];
}

pub use folders_config::*;
use log::info;

/// Call this function once in the beginning of a run to initialize the workspace
///
/// Creates all subfolder needed for the orchestrator.
/// Returns: The directory used for the current fuzzer run
/// Args:
/// `output_path`: base path where all runs should be saved into
/// `time_stamp`: used to create subfolder including the timestamp of the start of the run
/// `pb_kind`: include PB kind in directory name
pub fn prepare_sub_folders(output_path: &Path) -> io::Result<()> {
    if output_path.exists() && output_path.is_file() {
        return Err(Error::new(
            ErrorKind::NotADirectory,
            format!(
                "Current workspace needs to be a directory, but was: {}",
                output_path.display()
            ),
        ));
    }
    if !output_path.exists() {
        info!("Creating base directory: {}", output_path.display());
        create_dir(output_path)?;
    }
    create_folders(output_path)?;
    Ok(())
}

/// Simply create all folders in the current working directory which are needed for the
/// orchestrator to output different files.
///
/// Deletion of some folders should be done before calling this method
fn create_folders(output_path: &Path) -> io::Result<()> {
    assert!(
        output_path.exists() && output_path.is_dir(),
        "Expected that output directory ({}) should already exist!",
        output_path.display()
    );

    for folder_config in FOLDERS {
        let folder = output_path.join(folder_config.name);
        if !folder.exists() {
            info!("Creating folder: {}", folder.display());
            create_dir(folder)?;
        }
    }
    Ok(())
}
