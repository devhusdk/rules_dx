use std::collections::{BTreeMap, BTreeSet};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use super::check::{push_finding, CheckOutcome, FindingKind};

pub(crate) struct Heading {
    pub(crate) level: usize,
    pub(crate) line: u32,
    pub(crate) slug: String,
    pub(crate) top_level: bool,
}

pub(crate) fn check_headings(headings: &[Heading], outcome: &mut CheckOutcome) {
    if headings.is_empty() {
        return;
    }
    let top: Vec<&Heading> = headings.iter().filter(|h| h.top_level).collect();
    if top.is_empty() {
        return;
    }
    let mut h1_lines = top.iter().filter(|h| h.level == 1).map(|h| h.line);
    match h1_lines.next() {
        None => push_finding(
            outcome,
            top[0].line,
            FindingKind::HeadingHierarchy,
            "no H1 heading: exactly one H1 is required",
        ),
        Some(_) => {
            for extra in h1_lines {
                push_finding(
                    outcome,
                    extra,
                    FindingKind::HeadingHierarchy,
                    "multiple H1 headings: exactly one H1 is required",
                );
            }
        }
    }
    let mut previous = top[0].level;
    for heading in top.iter().skip(1) {
        if heading.level > previous + 1 {
            push_finding(
                outcome,
                heading.line,
                FindingKind::HeadingHierarchy,
                &format!(
                    "skipped heading level: H{} follows H{previous}",
                    heading.level
                ),
            );
        }
        previous = heading.level;
    }
}

pub(crate) fn slugs_in(content: &str) -> BTreeSet<String> {
    headings(content, &line_starts(content))
        .into_iter()
        .map(|h| h.slug)
        .collect()
}

pub(crate) fn headings(text: &str, starts: &[usize]) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut depth = 0usize;
    let mut open: Option<(usize, u32, bool, String)> = None;
    for (event, range) in Parser::new_ext(text, Options::empty()).into_offset_iter() {
        match event {
            Event::Start(Tag::BlockQuote(_)) | Event::Start(Tag::Item) => depth += 1,
            Event::End(TagEnd::BlockQuote(_)) | Event::End(TagEnd::Item) => {
                depth = depth.saturating_sub(1)
            }
            Event::Start(Tag::Heading { level, .. }) => {
                open = Some((
                    level as usize,
                    line_of(starts, range.start),
                    depth == 0,
                    String::new(),
                ));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, _, _, buf)) = open.as_mut() {
                    buf.push_str(&t);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some((_, _, _, buf)) = open.as_mut() {
                    buf.push(' ');
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, line, top_level, buf)) = open.take() {
                    let base = slug(&buf);
                    let slug = if base.is_empty() {
                        base
                    } else {
                        let count = counts.entry(base.clone()).or_insert(0);
                        let s = if *count == 0 {
                            base.clone()
                        } else {
                            format!("{base}-{}", *count)
                        };
                        *count += 1;
                        s
                    };
                    out.push(Heading {
                        level,
                        line,
                        slug,
                        top_level,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

pub(crate) fn line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }
    starts
}

pub(crate) fn line_of(starts: &[usize], offset: usize) -> u32 {
    starts.partition_point(|start| *start <= offset) as u32
}

pub(crate) struct CodeRegion {
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) fence_info: Option<String>,
    pub(crate) unclosed: bool,
}

pub(crate) fn code_regions(text: &str, starts: &[usize]) -> Vec<CodeRegion> {
    let mut regions = Vec::new();
    // (start_line, info, opening fence (char, len), end of opening line, last text end)
    type Open = (u32, Option<String>, Option<(char, usize)>, usize, usize);
    let mut open: Option<Open> = None;
    for (event, range) in Parser::new_ext(text, Options::empty()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => Some(info.to_string()),
                    CodeBlockKind::Indented => None,
                };
                let mut fence = None;
                let mut open_end = range.end;
                if info.is_some() {
                    let rest = text[range.start..].trim_start_matches([' ', '\t', '>']);
                    if let Some(c) = rest.chars().next().filter(|c| *c == '`' || *c == '~') {
                        fence = Some((c, rest.chars().take_while(|x| *x == c).count()));
                    }
                    open_end = text[range.start..range.end]
                        .find('\n')
                        .map_or(range.end, |i| range.start + i + 1);
                }
                open = Some((
                    line_of(starts, range.start),
                    info,
                    fence,
                    open_end,
                    open_end,
                ));
            }
            Event::Text(_) => {
                if let Some(o) = open.as_mut() {
                    o.4 = o.4.max(range.end.min(text.len()));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((start_line, fence_info, fence, _, content_end)) = open.take() {
                    let end_line = line_of(starts, range.end.saturating_sub(1)).max(start_line);
                    let unclosed = match (fence_info.is_some(), fence) {
                        (false, _) => false,
                        (true, None) => true,
                        (true, Some((c, len))) => {
                            let from = content_end.min(range.end);
                            let tail = text[from..range.end]
                                .trim_start_matches(|x: char| x.is_whitespace() || x == '>')
                                .trim_end();
                            !(tail.len() >= len && tail.chars().all(|x| x == c))
                        }
                    };
                    regions.push(CodeRegion {
                        start_line,
                        end_line,
                        fence_info,
                        unclosed,
                    });
                }
            }
            _ => {}
        }
    }
    regions
}

pub fn slug(text: &str) -> String {
    slug::slugify(deunicode::deunicode(text))
}
