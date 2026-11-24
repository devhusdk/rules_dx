use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

pub use dx_atomic_fs::path_io::{self, PathIoError};

use crate::proto::DocIr;

#[must_use]
pub fn run_process<O, PE: Display, RE: Display>(
    program: &str,
    usage: &str,
    parse: impl FnOnce(&[String]) -> Result<O, PE>,
    run: impl FnOnce(&O) -> Result<(), RE>,
    args: &[String],
) -> i32 {
    let mut stdout = String::new();
    let mut stderr = String::new();
    let code = execute(args, program, usage, parse, run, &mut stdout, &mut stderr);
    if !stdout.is_empty() {
        println!("{stdout}");
    }
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
    i32::from(code)
}

fn execute<O, PE: Display, RE: Display>(
    args: &[String],
    program: &str,
    usage: &str,
    parse: impl FnOnce(&[String]) -> Result<O, PE>,
    run: impl FnOnce(&O) -> Result<(), RE>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlagError {
    Unknown { flag: String },
    MissingValue { flag: String },
    Empty { flag: String },
    Required { flag: String },
    Repeated { flag: String },
}

impl Display for FlagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { flag } => write!(f, "unknown argument '{flag}'"),
            Self::MissingValue { flag } => write!(f, "{flag} needs a value"),
            Self::Empty { flag } => write!(f, "{flag} must not be empty"),
            Self::Required { flag } => write!(f, "{flag} is required"),
            Self::Repeated { flag } => write!(f, "{flag} given more than once"),
        }
    }
}

impl std::error::Error for FlagError {}

impl From<FlagError> for String {
    fn from(error: FlagError) -> Self {
        error.to_string()
    }
}

#[derive(Debug)]
pub struct Flags {
    pairs: Vec<(String, String)>,
}

pub fn flags(args: &[String], known: &[&str]) -> Result<Flags, FlagError> {
    let mut pairs = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let Some(flag) = args.get(index).map(String::as_str) else {
            break;
        };
        if !known.contains(&flag) {
            return Err(FlagError::Unknown {
                flag: flag.to_owned(),
            });
        }
        let Some(value) = args.get(index + 1) else {
            return Err(FlagError::MissingValue {
                flag: flag.to_owned(),
            });
        };
        if value.is_empty() {
            return Err(FlagError::Empty {
                flag: flag.to_owned(),
            });
        }
        pairs.push((flag.to_owned(), value.clone()));
        index += 2;
    }
    Ok(Flags { pairs })
}

impl Flags {
    pub fn one(&mut self, flag: &str) -> Result<String, FlagError> {
        self.optional(flag)?.ok_or_else(|| FlagError::Required {
            flag: flag.to_owned(),
        })
    }

    pub fn optional(&mut self, flag: &str) -> Result<Option<String>, FlagError> {
        let mut values = self.many(flag);
        if values.len() > 1 {
            return Err(FlagError::Repeated {
                flag: flag.to_owned(),
            });
        }
        Ok(values.pop())
    }

    pub fn many(&mut self, flag: &str) -> Vec<String> {
        let (taken, rest) = std::mem::take(&mut self.pairs)
            .into_iter()
            .partition(|(name, _)| name == flag);
        self.pairs = rest;
        taken.into_iter().map(|(_, value)| value).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub exec_path: String,
    pub rel: String,
    pub generated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcesError {
    pub line: String,
}

impl Display for SourcesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad srcs manifest line '{}'", self.line)
    }
}

impl std::error::Error for SourcesError {}

impl From<SourcesError> for String {
    fn from(error: SourcesError) -> Self {
        error.to_string()
    }
}

pub fn parse_sources(text: &str) -> Result<Vec<Source>, SourcesError> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let bad = || SourcesError {
                line: line.to_owned(),
            };
            let fields: Vec<&str> = line.split('\t').collect();
            let [exec_path, rel, generated] = fields.as_slice() else {
                return Err(bad());
            };
            let generated = match *generated {
                "1" => true,
                "0" => false,
                _ => return Err(bad()),
            };
            Ok(Source {
                exec_path: (*exec_path).to_owned(),
                rel: (*rel).to_owned(),
                generated,
            })
        })
        .collect()
}

#[derive(Debug)]
pub enum WriteShardError {
    Encode(crate::Error),
    Write(PathIoError),
}

impl Display for WriteShardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(f, "cannot encode IR: {error:?}"),
            Self::Write(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for WriteShardError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(_) => None,
            Self::Write(error) => Some(error),
        }
    }
}

impl From<WriteShardError> for String {
    fn from(error: WriteShardError) -> Self {
        error.to_string()
    }
}

