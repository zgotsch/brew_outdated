#[macro_use]
extern crate lazy_static;

use futures::join;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::stream::StreamExt;
use regex::Regex;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::Path;
use std::rc::Rc;
use yansi::Paint;

mod history;
mod homebrew;

lazy_static! {
    static ref EXTRACT_CMD_RE: Regex = Regex::new(r#"^\s*(\S+)"#).unwrap();
}

fn extract_cmd(line: &str) -> &str {
    let first_token: &str = EXTRACT_CMD_RE
        .captures(line)
        .unwrap()
        .get(1)
        .unwrap()
        .as_str();
    return Path::new(first_token)
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();
}

#[tokio::main]
async fn main() -> Result<(), String> {
    // get commands run recently
    // run brew outdated
    // get things that are in both and print them nicely

    // a guess for a reasonable upper bound of executables per brew package
    const EXECUTABLES_PER_PACKAGE: usize = 10;
    let brew_fut = async {
        let outdated: Vec<homebrew::BrewOutdatedEntry> = homebrew::outdated().await.unwrap();
        let outdated_executable_to_package: HashMap<OsString, Rc<homebrew::BrewOutdatedEntry>> =
            HashMap::with_capacity(outdated.len() * EXECUTABLES_PER_PACKAGE);
        let outdated_executables: HashSet<OsString> =
            HashSet::with_capacity(outdated.len() * EXECUTABLES_PER_PACKAGE);

        let executables_futures = FuturesUnordered::new();
        for outdated_entry in outdated.into_iter() {
            executables_futures.push(async {
                let executables = homebrew::executables(
                    &outdated_entry.package_name,
                    &outdated_entry.installed_versions.last().unwrap(),
                )
                .await;
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

        // executables_futures.for_each_concurrent(None, || async {});
    };

    let used_executables = async {
        history::recent_history()
            .await
            .unwrap()
            .iter()
            .map(|line| extract_cmd(line).into())
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
                Paint::red(&entry.installed_versions.last().unwrap()).bold(),
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

    Ok(())
}
