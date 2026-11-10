//! Pins refs/remotes/origin/main to the commit the affected checks compare against.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::io::Write;
use std::path::Path;

use dx_process::capture::{DEFAULT_TIMEOUT, git as git_command, run_captured};

pub struct Event {
    pub name: String,
    pub push_before: String,
    pub merge_group_base: String,
}

struct Git {
    code: i32,
    out: String,
    err: String,
}

fn git(args: &[&str]) -> Git {
    match run_captured(git_command(Path::new(".")).args(args), DEFAULT_TIMEOUT) {
        Ok(done) => Git {
            code: done.code.unwrap_or(-1),
            out: done.stdout_text().trim().to_string(),
            err: done.stderr_text(),
        },
        Err(error) => Git {
            code: -1,
            out: String::new(),
            err: error.to_string(),
        },
    }
}

fn git_checked(args: &[&str]) -> Result<Git, String> {
    let done = git(args);
    if done.code == 0 {
        Ok(done)
    } else {
        Err(format!("git {} failed: {}", args.join(" "), done.err))
    }
}

fn candidate(event: &Event, fallback: &str) -> Result<String, String> {
    match event.name.as_str() {
        "merge_group" => {
            if event.merge_group_base.is_empty() {
                Err("gate: merge_group event has no base_sha".to_string())
            } else {
                Ok(event.merge_group_base.clone())
            }
        }
        "push" => {
            let before = &event.push_before;
            let zeros = before.chars().all(|c| c == '0');
            let object = format!("{before}^{{commit}}");
            let known = !before.is_empty() && !zeros && git(&["cat-file", "-e", &object]).code == 0;
            Ok(if known {
                before.clone()
            } else {
                fallback.to_string()
            })
        }
        _ => Ok(fallback.to_string()),
    }
}

/// Pins the base ref and returns the base commit and the HEAD commit.
pub fn pin(event: &Event) -> Result<(String, String), String> {
    let head = git_checked(&["rev-parse", "HEAD"])?.out;
    let parent = git(&["rev-parse", "--verify", "-q", "HEAD^1"]);
    let fallback = if parent.code == 0 {
        parent.out
    } else {
        String::new()
    };
    let candidate = candidate(event, &fallback)?;
    if candidate.is_empty() {
        return Err(format!("gate: no base commit for event {:?}", event.name));
    }
    let merged = git(&["merge-base", &candidate, &head]);
    let base = if merged.code == 0 {
        merged.out
    } else {
        candidate
    };
    if base.is_empty() {
        return Err("gate: empty base".to_string());
    }
    if base == head {
        return Err(format!(
            "gate: base {base} equals HEAD, nothing would be compared"
        ));
    }
    git_checked(&["update-ref", "refs/remotes/origin/main", &base])?;
    if git(&["rev-parse", "--verify", "-q", "refs/remotes/origin/HEAD"]).code == 0 {
        git_checked(&["update-ref", "--no-deref", "-d", "refs/remotes/origin/HEAD"])?;
    }
    Ok((base, head))
}

/// Appends GATE_BASE to the GitHub env file.
pub fn export(path: &str, base: &str) -> Result<(), String> {
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .map_err(|error| format!("cannot open {path}: {error}"))?;
    writeln!(file, "GATE_BASE={base}").map_err(|error| format!("cannot write {path}: {error}"))
}
