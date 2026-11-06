use std::collections::BTreeMap;

use clap::error::ErrorKind;
use clap::Parser as ClapParser;
use serde::Serialize;

use super::frontmatter::{check_headings, code_regions, headings, line_starts};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingKind {
    HeadingHierarchy,
    MissingCodeFenceLanguage,
    UnclosedCodeFence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub line: u32,
    pub kind: FindingKind,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckOutcome {
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MarkdownError {
    #[error("{message}")]
    Io { message: String },
}

impl From<String> for MarkdownError {
    fn from(message: String) -> Self {
        Self::Io { message }
    }
}

pub fn check_markdown(text: &str) -> CheckOutcome {
    let mut outcome = CheckOutcome::default();

    let starts = line_starts(text);
    let regions = code_regions(text, &starts);
    for region in &regions {
        let Some(info) = region.fence_info.as_deref() else {
            continue;
        };
        if info.trim().is_empty() {
            push_finding(
                &mut outcome,
                region.start_line,
                FindingKind::MissingCodeFenceLanguage,
                "fenced code block without a language tag",
            );
        }
        if region.unclosed {
            push_finding(
                &mut outcome,
                region.start_line,
                FindingKind::UnclosedCodeFence,
                "fenced code block opened here is never closed",
            );
        }
    }

    check_headings(&headings(text, &starts), &mut outcome);
    outcome.findings.sort_by_key(|f| (f.line, f.kind));
    outcome
}

pub(crate) fn push_finding(
    outcome: &mut CheckOutcome,
    line: u32,
    kind: FindingKind,
    message: &str,
) {
    outcome.findings.push(Finding {
        line,
        kind,
        message: message.to_string(),
    });
}

#[derive(Serialize)]
pub(crate) struct FindingLine<'a> {
    pub(crate) path: &'a str,
    pub(crate) line: u32,
    pub(crate) kind: &'static str,
    pub(crate) message: &'a str,
}
pub fn kind_id(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::HeadingHierarchy => "heading-hierarchy",
        FindingKind::MissingCodeFenceLanguage => "missing-code-fence-language",
        FindingKind::UnclosedCodeFence => "unclosed-code-fence",
    }
}

