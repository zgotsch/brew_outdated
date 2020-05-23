use futures::future::FutureExt;
use futures::join;
use regex::Regex;
use serde::Deserialize;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::str;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;

type HistoryLine = String;

pub fn bash_history_file_location() -> Option<PathBuf> {
    if let Ok(path) = env::var("HISTFILE") {
        return Some(PathBuf::from(path));
    }
    if let Ok(home_dir) = env::var("HOME") {
        return Some([&home_dir, ".bash_history"].iter().collect());
    }
    return None;
}

async fn read_simple_history(path: &Path) -> Option<Vec<HistoryLine>> {
    let bytes: Vec<u8> = fs::read(path).await.ok()?;
    let file_string = str::from_utf8(&bytes).ok()?;
    return Some(file_string.lines().map(|l| l.to_owned()).collect());
}

async fn bash_history() -> Option<Vec<HistoryLine>> {
    read_simple_history(&bash_history_file_location()?).await
}
async fn zsh_history() -> Option<Vec<HistoryLine>> {
    if let Ok(path) = env::var("HISTFILE") {
        return read_simple_history(Path::new(&path)).await;
    }
    if let Ok(home_dir) = env::var("HOME") {
        const ZSH_LOCATIONS: [&str; 3] = [".histfile", ".zhistory", ".zsh_history"];
        for &location in &ZSH_LOCATIONS {
            let file_path: PathBuf = [&home_dir, location].iter().collect();
            if let Some(history) = read_simple_history(&file_path).await {
                return Some(history);
            }
        }
    }
    return None;
}
async fn nushell_history() -> Option<Vec<HistoryLine>> {
    let home_dir = env::var("HOME").ok()?;
    let nushell_history_path: PathBuf = [&home_dir, "Library/Application Support/nu/history.txt"]
        .iter()
        .collect();
    read_simple_history(&nushell_history_path).await
}

#[derive(Deserialize, Debug)]
pub struct FishLine {
    #[serde(rename = "when")]
    time: u64,
    cmd: HistoryLine,
}
pub async fn fish_history() -> Option<Vec<FishLine>> {
    let dir_path = env::var("XDG_DATA_HOME")
        .or_else(|_| -> Result<String, std::env::VarError> {
            let home_dir = env::var("HOME")?;
            return Ok([&home_dir, ".local/share"]
                .iter()
                .collect::<PathBuf>()
                .to_string_lossy()
                .to_string());
        })
        .ok()?;

    let fish_history_path: PathBuf = [&dir_path, "fish/fish_history"].iter().collect();

    let bytes: Vec<u8> = fs::read(fish_history_path).await.ok()?;
    let file_contents = str::from_utf8(&bytes).ok()?;

    let mut fish_lines: Vec<FishLine> = Vec::new();
    let mut current_cmd = None;
    let mut current_time = None;
    let cmd_re = Regex::new("^- cmd: (.*)$").unwrap();
    let time_re = Regex::new(r#"^\s*when: (\d+)$"#).unwrap();
    for line in file_contents.lines() {
        if line.starts_with("-") {
            if let (Some(cmd), Some(time)) = (current_cmd.take(), current_time.take()) {
                fish_lines.push(FishLine { cmd, time });
                current_cmd = None;
                current_time = None;
            }
        }
        if let Some(caps) = cmd_re.captures(line) {
            current_cmd = Some(caps[1].to_owned());
        }
        if let Some(caps) = time_re.captures(line) {
            current_time = Some(caps[1].parse::<u64>().unwrap());
        }
    }

    return Some(fish_lines);
}

pub async fn recent_history() -> Option<Vec<HistoryLine>> {
    // Recent for histories which do not support time is the last 1000 lines
    let bash = bash_history().map(|v| {
        v.map(|lines: Vec<HistoryLine>| {
            lines
                .into_iter()
                .rev()
                .take(1000)
                .collect::<Vec<HistoryLine>>()
        })
    });
    let zsh = zsh_history().map(|v| {
        v.map(|lines: Vec<HistoryLine>| {
            lines
                .into_iter()
                .rev()
                .take(1000)
                .collect::<Vec<HistoryLine>>()
        })
    });
    let nushell = nushell_history().map(|v| {
        v.map(|lines: Vec<HistoryLine>| {
            lines
                .into_iter()
                .rev()
                .take(1000)
                .collect::<Vec<HistoryLine>>()
        })
    });

    // Recent for histories which do support time is the last
    let current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let cutoff_time = current_time - Duration::from_secs(60 * 60 * 24 * 14); // 2 weeks
    let cutoff_time_u64 = cutoff_time.as_secs() as u64;
    let fish = fish_history().map(|outcome: Option<Vec<FishLine>>| {
        outcome.map(|lines| {
            lines
                .into_iter()
                .filter_map(|fish| match fish.time < cutoff_time_u64 {
                    true => None,
                    _ => Some(fish.cmd),
                })
                .collect::<Vec<HistoryLine>>()
        })
    });

    let (bash, zsh, nushell, fish) = join!(bash, zsh, nushell, fish);
    return Some(
        vec![bash, zsh, nushell, fish]
            .into_iter()
            .flat_map(|x: Option<Vec<String>>| match x {
                Some(v) => v,
                None => Vec::new(),
            })
            .collect(),
    );
}