pub fn write_shard(out: &str, shard: &DocIr) -> Result<(), WriteShardError> {
    let bytes = crate::encode_shard(shard).map_err(WriteShardError::Encode)?;
    let path = Path::new(out);
    dx_atomic_fs::write_scratch(path, &bytes).map_err(|source| {
        WriteShardError::Write(PathIoError::Write {
            path: PathBuf::from(path),
            source,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn rejected(list: &[&str]) -> (FlagError, String) {
        let error = flags(&args(list), &["--a", "--b"]).expect_err("rejected");
        let text = error.to_string();
        (error, text)
    }

    fn flag(name: &str) -> String {
        name.to_owned()
    }

    fn run_main<E: Display>(list: &[&str], run_result: Result<(), E>) -> (u8, String, String) {
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
    fn run_process_returns_two_on_a_parse_error() {
        let code = run_process(
            "prog",
            "usage: prog",
            |args| flags(args, &["--a"]).map(|_| ()),
            |()| Ok::<(), FlagError>(()),
            &args(&["--x", "1"]),
        );
        assert_eq!(code, 2);
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert_eq!(
            rejected(&["--x", "1"]),
            (
                FlagError::Unknown { flag: flag("--x") },
                "unknown argument '--x'".to_owned()
            )
        );
    }

    #[test]
    fn missing_value_is_rejected() {
        assert_eq!(
            rejected(&["--a"]),
            (
                FlagError::MissingValue { flag: flag("--a") },
                "--a needs a value".to_owned()
            )
        );
    }

    #[test]
    fn empty_value_is_rejected() {
        assert_eq!(
            rejected(&["--a", ""]),
            (
                FlagError::Empty { flag: flag("--a") },
                "--a must not be empty".to_owned()
            )
        );
    }

    #[test]
    fn duplicate_one_is_rejected() {
        let mut parsed = flags(&args(&["--a", "1", "--a", "2"]), &["--a"]).unwrap();
        let error = parsed.one("--a").expect_err("repeated");
        assert_eq!(error, FlagError::Repeated { flag: flag("--a") });
        assert_eq!(error.to_string(), "--a given more than once");
    }

    #[test]
    fn duplicate_optional_is_rejected() {
        let mut parsed = flags(&args(&["--a", "1", "--a", "2"]), &["--a"]).unwrap();
        assert_eq!(
            parsed.optional("--a"),
            Err(FlagError::Repeated { flag: flag("--a") })
        );
    }

    #[test]
    fn missing_one_is_required() {
        let mut parsed = flags(&args(&[]), &["--a"]).unwrap();
        let error = parsed.one("--a").expect_err("required");
        assert_eq!(error, FlagError::Required { flag: flag("--a") });
        assert_eq!(error.to_string(), "--a is required");
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
            run_main(&["--help"], Ok::<(), String>(())),
            (0, "usage: prog".to_owned(), String::new())
        );
    }

    #[test]
    fn parse_error_exits_two_with_usage() {
        assert_eq!(
            run_main(&["--x", "1"], Ok::<(), String>(())),
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
    fn typed_run_error_prints_its_display() {
        assert_eq!(
            run_main(
                &["--a", "1"],
                Err(SourcesError {
                    line: "x".to_owned()
                })
            ),
            (
                1,
                String::new(),
                "prog: bad srcs manifest line 'x'".to_owned()
            )
        );
    }

    #[test]
    fn string_shims_keep_the_text() {
        assert_eq!(
            String::from(FlagError::Required { flag: flag("--a") }),
            "--a is required"
        );
        assert_eq!(
            String::from(SourcesError {
                line: "x".to_owned()
            }),
            "bad srcs manifest line 'x'"
        );
        assert_eq!(
            String::from(WriteShardError::Encode(crate::Error::EmptyLanguage)),
            "cannot encode IR: EmptyLanguage"
        );
    }

    #[test]
    fn write_shard_reports_unwritable_path() {
        let exe = std::env::current_exe().unwrap_or_default();
        let out = exe.join("shard.bin").display().to_string();
        let shard = DocIr {
            language: "rust".to_owned(),
            package: "pkg".to_owned(),
            ..DocIr::default()
        };
        let error = write_shard(&out, &shard).expect_err("unwritable");
        assert!(
            matches!(&error, WriteShardError::Write(PathIoError::Write { path, .. }) if path == Path::new(&out)),
            "{error:?}"
        );
        let detail = error.to_string();
        assert!(
            detail.starts_with(&format!("cannot write {out}: ")),
            "{detail}"
        );
    }

    #[test]
    fn sources_are_parsed() {
        assert_eq!(
            parse_sources("bazel-out/a.d\tpkg/a.d\t1\n\nb.d\tpkg/b.d\t0\n"),
            Ok(vec![
                Source {
                    exec_path: "bazel-out/a.d".to_owned(),
                    rel: "pkg/a.d".to_owned(),
                    generated: true,
                },
                Source {
                    exec_path: "b.d".to_owned(),
                    rel: "pkg/b.d".to_owned(),
                    generated: false,
                },
            ])
        );
    }

    #[test]
    fn malformed_source_lines_are_rejected() {
        for (text, line) in [
            ("a\tb\n", "a\tb"),
            ("a\tb\t0\tc\n", "a\tb\t0\tc"),
            ("a\tb\tyes\n", "a\tb\tyes"),
        ] {
            let error = parse_sources(text).expect_err("malformed");
            assert_eq!(
                error,
                SourcesError {
                    line: line.to_owned()
                }
            );
            assert_eq!(
                error.to_string(),
                format!("bad srcs manifest line '{line}'")
            );
        }
    }
}
