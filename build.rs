use std::{error::Error, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .map(|o| o.stdout)?;
    let git_hash = String::from_utf8(output)?;
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    Ok(())
}
