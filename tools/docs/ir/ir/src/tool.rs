//! Shared entry point, flag reader and shard writer for the docs extractor binaries.

use std::process::ExitCode;

use crate::proto::DocIr;

/// Prints usage on -h/--help, exits 2 with "<program>: <detail>\n<usage>" on a parse error, 1 with "<program>: <detail>" on a run error.
pub fn main<O>(
    program: &str,
    usage: &str,
    parse: impl FnOnce(&[String]) -> Result<O, String>,
    run: impl FnOnce(&O) -> Result<(), String>,
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

fn execute<O>(
    args: &[String],
    program: &str,
    usage: &str,
    parse: impl FnOnce(&[String]) -> Result<O, String>,
    run: impl FnOnce(&O) -> Result<(), String>,
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

/// The `--flag VALUE` pairs of one invocation.
#[derive(Debug)]
pub struct Flags {
    pairs: Vec<(String, String)>,
}

/// Reads `--flag VALUE` pairs; rejects unknown flags, missing values and empty values.
pub fn flags(args: &[String], known: &[&str]) -> Result<Flags, String> {
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        if !known.contains(&flag) {
            return Err(format!("unknown argument '{flag}'"));
        }
        let Some(value) = args.get(index + 1) else {
            return Err(format!("{flag} needs a value"));
        };
        if value.is_empty() {
            return Err(format!("{flag} must not be empty"));
        }
        pairs.push((flag.to_owned(), value.clone()));
        index += 2;
    }
    Ok(Flags { pairs })
}

impl Flags {
    /// The value of a flag that must appear exactly once.
    pub fn one(&mut self, flag: &str) -> Result<String, String> {
        self.optional(flag)?
            .ok_or_else(|| format!("{flag} is required"))
    }

    /// The value of a flag that may appear at most once.
    pub fn optional(&mut self, flag: &str) -> Result<Option<String>, String> {
        let mut values = self.many(flag);
        if values.len() > 1 {
            return Err(format!("{flag} given more than once"));
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
pub fn write_shard(out: &str, shard: &DocIr) -> Result<(), String> {
    let bytes =
        crate::encode_shard(shard).map_err(|error| format!("cannot encode IR: {error:?}"))?;
    std::fs::write(out, bytes).map_err(|error| format!("cannot write {out}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn rejected(list: &[&str]) -> String {
        match flags(&args(list), &["--a", "--b"]) {
            Err(detail) => detail,
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
            Err("--a given more than once".to_owned())
        );
    }

    #[test]
    fn duplicate_optional_is_rejected() {
        let mut parsed = flags(&args(&["--a", "1", "--a", "2"]), &["--a"]).unwrap();
        assert_eq!(
            parsed.optional("--a"),
            Err("--a given more than once".to_owned())
        );
    }

    #[test]
    fn missing_one_is_required() {
        let mut parsed = flags(&args(&[]), &["--a"]).unwrap();
        assert_eq!(parsed.one("--a"), Err("--a is required".to_owned()));
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
            .unwrap_or_default();
        assert!(
            detail.starts_with("cannot encode IR")
                || detail.starts_with("cannot write /nonexistent-dir/shard.bin"),
            "{detail}"
        );
    }
}
