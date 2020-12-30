#[macro_use]
extern crate lazy_static;

use fork::{daemon, Fork};
use futures::join;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::io;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{exit, Command, Output};
use std::rc::Rc;
use std::time::SystemTime;
use yansi::Paint;

mod history;
mod homebrew;

lazy_static! {
    static ref EXTRACT_CMD_RE: Regex = Regex::new(r#"^\s*(\S+)"#).unwrap();
    static ref RS_ERROR_DIR: PathBuf = PathBuf::from("/tmp/rs_outdated");
    static ref BREW_UPDATE_ERROR_FILE_RE: Regex = Regex::new(r#"^brew_output_\d+$"#).unwrap();
}

fn extract_cmd(line: &str) -> Option<&str> {
    let first_token: &str = EXTRACT_CMD_RE.captures(line)?.get(1)?.as_str();
    Path::new(first_token).file_name()?.to_str()
}

fn make_error_filename() -> PathBuf {
    let unix_time = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap();
    let mut path = RS_ERROR_DIR.clone();
    path.push(format!("brew_output_{}", unix_time.as_secs()));
    path
}

fn error_file_paths() -> io::Result<impl Iterator<Item = io::Result<PathBuf>>> {
    Ok(std::fs::read_dir(&*RS_ERROR_DIR)?.filter_map(|r| match r {
        Ok(dir_entry) => {
            if BREW_UPDATE_ERROR_FILE_RE.is_match(
                &dir_entry
                    .path()
                    .file_name()
                    .unwrap_or(std::ffi::OsStr::new(""))
                    .to_str()
                    .unwrap(),
            ) {
                Some(Ok(dir_entry.path()))
            } else {
                None
            }
        }
        Err(e) => Some(Err(e)),
    }))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // If we find an error file, it means we didn't succeed at running brew update last time we ran
    // print a message for the user
    match error_file_paths() {
        Ok(error_file_paths) => {
            if let Some(latest_error_path) = error_file_paths.map(|p| p.unwrap()).max() {
                println!("{} `{}` did not run successfully last time. Please see {} for the error output.\n\
                          This message will stop appearing when the deferred `{}` runs successfully.\n",
                          Paint::red("Warning:").bold(),
                          Paint::new("brew update").bold(),
                          Paint::new(latest_error_path.display()).bold(),
                          Paint::new("brew update").bold(),
                        );
            }
        }
        _ => (),
    }

    // get commands run recently
    // run brew outdated
    // get things that are in both and print them nicely

    // a guess for a reasonable upper bound of executables per brew package
    const EXECUTABLES_PER_PACKAGE: usize = 10;
    let brew_fut = async {
        let outdated: homebrew::BrewOutdatedOutput = homebrew::outdated().await.unwrap();
        let outdated_executable_to_package: HashMap<
            OsString,
            Rc<homebrew::BrewOutdatedFormulaEntry>,
        > = HashMap::with_capacity(outdated.formulae.len() * EXECUTABLES_PER_PACKAGE);
        let outdated_executables: HashSet<OsString> =
            HashSet::with_capacity(outdated.formulae.len() * EXECUTABLES_PER_PACKAGE);

        let executables_futures = FuturesUnordered::new();
        for outdated_entry in outdated.formulae.into_iter() {
            executables_futures.push(async {
                let executables = homebrew::executables(
                    &outdated_entry.package_name,
                    &outdated_entry.latest_installed_version(),
                )
                .await
                .unwrap_or_else(|_| {
                    eprintln!(
                        r#"Couldn't get executables for package "{}""#,
                        &outdated_entry.package_name
                    );
                    Vec::new()
                });
                (outdated_entry, executables)
            });
        }

        executables_futures
            .fold(
                (outdated_executable_to_package, outdated_executables),
                |(mut outdated_executable_to_package, mut outdated_executables),
                 (entry, executables)| async move {
                    let entry = Rc::new(entry);
                    for executable in executables {
                        outdated_executables.insert(executable.clone());
                        outdated_executable_to_package.insert(executable, entry.clone());
                    }
                    (outdated_executable_to_package, outdated_executables)
                },
            )
            .await
    };

    let used_executables = async {
        history::recent_history()
            .await
            .expect("Couldn't get shell history")
            .iter()
            .filter_map(|line| extract_cmd(line).map(|s| s.into()))
            .collect::<HashSet<std::ffi::OsString>>()
    };

    let ((outdated_executable_to_package, outdated_executables), used_executables) =
        join!(brew_fut, used_executables);

    let mut should_update_executables = outdated_executables
        .intersection(&used_executables)
        .peekable();

    if should_update_executables.peek().is_some() {
        let mut should_update_packages = Vec::with_capacity(outdated_executables.len());

        println!("You have recently used out-of-date executables which are managed by `brew`.");
        println!("Consider updating the following:");
        for executable in should_update_executables {
            let entry = outdated_executable_to_package.get(executable).unwrap();
            println!(
                "\t{} (installed: {}, available: {})",
                Paint::new(executable.to_string_lossy()).bold(),
                Paint::red(&entry.latest_installed_version()).bold(),
                Paint::green(&entry.current_version).bold()
            );
            should_update_packages.push(entry.package_name.as_str());
        }
        let brew_cmd = vec![vec!["brew", "upgrade"], should_update_packages]
            .concat()
            .join(" ");
        println!(
            "To upgrade all of these in one command, run `{}`",
            Paint::new(brew_cmd).bold()
        );
    }

    if let Ok(Fork::Child) = daemon(false, false) {
        let error_to_write = match Command::new("brew").arg("update").output() {
            Ok(Output { status, stderr, .. }) => {
                if status.success() {
                    None
                } else {
                    Some(String::from_utf8_lossy(&stderr).to_string())
                }
            }
            Err(e) => Some(e.to_string()),
        };
        // If this fails, not much we can do...
        std::fs::create_dir(&*RS_ERROR_DIR)
            .or_else(|e| match e.kind() {
                std::io::ErrorKind::AlreadyExists => Ok(()),
                _ => Err(e),
            })
            .unwrap();

        if let Some(error_string) = error_to_write {
            let mut file = std::fs::File::create(make_error_filename()).unwrap();
            file.write_all(error_string.as_bytes()).unwrap();
        } else {
            // if successful, rename error files
            for filepath in error_file_paths().unwrap() {
                if let Ok(filepath) = filepath {
                    let _ = std::fs::rename(&filepath, format!("{}_resolved", filepath.display()));
                }
            }
        }

        exit(0);
    }

    Ok(())
}
