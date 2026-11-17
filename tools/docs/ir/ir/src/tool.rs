//! Shared entry point, flag reader and shard writer for the docs extractor binaries.

use std::fmt::Display;
use std::process::ExitCode;

use dx_error::io::{IoContext as _, IoError};

use crate::proto::DocIr;

/// Prints usage on -h/--help, exits 2 with "<program>: <detail>\n<usage>" on a parse error, 1 with "<program>: <detail>" on a run error.
pub fn main<O, P: Display, R: Display>(
    program: &str,
    usage: &str,
    parse: impl FnOnce(&[String]) -> Result<O, P>,
    run: impl FnOnce(&O) -> Result<(), R>,
) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let code = execute(&args, program, usage, parse, run, &mut stdout, &mut stderr);
    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    ExitCode::from(code)
}

fn execute<O, P: Display, R: Display>(
    args: &[String],
    program: &str,
    usage: &str,
    parse: impl FnOnce(&[String]) -> Result<O, P>,
    run: impl FnOnce(&O) -> Result<(), R>,
    stdout: &mut String,
    stderr: &mut String,
) -> u8 {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        stdout.push_str(usage);
        return 0;
    }
    let options = match parse(args) {
        Ok(options) => options,
        Err(detail) => {
            *stderr = format!("{program}: {detail}\n{usage}");
            return 2;
        }
    };
    match run(&options) {
        Ok(()) => 0,
        Err(detail) => {
            *stderr = format!("{program}: {detail}");
            1
        }
    }
}

/// A command line that does not match the extractor's flags.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlagError {
    #[error("unknown argument '{0}'")]
    Unknown(String),
    #[error("{0} needs a value")]
    MissingValue(String),
    #[error("{0} must not be empty")]
    Empty(String),
    #[error("{0} is required")]
    Required(String),
    #[error("{0} given more than once")]
    Repeated(String),
}

/// A shard that cannot be encoded or written.
#[derive(Debug, thiserror::Error)]
pub enum WriteShardError {
    #[error("cannot encode IR: {0:?}")]
    Encode(crate::Error),
    #[error(transparent)]
    Write(#[from] IoError),
}

/// The `--flag VALUE` pairs of one invocation.
#[derive(Debug)]
pub struct Flags {
    pairs: Vec<(String, String)>,
}

/// Reads `--flag VALUE` pairs; rejects unknown flags, missing values and empty values.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn flags(args: &[String], known: &[&str]) -> Result<Flags, FlagError> {
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let Some(flag) = args.get(index).map(String::as_str) else {
            break;
        };
        if !known.contains(&flag) {
            return Err(FlagError::Unknown(flag.to_owned()));
        }
        let Some(value) = args.get(index + 1) else {
            return Err(FlagError::MissingValue(flag.to_owned()));
        };
        if value.is_empty() {
            return Err(FlagError::Empty(flag.to_owned()));
        }
        pairs.push((flag.to_owned(), value.clone()));
        index += 2;
    }
    Ok(Flags { pairs })
}

impl Flags {
    /// The value of a flag that must appear exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn one(&mut self, flag: &str) -> Result<String, FlagError> {
        self.optional(flag)?
            .ok_or_else(|| FlagError::Required(flag.to_owned()))
    }

    /// The value of a flag that may appear at most once.
    ///
    /// # Errors
    ///
    /// Returns an error when the operation fails.
    pub fn optional(&mut self, flag: &str) -> Result<Option<String>, FlagError> {
        let mut values = self.many(flag);
        if values.len() > 1 {
            return Err(FlagError::Repeated(flag.to_owned()));
        }
        Ok(values.pop())
    }

    /// Every value of a repeatable flag, in order.
    pub fn many(&mut self, flag: &str) -> Vec<String> {
        let (taken, rest) = std::mem::take(&mut self.pairs)
            .into_iter()
            .partition(|(name, _)| name == flag);
        self.pairs = rest;
        taken.into_iter().map(|(_, value)| value).collect()
    }
}

/// Encodes and writes one shard.
///
/// # Errors
///
/// Returns an error when the operation fails.
pub fn write_shard(out: &str, shard: &DocIr) -> Result<(), WriteShardError> {
    let bytes = crate::encode_shard(shard).map_err(WriteShardError::Encode)?;
    std::fs::write(out, bytes).write_context(out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn rejected(list: &[&str]) -> String {
        match flags(&args(list), &["--a", "--b"]) {
            Err(detail) => detail.to_string(),
            Ok(_) => String::new(),
        }
    }

    fn run_main(list: &[&str], run_result: Result<(), String>) -> (u8, String, String) {
        let mut stdout = String::new();
        let mut stderr = String::new();
        let code = execute(
            &args(list),
            "prog",
            "usage: prog",
            |args| flags(args, &["--a"]).map(|_| ()),
            |()| run_result,
            &mut stdout,
            &mut stderr,
        );
        (code, stdout, stderr)
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert_eq!(rejected(&["--x", "1"]), "unknown argument '--x'");
    }

    #[test]
    fn missing_value_is_rejected() {
        assert_eq!(rejected(&["--a"]), "--a needs a value");
    }

    #[test]
    fn empty_value_is_rejected() {
        assert_eq!(rejected(&["--a", ""]), "--a must not be empty");
    }

    #[test]
    fn duplicate_one_is_rejected() {
        let mut parsed = flags(&args(&["--a", "1", "--a", "2"]), &["--a"]).unwrap();
        assert_eq!(
            parsed.one("--a"),
            Err(FlagError::Repeated("--a".to_owned()))
        );
    }

    #[test]
    fn duplicate_optional_is_rejected() {
        let mut parsed = flags(&args(&["--a", "1", "--a", "2"]), &["--a"]).unwrap();
        assert_eq!(
            parsed.optional("--a"),
            Err(FlagError::Repeated("--a".to_owned()))
        );
    }

    #[test]
    fn missing_one_is_required() {
        let mut parsed = flags(&args(&[]), &["--a"]).unwrap();
        assert_eq!(parsed.one("--a"), Err(FlagError::Required("--a".to_owned())));
    }

    #[test]
    fn values_are_read() {
        let mut parsed = flags(
            &args(&["--a", "1", "--b", "2", "--b", "3"]),
            &["--a", "--b", "--c"],
        )
        .unwrap();
        assert_eq!(parsed.one("--a"), Ok("1".to_owned()));
        assert_eq!(parsed.optional("--c"), Ok(None));
        assert_eq!(parsed.many("--b"), vec!["2".to_owned(), "3".to_owned()]);
    }

    #[test]
    fn help_prints_usage_and_succeeds() {
        assert_eq!(
            run_main(&["--help"], Ok(())),
            (0, "usage: prog".to_owned(), String::new())
        );
    }

    #[test]
    fn parse_error_exits_two_with_usage() {
        assert_eq!(
            run_main(&["--x", "1"], Ok(())),
            (
                2,
                String::new(),
                "prog: unknown argument '--x'\nusage: prog".to_owned()
            )
        );
    }

    #[test]
    fn run_error_exits_one() {
        assert_eq!(
            run_main(&["--a", "1"], Err("boom".to_owned())),
            (1, String::new(), "prog: boom".to_owned())
        );
    }

    #[test]
    fn write_shard_reports_unwritable_path() {
        let detail = write_shard("/nonexistent-dir/shard.bin", &DocIr::default())
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        assert!(
            detail.starts_with("cannot encode IR")
                || detail.starts_with("cannot write /nonexistent-dir/shard.bin"),
            "{detail}"
        );
    }
}
