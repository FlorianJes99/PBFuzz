use std::borrow::Cow;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::{
    fs::{self, OpenOptions},
    io::{self},
    path::{Path, PathBuf},
};

use log::error;

use crate::orchestrator::issue_report::issue_record::IssueRecord;

/// Can be used to provide additional data, which should be flushed to a file in the case directory.
///
/// filename: the name of the file, should include also the concrete textfile type (Example: test.txt)
/// contents: raw bytes we want to flush, wrapped by a Clone-on-Write smart pointer
pub struct Artifact<'a> {
    /// filename: the name of the file, should include also the concrete textfile type (Example: test.txt)
    pub file_name: String,
    /// contents: raw bytes we want to flush, wrapped by a Clone-on-Write smart pointer
    pub contents: Cow<'a, [u8]>,
}

/// Trait for handling file creation for an issue report.
pub trait IssueReport {
    /// Function to write a report into the provided writer.
    ///
    /// Implementations should provide, what data to write into the report.
    /// writer: The content is written into it.
    fn create_report<W: Write>(&self, writer: &mut W) -> io::Result<()>;

    // Provide additional artifacts in the form of files, which also should be written into an issue report.
    fn artifacts(&self) -> Vec<Artifact<'_>>;
}

pub struct CaseWriter {
    cases_folder: PathBuf,
    index_file: PathBuf,
}

impl CaseWriter {
    pub fn new(base_folder: &Path, index_file: PathBuf) -> Self {
        let cases_folder = base_folder.join("cases/");
        assert!(std::fs::exists(&cases_folder).expect("Check if file exists should work!"));
        Self {
            cases_folder,
            index_file,
        }
    }

    pub fn process(
        &self,
        issue_record: &IssueRecord,
        report_writer: &impl IssueReport,
        files_to_move: &[&Path],
    ) -> io::Result<()> {
        let case_folder = self
            .cases_folder
            .join(format!("case_{}", issue_record.iteration));
        std::fs::create_dir(&case_folder)?;
        if let Err(e) = self.move_files(&case_folder, files_to_move) {
            error!(
                "Error while moving relevant files into case {}: {e}",
                issue_record.iteration
            )
        }
        let meta_json = &case_folder.join("meta.json");
        if let Err(e) = self.write_case(meta_json, issue_record) {
            error!(
                "Error while writing the json record of the case {}: {e}",
                issue_record.iteration
            )
        }
        let report_file = &case_folder.join("report.md");
        if let Err(e) = self.write_report(report_file, report_writer) {
            error!(
                "Error while writing the report for a case {}: {e}",
                issue_record.iteration
            )
        }

        for artifact in &report_writer.artifacts() {
            if let Err(e) = self.write_artifact(&case_folder, artifact) {
                error!("{e}");
            }
        }
        self.append_case(issue_record)
    }

    fn write_case(&self, meta_json: &Path, issue_record: &IssueRecord) -> io::Result<()> {
        fs::write(meta_json, serde_json::to_string_pretty(&issue_record)?)
    }

    fn write_report(&self, report_file: &Path, report_writer: &impl IssueReport) -> io::Result<()> {
        let mut w = File::create(report_file).map(BufWriter::new)?;
        report_writer.create_report(&mut w)
    }

    fn write_artifact(&self, case_folder: &Path, artifact: &Artifact<'_>) -> io::Result<()> {
        let file_name = Path::new(&artifact.file_name).file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid name specified for report artifact",
            )
        })?;
        fs::write(case_folder.join(file_name), artifact.contents.as_ref())
    }

    fn move_files(&self, case_folder: &Path, files_to_move: &[&Path]) -> io::Result<()> {
        for f in files_to_move {
            if f.exists()
                && let Some(name) = f.file_name()
            {
                let new_file = case_folder.join(name);
                std::fs::rename(f, new_file)?
            }
        }
        Ok(())
    }

    fn append_case(&self, issue_record: &IssueRecord) -> io::Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.index_file)?;
        writeln!(f, "{}", serde_json::to_string(issue_record)?)
    }
}
