use std::{path::Path, str::Split};
use tokio::{
    fs,
    process::{Child, Command},
};

use crate::{LogEntry, RefNames};

/// pub functions from this file are only for benchmarking purposes
// TODO: nothing to benchmark anymore here

pub fn log_entry_from_split(split: &mut Split<&str>) -> LogEntry {
    LogEntry {
        // TODO: remove color reset workaround once https://github.com/uttarayan21/ansi-to-tui/issues/40 is fixed
        graph: String::from(split.next().unwrap()).replace("[m", "[0m"),
        hash: String::from(split.next().unwrap_or("")),
        subject: String::from(split.next().unwrap_or("")),
        author: String::from(split.next().unwrap_or("")),
        date: String::from(split.next().unwrap_or("")),
        refs: RefNames::from(split.next().unwrap_or("")),
        reached_by: String::from(split.next().unwrap_or("")),
    }
}

pub async fn get_log<'a>(
    repository: &Path,
    revision_range: &[String],
) -> Result<Child, std::io::Error> {
    let repository = fs::canonicalize(repository).await?;
    let child = Command::new("git")
        .kill_on_drop(true)
        .current_dir(repository)
        .args([
            "log",
            "--graph",
            "--color=always",
            "--oneline",
            "--decorate=full", // full decoration needed for refs/tags, refs/remotes etc.
        ])
        // %S for which command line ref reached that commit
        // %D refs
        .arg("--format=\x1f%H\x1f%s\x1f%aN\x1f%ar\x1f%D\x1f%S")
        .args(revision_range)
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    Ok(child)
}
