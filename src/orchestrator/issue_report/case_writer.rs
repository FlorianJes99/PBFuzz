use std::fs::File;
use std::io::{BufWriter, Write};
use std::{
    fs::{self, OpenOptions},
    io::{self},
    path::{Path, PathBuf},
};

use crate::orchestrator::issue_report::issue_record::IssueRecord;

pub trait IssueReport {
    fn create_report<W: Write>(&self, writer: &mut W) -> io::Result<()>;
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
        self.move_files(&case_folder, files_to_move)?;
        let meta_json = &case_folder.join("meta.json");
        self.write_case(meta_json, issue_record)?;
        let report_file = &case_folder.join("report.md");
        self.write_report(report_file, report_writer)?;
        self.append_case(issue_record)
    }

    fn write_case(&self, meta_json: &Path, issue_record: &IssueRecord) -> io::Result<()> {
        fs::write(meta_json, serde_json::to_string_pretty(&issue_record)?)
    }

    fn write_report(&self, report_file: &Path, report_writer: &impl IssueReport) -> io::Result<()> {
        let mut w = File::create(report_file).map(BufWriter::new)?;
        report_writer.create_report(&mut w)
    }

    fn move_files(&self, case_folder: &Path, files_to_move: &[&Path]) -> io::Result<()> {
        for f in files_to_move {
            if let Some(name) = f.file_name() {
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