fn usage() -> String {
    "usage: quality_markdown --source WS_PATH=EXEC_PATH [--source ...] [--sibling WS_PATH=EXEC_PATH ...]".into()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mapping {
    ws: String,
    exec: String,
}

#[derive(ClapParser)]
#[command(disable_help_flag = true)]
struct Cli {
    #[arg(long, allow_hyphen_values = true, value_parser = parse_source_mapping)]
    source: Vec<Mapping>,
    #[arg(long, allow_hyphen_values = true, value_parser = parse_sibling_mapping)]
    sibling: Vec<Mapping>,
}

enum EmptyRejection {
    Attached(String),
    Separate(&'static str),
    Neither,
}

fn empty_rejection(args: &[String]) -> EmptyRejection {
    for (index, arg) in args.iter().enumerate() {
        if arg == "--source=" || arg == "--sibling=" {
            return EmptyRejection::Attached(arg.clone());
        }
        if arg.is_empty() && index > 0 {
            if args[index - 1] == "--sibling" {
                return EmptyRejection::Separate("--sibling");
            }
            if args[index - 1] == "--source" {
                return EmptyRejection::Separate("--source");
            }
        }
    }
    EmptyRejection::Neither
}

fn check_mapping(flag: &str, raw: &str, error: &clap::Error) -> Vec<String> {
    let legacy = if flag == "--sibling" {
        parse_sibling_mapping(raw)
    } else {
        parse_source_mapping(raw)
    };
    match legacy {
        Err(message) => vec![message, usage()],
        Ok(_) => vec![dx_output::first_line(error), usage()],
    }
}

fn rejecting_flag(args: &[String], raw: &str) -> &'static str {
    for (index, arg) in args.iter().enumerate() {
        if arg == raw && index > 0 {
            if args[index - 1] == "--sibling" {
                return "--sibling";
            }
            if args[index - 1] == "--source" {
                return "--source";
            }
        }
    }
    for flag in ["--source", "--sibling"] {
        if args.iter().any(|arg| arg == &format!("{flag}={raw}")) {
            return flag;
        }
    }
    "--source"
}

fn parse_error(error: clap::Error, args: &[String]) -> Vec<String> {
    match error.kind() {
        ErrorKind::UnknownArgument => {
            vec![
                format!(
                    "unknown argument: {}",
                    dx_output::unknown_token(&error, args)
                ),
                usage(),
            ]
        }
        ErrorKind::InvalidValue => {
            let flag = dx_output::missing_value_flag(&error);
            vec![format!("missing value for {flag}"), usage()]
        }
        ErrorKind::ValueValidation => {
            if let Some(raw) = dx_output::rejected_value(&error) {
                let flag = rejecting_flag(args, &raw);
                return check_mapping(flag, &raw, &error);
            }
            match empty_rejection(args) {
                EmptyRejection::Attached(echoed) => {
                    vec![format!("unknown argument: {echoed}"), usage()]
                }
                EmptyRejection::Separate(flag) => check_mapping(flag, "", &error),
                EmptyRejection::Neither => check_mapping("--source", "", &error),
            }
        }
        _ => vec![dx_output::first_line(&error), usage()],
    }
}

fn parse_args(args: &[String]) -> Result<Cli, Vec<String>> {
    Cli::try_parse_from(
        std::iter::once("quality_markdown").chain(args.iter().map(|arg| arg as &str)),
    )
    .map_err(|error| parse_error(error, args))
}

fn parse_mapping(flag: &'static str, raw: &str) -> Result<Mapping, String> {
    match raw.split_once('=') {
        Some((ws, exec)) if !ws.is_empty() && !exec.is_empty() => Ok(Mapping {
            ws: ws.to_string(),
            exec: exec.to_string(),
        }),
        _ => Err(format!("malformed {flag} {raw:?}, want WS_PATH=EXEC_PATH")),
    }
}

fn parse_source_mapping(raw: &str) -> Result<Mapping, String> {
    parse_mapping("--source", raw)
}

fn parse_sibling_mapping(raw: &str) -> Result<Mapping, String> {
    parse_mapping("--sibling", raw)
}

pub fn run_cli(
    args: &[String],
    read_file: &dyn Fn(&str) -> Result<Vec<u8>, MarkdownError>,
    print_out: &mut dyn FnMut(&str),
    print_err: &mut dyn FnMut(&str),
) -> i32 {
    let cli = match parse_args(args) {
        Ok(cli) => cli,
        Err(lines) => {
            for line in &lines {
                print_err(line);
            }
            return 2;
        }
    };
    let sources: Vec<(String, String)> = cli
        .source
        .into_iter()
        .map(|mapping| (mapping.ws, mapping.exec))
        .collect();
    let mut ordered_ws: Vec<String> = Vec::new();
    for (ws, _) in &sources {
        if !ordered_ws.contains(ws) {
            ordered_ws.push(ws.clone());
        }
    }
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    for (ws, exec) in &sources {
        if texts.contains_key(ws) {
            continue;
        }
        let bytes = match read_file(exec) {
            Ok(bytes) => bytes,
            Err(message) => {
                print_err(&format!("cannot read {ws}: {message}"));
                return 2;
            }
        };
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                print_err(&format!("not UTF-8: {ws}"));
                return 2;
            }
        };
        texts.insert(ws.clone(), text);
    }

    for ws in &ordered_ws {
        let text = texts.get(ws).map(String::as_str).unwrap_or_default();
        let outcome = check_markdown(text);
        for finding in &outcome.findings {
            let line = FindingLine {
                path: ws.as_str(),
                line: finding.line,
                kind: kind_id(finding.kind),
                message: finding.message.as_str(),
            };
            match dx_fingerprint::to_json(&line) {
                Ok(rendered) => print_out(&rendered),
                Err(error) => print_err(&error.to_string()),
            }
        }
    }
    0
}
