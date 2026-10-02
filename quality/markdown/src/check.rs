use std::collections::{BTreeMap, BTreeSet};

use clap::error::ErrorKind;
use clap::Parser as ClapParser;
use pulldown_cmark::{BrokenLink, Event, LinkType, Options, Parser, Tag, TagEnd};
use serde::Serialize;

use super::frontmatter::{
    check_headings, code_regions, heading, is_closed_fence, line_of, line_starts, slug, Heading,
};
use super::links::{broken_label, check_target, scan_bare_autolinks, BrokenRef, DocLink, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FindingKind {
    MissingFileTarget,
    MissingAnchor,
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
    pub skipped_remotes: Vec<String>,
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

pub fn check_markdown(
    source_path: &str,
    text: &str,
    siblings: &BTreeMap<String, String>,
) -> CheckOutcome {
    let mut outcome = CheckOutcome::default();
    let mut headings: Vec<Heading> = Vec::new();
    let mut doc_links: Vec<DocLink> = Vec::new();
    let mut slug_counts: BTreeMap<String, usize> = BTreeMap::new();

    let starts = line_starts(text);
    let regions = code_regions(text, &starts);
    let source_lines: Vec<&str> = text.lines().collect();
    let mut suppressed: BTreeSet<u32> = BTreeSet::new();
    for region in &regions {
        for line in region.start_line..=region.end_line {
            suppressed.insert(line);
        }
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
        if !is_closed_fence(&source_lines, region) {
            push_finding(
                &mut outcome,
                region.start_line,
                FindingKind::UnclosedCodeFence,
                "fenced code block opened here is never closed",
            );
        }
    }

    let mut heading_lines: Vec<u32> = Vec::new();
    let mut code_spans: Vec<Span> = Vec::new();
    let mut link_spans: Vec<Span> = Vec::new();
    let mut html_spans: Vec<Span> = Vec::new();
    let mut html_start: Option<usize> = None;
    let mut broken: Vec<BrokenRef> = Vec::new();
    {
        let callbacks = &mut |link: BrokenLink| {
            broken.push(BrokenRef {
                span: link.span.clone(),
                link_type: link.link_type,
                reference: link.reference.to_string(),
            });
            None
        };
        let parser = Parser::new_with_broken_link_callback(text, Options::empty(), Some(callbacks));
        for (event, range) in parser.into_offset_iter() {
            match event {
                Event::Start(Tag::Heading { .. }) => {
                    heading_lines.push(line_of(&starts, range.start));
                }
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    ..
                })
                | Event::Start(Tag::Image {
                    link_type,
                    dest_url,
                    ..
                }) => {
                    if link_type == LinkType::Email {
                        link_spans.push(range);
                        continue;
                    }
                    let at = line_of(&starts, range.start);
                    link_spans.push(range.clone());
                    doc_links.push(DocLink::Dest {
                        line: at,
                        col: range.start,
                        target: dest_url.to_string(),
                    });
                }
                Event::Code(_) => {
                    code_spans.push(range);
                }
                Event::Start(Tag::HtmlBlock) => {
                    html_start = Some(range.start);
                }
                Event::End(TagEnd::HtmlBlock) => {
                    if let Some(start) = html_start.take() {
                        for line in line_of(&starts, start)..=line_of(&starts, range.end.max(1) - 1)
                        {
                            suppressed.insert(line);
                        }
                    }
                }
                Event::Html(_) | Event::InlineHtml(_) => {
                    html_spans.push(range);
                }
                _ => {}
            }
        }
    }
    for broken_ref in &broken {
        if broken_ref.link_type == LinkType::Shortcut {
            continue;
        }
        let at = line_of(&starts, broken_ref.span.start);
        doc_links.push(DocLink::UndefinedRef {
            line: at,
            col: broken_ref.span.start,
            label: broken_label(text, broken_ref),
        });
    }

    for (index, line) in source_lines.iter().enumerate() {
        let line_no = (index as u32) + 1;
        if suppressed.contains(&line_no) {
            continue;
        }
        let base = starts[index];
        doc_links.extend(scan_bare_autolinks(
            line,
            base,
            line_no,
            &code_spans,
            &link_spans,
            &html_spans,
        ));
    }

    for line_no in heading_lines {
        let line = source_lines
            .get((line_no - 1) as usize)
            .copied()
            .unwrap_or("");
        if let Some((level, text)) = heading(line) {
            let base = slug(&text);
            let slug = if base.is_empty() {
                base
            } else {
                let count = slug_counts.entry(base.clone()).or_insert(0);
                let slug = if *count == 0 {
                    base.clone()
                } else {
                    format!("{base}-{}", *count)
                };
                *count += 1;
                slug
            };
            headings.push(Heading {
                level,
                line: line_no,
                slug,
            });
        }
    }

    doc_links.sort_by_key(|link| (link.line(), link.col()));
    check_headings(&headings, &mut outcome);
    let own_slugs: BTreeSet<String> = headings.into_iter().map(|h| h.slug).collect();
    for link in &doc_links {
        match link {
            DocLink::Dest { line, target, .. } => {
                check_target(
                    source_path,
                    *line,
                    target,
                    siblings,
                    &own_slugs,
                    &mut outcome,
                );
            }
            DocLink::UndefinedRef { line, label, .. } => {
                push_finding(
                    &mut outcome,
                    *line,
                    FindingKind::MissingFileTarget,
                    &format!("reference link label has no definition: [{label}]"),
                );
            }
        }
    }
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
        FindingKind::MissingFileTarget => "missing-file-target",
        FindingKind::MissingAnchor => "missing-anchor",
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
    let sibling_specs: Vec<(String, String)> = cli
        .sibling
        .into_iter()
        .map(|mapping| (mapping.ws, mapping.exec))
        .collect();

    let mut siblings: BTreeMap<String, String> = BTreeMap::new();
    let mut ordered_ws: Vec<String> = Vec::new();
    for (ws, _) in &sources {
        if !ordered_ws.contains(ws) {
            ordered_ws.push(ws.clone());
        }
    }
    for (ws, exec) in sources.iter().chain(sibling_specs.iter()) {
        if siblings.contains_key(ws) {
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
        siblings.insert(ws.clone(), text);
    }

    let mut seen_remotes: Vec<String> = Vec::new();
    for ws in &ordered_ws {
        let text = siblings.get(ws).cloned().unwrap_or_default();
        let outcome = check_markdown(ws, &text, &siblings);
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
        for remote in &outcome.skipped_remotes {
            if !seen_remotes.iter().any(|seen| seen == remote) {
                seen_remotes.push(remote.clone());
                print_err(&format!("skipped remote target (never fetched): {remote}"));
            }
        }
    }
    0
}
