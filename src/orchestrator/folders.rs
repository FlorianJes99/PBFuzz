use std::io;
use std::{
    fs::{create_dir, read_dir, remove_file},
    io::{Error, ErrorKind},
    path::Path,
};

pub struct FolderConfig {
    pub name: &'static str,
    pub should_delete: bool,
}

#[rustfmt::skip]
mod folders_config {
    use super::FolderConfig;
    pub const SAT_FOLDER: FolderConfig     = FolderConfig { name: "sat",     should_delete: true  };
    pub const UNSAT_FOLDER: FolderConfig   = FolderConfig { name: "unsat",   should_delete: true  };
    pub const UNKNOWN_FOLDER: FolderConfig = FolderConfig { name: "unknown", should_delete: true  };
    pub const BUG_FOLDER: FolderConfig     = FolderConfig { name: "bug",     should_delete: false };
    pub const ERROR_FOLDER: FolderConfig   = FolderConfig { name: "error",   should_delete: false };
    pub const DATA_FOLDER: FolderConfig    = FolderConfig { name: "data",    should_delete: false };
    pub const FOLDERS: [FolderConfig; 6] = 
        [SAT_FOLDER, UNSAT_FOLDER, UNKNOWN_FOLDER, BUG_FOLDER, ERROR_FOLDER, DATA_FOLDER];
}

pub use folders_config::*;
use log::info;

/// Call this function once in the beginning of a run to initialize the workspace
///
/// Creates all subfolder needed for the orchestrator
pub fn prepare_output_folder(output_path: &Path) -> io::Result<()> {
    if !output_path.exists() {
        info!("Creating directory: {}", output_path.display());
        create_dir(output_path)?;
        create_folders(output_path)
    } else if output_path.is_dir() {
        // clean all files in workspace
        delete_files_in_folder(output_path)?;
        for folder_config in FOLDERS {
            let folder = output_path.join(folder_config.name);
            if !folder.exists() {
                create_dir(folder)?;
            } else if folder_config.should_delete {
                delete_files_in_folder(&folder)?;
            }
        }
        Ok(())
    } else {
        Err(Error::new(
            ErrorKind::NotADirectory,
            format!(
                "Current workspace needs to be a directory, but was: {}",
                output_path.display()
            ),
        ))
    }
}

fn delete_files_in_folder(path: &Path) -> io::Result<()> {
    assert!(
        path.exists() && path.is_dir(),
        "Expected that output directory ({}) should already exist!",
        path.display()
    );
    info!("Deleting all files in folder: {}", path.display());
    for entry in read_dir(path)? {
        let path = entry?.path();
        if path.is_file() {
            remove_file(path)?;
        }
    }
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
