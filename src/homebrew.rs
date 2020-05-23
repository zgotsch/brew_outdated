use derive_more::{Display, Error};
use regex::Regex;
use serde::Deserialize;
use std::str;
use tokio::process::Command;

lazy_static! {
    static ref BREW_PREFIX_RE: Regex = {
        let brew_prefix = std::process::Command::new("brew")
            .arg("--prefix")
            .output()
            .map_err(|e| e.to_string())
            .map(|output| {
                str::from_utf8(&output.stdout)
                    .map(|s| s.trim().to_owned())
                    .map_err(|e| e.to_string())
                    .unwrap()
            });
        Regex::new(&format!(r#"{}/bin/(\S+)"#, brew_prefix.unwrap())).unwrap()
    };
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

pub async fn executables(package_name: &str) -> Vec<String> {
    let brew_unlink_output = Command::new("brew")
        .arg("unlink")
        .arg("--dry-run")
        .arg(package_name)
        .output();

    let out: Vec<String> = BREW_PREFIX_RE
        .captures_iter(str::from_utf8(&brew_unlink_output.await.unwrap().stdout).unwrap())
        .map(|cap| cap[1].to_owned())
        .collect();

    return out;
}
