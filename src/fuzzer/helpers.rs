use std::{fmt::Display, fs::remove_file, io, path::PathBuf, time::Duration};

use log::error;
use serde::{Deserialize, Serialize};

/// const to stretch strings to a specific length
pub const OFFSET: usize = 50;

/// Wrap a printable Item in a block of equal-signs and a header
pub fn print_block<T: Display>(heading: &str, item: T) {
    let amount_signs = 40;
    println!();
    println!("{:=^amount_signs$}", heading);
    println!("{item}");
    println!("{}", "=".repeat(amount_signs));
    println!()
}

/// Returns the parts of a duration in the format (hours, minutes, seconds)
///
/// Example
/// ```rust
/// # use fuzzer_pseudo_bool::fuzzer::helpers::get_hms_parts;
/// # use std::time::Duration;
/// let dur = Duration::from_secs(9_812);
/// let (h, m, s) = get_hms_parts(&dur);
/// assert_eq!(2, h);
/// assert_eq!(43, m);
/// assert_eq!(32, s);
/// ````
pub fn get_hms_parts(duration: &Duration) -> (u64, u64, u64) {
    let secs = duration.as_secs();
    let hours = secs / 3600;
    let remainder = secs % 3600;
    let mins = remainder / 60;
    let secs = remainder % 60;
    (hours, mins, secs)
}

/// Returns the current time in Year-Month-Day-Hour-Minute-Second format
pub fn current_time_as_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

/// Compute the running average given a current average, a sample, and the size of already measured
/// samples
///
/// Example:
/// ```rust
///  # use fuzzer_pseudo_bool::fuzzer::helpers::compute_running_average;
///  let current_average = (2 + 3) as f32 / 2.0;
///  let total_runs = 2;
///  let sample = 5;
///  let new_average = compute_running_average(current_average, sample, total_runs);
///  assert_eq!(new_average, (2 + 3 + 5) as f32 / 3.0);
/// ````
/// This is expected to return the same like just computing `(2+3+5) / 3`
pub fn compute_running_average(current_average: f32, sample: u32, total_runs: u32) -> f32 {
    (total_runs as f32 * current_average + sample as f32) / (total_runs + 1) as f32
}

/// Encapsulates logic of computing average such that there is no need for manually counting the
/// total runs until point x.
///
/// When updating the average with a new value the struct automatically updates the sample_count as
/// well
#[derive(Default, Serialize, Deserialize)]
pub struct RunningAverage {
    average: f32,
    sample_count: u32,
}

impl RunningAverage {
    pub fn new(average: f32, sample_count: u32) -> Self {
        RunningAverage {
            average,
            sample_count,
        }
    }
    pub fn update(&mut self, sample: u32) {
        self.average = (self.sample_count as f32 * self.average + sample as f32)
            / (self.sample_count + 1) as f32;
        self.sample_count += 1;
    }

    pub fn get_average(&self) -> f32 {
        self.average
    }
}

#[derive(Default, Serialize)]
pub struct RunningAverageDuration {
    average: Duration,
    sample_count: u32,
}

impl RunningAverageDuration {
    pub fn update(&mut self, sample: &Duration) {
        self.average = (self.sample_count * self.average + *sample) / (self.sample_count + 1);
        self.sample_count += 1;
    }

    pub fn get_average(&self) -> Duration {
        self.average
    }
}

pub struct MoveFile {
    from: PathBuf,
    to: PathBuf,
}

impl MoveFile {
    pub fn new(from: PathBuf, to: PathBuf) -> Self {
        Self { from, to }
    }
}

pub fn remove_files(files: &[MoveFile]) -> io::Result<()> {
    for file in files {
        remove_file(&file.from)?;
    }
    Ok(())
}

/// Tries to move all files
///
/// If some operation failes, the error is simply logged.
/// All the remaining files are still tried to be moved as well
pub fn move_files(files: &[MoveFile]) -> io::Result<()> {
    for file in files {
        if let Err(e) = std::fs::rename(&file.from, &file.to) {
            error!("{e}");
        };
    }
    Ok(())
}

/// Helper function to check two floats on equality
///
/// ```rust
/// # use fuzzer_pseudo_bool::fuzzer::helpers::float_equals;
/// assert!(float_equals(3.333332, 3.333335, 0.0001))
/// ````
pub fn f32_equals(a: f32, b: f32, epsilon: f32) -> bool {
    assert!(epsilon >= 0.0, "Epsilon needs to be a value >= 0");
    (a - b).abs() <= epsilon
}

/// Helper function to check two floats on equality
///
/// ```rust
/// # use fuzzer_pseudo_bool::fuzzer::helpers::float_equals;
/// assert!(float_equals(3.333332, 3.333335, 0.0001))
/// ````
pub fn f64_equals(a: f64, b: f64, epsilon: f64) -> bool {
    assert!(epsilon >= 0.0, "Epsilon needs to be a value >= 0");
    (a - b).abs() <= epsilon
}

/// Convert a duration into milliseconds
pub fn ms(duration: &Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duration_to_hours() {
        let dur = Duration::from_mins(255);
        let (h, m, s) = get_hms_parts(&dur);
        assert_eq!(4, h);
        assert_eq!(15, m);
        assert_eq!(0, s);
    }

    #[test]
    fn test_equals_float() {
        assert!(f32_equals(3.333332, 3.333335, 0.0001))
    }

    #[test]
    fn test_running_average() {
        let current_average = (2 + 3) as f32 / 2.0;
        let sample = 5;
        let total_runs = 2;
        let new_average = compute_running_average(current_average, sample, total_runs);
        let expected_average = (2 + 3 + sample) as f32 / 3.0;
        assert!(
            f32_equals(expected_average, new_average, 0.00001),
            "Expected: {}\nActual: {}",
            expected_average,
            new_average
        );

        let mut ra = RunningAverage::default();
        assert_eq!(0, ra.sample_count);
        assert_eq!(0.0, ra.average);
        ra.update(2);
        ra.update(3);
        ra.update(5);
        assert!(
            f32_equals(expected_average, ra.get_average(), 0.00001),
            "Expected: {}\nActual: {}",
            expected_average,
            ra.get_average()
        );
    }

    #[test]
    fn test_running_average_duration() {
        let sample = 5;
        let expected_average = (2 + 3 + sample) as f32 / 3.0;

        let mut ra = RunningAverageDuration::default();
        assert_eq!(0, ra.sample_count);
        assert_eq!(Duration::ZERO, ra.average);
        ra.update(&Duration::from_secs(2));
        ra.update(&Duration::from_secs(3));
        ra.update(&Duration::from_secs(5));
        assert!(
            f32_equals(expected_average, ra.average.as_secs_f32(), 0.00001),
            "Expected: {}\nActual: {}",
            expected_average,
            ra.get_average().as_secs_f32()
        );
    }

    #[test]
    fn test_ms() {
        let dur = Duration::from_millis(33424);
        let dur_ms = ms(&dur);
        assert!(f64_equals(33424.0, dur_ms, 0.00001))
    }
}
