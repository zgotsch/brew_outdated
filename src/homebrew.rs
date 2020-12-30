use derive_more::{Display, Error};
use regex::Regex;
use serde::Deserialize;
use std::io;
use std::path::PathBuf;
use std::str;
use std::{ffi::OsString, io::ErrorKind};
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
pub struct BrewOutdatedFormulaEntry {
    #[serde(rename = "name")]
    pub package_name: String,
    pub installed_versions: Vec<String>,
    pub current_version: String,
    pub pinned: bool,
}

impl BrewOutdatedFormulaEntry {
    pub fn latest_installed_version(&self) -> &String {
        return &self.installed_versions.last().expect(
            "Tried to get the latest installed version of a package with no installed versions.",
        );
    }
}

#[derive(Deserialize, Debug)]
pub struct BrewOutdatedOutput {
    pub formulae: Vec<BrewOutdatedFormulaEntry>,
}

#[derive(Debug, Display, Error)]
pub enum OutdatedError {
    UtfParseError(std::str::Utf8Error),
    BrewJsonParseError(serde_json::Error),
}
pub async fn outdated() -> Result<BrewOutdatedOutput, OutdatedError> {
    let output = Command::new("brew")
        .arg("outdated")
        .arg("--json=v2")
        .output()
        .await
        .expect("Failed to run `brew outdated`");
    let brew_outdated_json =
        str::from_utf8(&output.stdout).map_err(OutdatedError::UtfParseError)?;
    let brew_entries: BrewOutdatedOutput =
        serde_json::from_str(brew_outdated_json).map_err(OutdatedError::BrewJsonParseError)?;
    return Ok(brew_entries);
}

pub async fn executables(package_name: &str, installed_version: &str) -> io::Result<Vec<OsString>> {
    let package_path: PathBuf = [&*BREW_PREFIX, "Cellar", package_name, installed_version]
        .iter()
        .collect();
    let _ = tokio::fs::metadata(&package_path).await?;

    let bin_path = {
        let mut p = package_path.clone();
        p.push("bin");
        p
    };

    match tokio::fs::read_dir(&bin_path).await {
        Ok(read_dir) => {
            return Ok(read_dir
                .map(|r| r.unwrap().file_name())
                .collect::<Vec<OsString>>()
                .await);
        }
        Err(err) => {
            if err.kind() == ErrorKind::NotFound {
                return Ok(vec![]);
            } else {
                return Err(err);
            }
        }
    }
}
