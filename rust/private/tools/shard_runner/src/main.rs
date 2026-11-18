//! Runs one Bazel test shard of a libtest binary.
//!
//! The binary comes from the `DX_TEST_BINARY` runfiles key. Its tests are listed with
//! `--list --format terse`; the shard keeps the tests whose index `i` satisfies
//! `i % TEST_TOTAL_SHARDS == TEST_SHARD_INDEX` and runs them with `--exact`.
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

use std::env;
use std::ffi::OsString;
use std::fs::File;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

/// Bytes of test names passed to one child process, below the Windows command line limit.
const BATCH_BYTES: usize = 24 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(message) => {
            eprintln!("dx shard runner: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, String> {
    let binary = test_binary()?;
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let Some(shard) = Shard::from_env()? else {
        return status(Command::new(&binary).args(&args));
    };
    if let Some(status_file) = env::var_os("TEST_SHARD_STATUS_FILE") {
        File::create(&status_file)
            .map_err(|e| format!("cannot touch TEST_SHARD_STATUS_FILE: {e}"))?;
    }
    let listing = Command::new(&binary)
        .args(&args)
        .args(["--list", "--format", "terse"])
        .output()
        .map_err(|e| format!("cannot list {}: {e}", binary.display()))?;
    if !listing.status.success() {
        return Err(format!(
            "{} --list failed with {}:\n{}",
            binary.display(),
            listing.status,
            String::from_utf8_lossy(&listing.stderr)
        ));
    }
    let listing = String::from_utf8_lossy(&listing.stdout);
    let names = shard.select(&test_names(&listing));
    let mut code = ExitCode::SUCCESS;
    for batch in batches(&names, BATCH_BYTES) {
        let batch_code = status(Command::new(&binary).args(&args).arg("--exact").args(batch))?;
        if batch_code != ExitCode::SUCCESS {
            code = batch_code;
        }
    }
    Ok(code)
}

fn test_binary() -> Result<PathBuf, String> {
    let key = env::var("DX_TEST_BINARY").map_err(|_| "DX_TEST_BINARY is not set".to_owned())?;
    let runfiles =
        runfiles::Runfiles::create().map_err(|e| format!("cannot open runfiles: {e}"))?;
    runfiles
        .rlocation(&key)
        .filter(|path| path.exists())
        .ok_or_else(|| format!("{key} is not in the runfiles"))
}

fn status(command: &mut Command) -> Result<ExitCode, String> {
    let status = command
        .status()
        .map_err(|e| format!("cannot run {:?}: {e}", command.get_program()))?;
    Ok(match status.code() {
        Some(0) => ExitCode::SUCCESS,
        Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
        None => ExitCode::FAILURE,
    })
}

#[derive(Debug, PartialEq)]
struct Shard {
    index: usize,
    total: usize,
}

impl Shard {
    fn from_env() -> Result<Option<Self>, String> {
        Self::parse(
            env::var("TEST_SHARD_INDEX").ok().as_deref(),
            env::var("TEST_TOTAL_SHARDS").ok().as_deref(),
        )
    }

    fn parse(index: Option<&str>, total: Option<&str>) -> Result<Option<Self>, String> {
        let (Some(index), Some(total)) = (index, total) else {
            return Ok(None);
        };
        let index: usize = index
            .parse()
            .map_err(|_| format!("TEST_SHARD_INDEX={index} is not a number"))?;
        let total: usize = total
            .parse()
            .map_err(|_| format!("TEST_TOTAL_SHARDS={total} is not a number"))?;
        if index >= total {
            return Err(format!(
                "TEST_SHARD_INDEX={index} is not below TEST_TOTAL_SHARDS={total}"
            ));
        }
        Ok(Some(Self { index, total }))
    }

    fn select<'a>(&self, names: &[&'a str]) -> Vec<&'a str> {
        names
            .iter()
            .enumerate()
            .filter(|(i, _)| i % self.total == self.index)
            .map(|(_, name)| *name)
            .collect()
    }
}

/// Test names from libtest's terse listing, in listing order.
fn test_names(listing: &str) -> Vec<&str> {
    listing
        .lines()
        .filter_map(|line| {
            line.strip_suffix(": test")
                .or_else(|| line.strip_suffix(": benchmark"))
        })
        .collect()
}

/// Splits names into runs whose summed length stays within limit; a longer name gets its own run.
fn batches<'a>(names: &[&'a str], limit: usize) -> Vec<Vec<&'a str>> {
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut size = 0;
    for name in names {
        let cost = name.len() + 1;
        match out.last_mut() {
            Some(batch) if size + cost <= limit => batch.push(name),
            _ => {
                out.push(vec![name]);
                size = 0;
            }
        }
        size += cost;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shard_env() {
        assert_eq!(Shard::parse(None, None), Ok(None));
        assert_eq!(Shard::parse(Some("1"), None), Ok(None));
        assert_eq!(
            Shard::parse(Some("1"), Some("3")),
            Ok(Some(Shard { index: 1, total: 3 }))
        );
        assert!(Shard::parse(Some("3"), Some("3")).is_err());
        assert!(Shard::parse(Some("x"), Some("3")).is_err());
    }

    #[test]
    fn selects_round_robin() {
        let names = ["a", "b", "c", "d", "e"];
        assert_eq!(Shard { index: 0, total: 2 }.select(&names), ["a", "c", "e"]);
        assert_eq!(Shard { index: 1, total: 2 }.select(&names), ["b", "d"]);
        assert!(Shard { index: 4, total: 6 }.select(&names[..2]).is_empty());
    }

    #[test]
    fn reads_terse_listing() {
        let listing = "a::one: test\nb::two: benchmark\nnot a test line\nc: test\n";
        assert_eq!(test_names(listing), ["a::one", "b::two", "c"]);
    }

    #[test]
    fn batches_by_length() {
        assert!(batches(&[], 10).is_empty());
        assert_eq!(
            batches(&["aaaa", "bbbb", "cc"], 10),
            [vec!["aaaa", "bbbb"], vec!["cc"]]
        );
        assert_eq!(
            batches(&["toolongname", "a"], 4),
            [vec!["toolongname"], vec!["a"]]
        );
    }
}
