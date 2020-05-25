use derive_more::{Display, Error};
use regex::Regex;
use serde::Deserialize;
use std::ffi::OsString;
use std::path::PathBuf;
use std::str;
use tokio::process::Command;
use tokio::stream::StreamExt;

lazy_static! {
    static ref BREW_PREFIX: String = std::process::Command::new("brew")
        .arg("--prefix")
        .output()
        .map_err(|e| e.to_string())
        .and_then(|output| {
            str::from_utf8(&output.stdout)
                .map_err(|e| e.to_string())
                .map(|s| s.trim().to_owned())
        })
        .expect("Couldn't get brew prefix");
    static ref BREW_PREFIX_RE: Regex =
        Regex::new(&format!(r#"{}/bin/(\S+)"#, &*BREW_PREFIX)).unwrap();
}

#[derive(Deserialize, Debug)]
pub struct BrewOutdatedEntry {
    #[serde(rename = "name")]
    pub package_name: String,
    pub installed_versions: Vec<String>,
    pub current_version: String,
    pub pinned: bool,
}

#[derive(Debug, Display, Error)]
pub enum OutdatedError {
    UtfParseError(std::str::Utf8Error),
    BrewJsonParseError(serde_json::Error),
}
pub async fn outdated() -> Result<Vec<BrewOutdatedEntry>, OutdatedError> {
    let output = Command::new("brew")
        .arg("outdated")
        .arg("--json")
        .output()
        .await
        .expect("Failed to run `brew outdated`");
    let brew_outdated_json =
        str::from_utf8(&output.stdout).map_err(OutdatedError::UtfParseError)?;
    let brew_entries: Vec<BrewOutdatedEntry> =
        serde_json::from_str(brew_outdated_json).map_err(OutdatedError::BrewJsonParseError)?;
    return Ok(brew_entries);
}

pub async fn executables(package_name: &str, installed_version: &str) -> Vec<OsString> {
    let bin_path: PathBuf = [
        &*BREW_PREFIX,
        "Cellar",
        package_name,
        installed_version,
        "bin",
    ]
    .iter()
    .collect();
    tokio::fs::read_dir(&bin_path)
        .await
        .unwrap()
        .map(|r| r.unwrap().file_name())
        .collect::<Vec<OsString>>()
        .await
}
