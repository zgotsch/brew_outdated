#![feature(type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]

#[macro_use]
extern crate lazy_static;

use futures::future::join_all;
use futures::join;
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;

mod history;
mod homebrew;

lazy_static! {
    static ref EXTRACT_RE: Regex = Regex::new(r#"^\s*(\S+)"#).unwrap();
}

fn extract_cmd(line: &str) -> &str {
    let first_token: &str = EXTRACT_RE.captures(line).unwrap().get(1).unwrap().as_str();
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

    let outdated_executables = async {
        let outdated = homebrew::outdated().await.unwrap();
        let executables = join_all(
            outdated
                .iter()
                .map(|p| homebrew::executables(&p.package_name)),
        )
        .await
        .into_iter()
        .flatten()
        .collect::<HashSet<String>>();
        return executables;
    };

    let used_executables = async {
        let history = history::recent_history().await.unwrap();
        history
            .iter()
            .map(|line| extract_cmd(line).to_owned())
            .collect::<HashSet<String>>()
    };

    let (outdated_executables, used_executables) = join!(outdated_executables, used_executables);

    println!(
        "{:#?}",
        outdated_executables.intersection(&used_executables)
    );

    Ok(())
}
