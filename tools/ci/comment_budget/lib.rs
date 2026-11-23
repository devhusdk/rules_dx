use std::collections::BTreeMap;

use dx_guard::allowlist::{Counts, add};

pub const BUDGET: &str = "tools/ci/comment_budget.txt";

pub const UPDATE: &str = "tools/dx run //tools/ci:comment_budget_update";

pub const KIND: &str = "comment";

pub const MIN_FILES: usize = 3000;

pub const INPUTS: [&str; 3] = ["DX_SOURCES_JSON", "DX_RUST_JSON", "DX_OTHER_CODE_JSON"];

const SKIPPED_DIRS: [&str; 6] = [
    "fixture",
    "fixtures",
    "golden",
    "goldens",
    "testdata",
    "third_party",
];

const GENERATED: [&str; 3] = ["Code generated", "@generated", "DO NOT EDIT"];

const LICENSE: [&str; 3] = ["Copyright", "Licensed under", "SPDX-License-Identifier"];

const DIRECTIVES: [&str; 31] = [
    "+build",
    "-*- coding",
    "@ts-",
    "IWYU pragma",
    "NOLINT",
    "SAFETY:",
    "SPDX-License-Identifier",
    "Upstream:",
    "Why:",
    "buildifier:",
    "clang-format",
    "dx:",
    "eslint-disable",
    "eslint-enable",
    "floor:",
    "fmt:",
    "gazelle:",
    "go:",
    "isort:",
    "keep:",
    "noqa",
    "nolint",
    "pragma",
    "prettier-ignore",
    "pylint:",
    "ruff:",
    "shellcheck",
    "spellchecker:",
    "type:",
    "yaml-language-server",
    "zizmor:",
];

const CLAP_DERIVES: [&str; 4] = ["Args", "Parser", "Subcommand", "ValueEnum"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    C,
    Go,
    Js,
    Python,
    Rust,
    Shell,
    Yaml,
}

fn lang(path: &str) -> Option<Lang> {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name == "BUILD" || name == "BUILD.bazel" {
        return Some(Lang::Python);
    }
    match name.rsplit_once('.')?.1 {
        "cc" | "h" => Some(Lang::C),
        "go" => Some(Lang::Go),
        "js" | "mjs" | "ts" => Some(Lang::Js),
        "bzl" | "py" => Some(Lang::Python),
        "rs" => Some(Lang::Rust),
        "sh" => Some(Lang::Shell),
        "yml" | "yaml" => Some(Lang::Yaml),
        _ => None,
    }
}

pub fn skipped(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with(".dx/")
        || path.split('/').any(|part| {
            SKIPPED_DIRS.contains(&part)
                || part.ends_with("_fixture")
                || part.ends_with("_fixtures")
        })
        || name.contains(".golden")
        || name.contains("-lock.")
        || name.contains(".lock.")
}

#[derive(Debug, Default)]
struct Line {
    code: String,
    comments: Vec<String>,
}

enum State {
    Code,
    Block {
        depth: usize,
    },
    Quoted {
        close: String,
        escapes: bool,
        multiline: bool,
    },
    Regex {
        class: bool,
    },
}

enum Open {
    Line,
    Block,
    Quoted {
        open: usize,
        close: String,
        escapes: bool,
        multiline: bool,
    },
    Regex,
    Skip(usize),
}

fn quoted(open: usize, close: &str, escapes: bool, multiline: bool) -> Option<Open> {
    Some(Open::Quoted {
        open,
        close: close.to_owned(),
        escapes,
        multiline,
    })
}

fn starts(chars: &[char], at: usize, text: &str) -> bool {
    text.chars()
        .enumerate()
        .all(|(offset, expected)| chars.get(at + offset) == Some(&expected))
}

fn ident(c: Option<&char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || *c == '_')
}

fn indent(text: &str) -> usize {
    text.len() - text.trim_start_matches(' ').len()
}

fn rust_open(chars: &[char], at: usize) -> Option<Open> {
    let c = *chars.get(at)?;
    let raw = if matches!(c, 'b' | 'c') && chars.get(at + 1) == Some(&'r') {
        at + 1
    } else {
        at
    };
    if chars.get(raw) == Some(&'r') && !ident(at.checked_sub(1).and_then(|b| chars.get(b))) {
        let hashes = chars
            .get(raw + 1..)
            .unwrap_or_default()
            .iter()
            .take_while(|next| **next == '#')
            .count();
        if chars.get(raw + 1 + hashes) == Some(&'"') {
            let close = format!("\"{}", "#".repeat(hashes));
            return quoted(raw + 2 + hashes - at, &close, false, true);
        }
    }
    match c {
        '"' => quoted(1, "\"", true, true),
        '\'' if chars.get(at + 1) == Some(&'\\') => quoted(1, "'", true, false),
        '\'' if chars.get(at + 2) == Some(&'\'') => Some(Open::Skip(3)),
        _ => None,
    }
}

fn c_open(chars: &[char], at: usize) -> Option<Open> {
    let c = *chars.get(at)?;
    if c == 'R' && chars.get(at + 1) == Some(&'"') {
        let prefix: String = chars
            .get(..at)
            .unwrap_or_default()
            .iter()
            .rev()
            .take_while(|before| ident(Some(*before)))
            .collect();
        if ["", "8u", "u", "U", "L"].contains(&prefix.as_str()) {
            let delim: String = chars
                .get(at + 2..)
                .unwrap_or_default()
                .iter()
                .take_while(|next| **next != '(')
                .collect();
            if chars.get(at + 2 + delim.chars().count()) == Some(&'(') {
                let close = format!("){delim}\"");
                return quoted(3 + delim.chars().count(), &close, false, true);
            }
        }
    }
    match c {
        '"' => quoted(1, "\"", true, false),
        '\'' => quoted(1, "'", true, false),
        _ => None,
    }
}

fn yaml_scalar_start(code: &str) -> bool {
    let trimmed = code.trim_end();
    trimmed.is_empty()
        || trimmed.ends_with(['[', '{', ','])
        || (trimmed.len() < code.len() && trimmed.ends_with([':', '-', '?']))
}

fn block_scalar(text: &str, code: &str) -> Option<usize> {
    let token = code.split_whitespace().last()?;
    let mut rest = token.chars();
    if !matches!(rest.next(), Some('|' | '>'))
        || !rest.all(|c| c == '-' || c == '+' || c.is_ascii_digit())
    {
        return None;
    }
    let mut column = indent(text);
    let mut parent = column;
    let mut body = text.trim_start();
    while let Some(after) = body.strip_prefix("- ") {
        parent = column;
        column += 2 + after.len() - after.trim_start().len();
        body = after.trim_start();
    }
    Some(if body.starts_with(['|', '>']) {
        parent
    } else {
        column
    })
}

struct Scanner {
    lang: Lang,
    state: State,
    queued: Vec<(String, bool)>,
    heredocs: Vec<(String, bool)>,
    scalar: Option<usize>,
    last: Option<char>,
}

impl Scanner {
    fn heredoc(&mut self, chars: &[char], at: usize) -> Option<Open> {
        let mut end = at + 2;
        let dash = chars.get(end) == Some(&'-');
        end += usize::from(dash);
        while chars.get(end).is_some_and(|c| *c == ' ' || *c == '\t') {
            end += 1;
        }
        let mut word = String::new();
        if let Some(quote) = chars.get(end).copied().filter(|c| *c == '\'' || *c == '"') {
            end += 1;
            while let Some(c) = chars.get(end).filter(|c| **c != quote) {
                word.push(*c);
                end += 1;
            }
            end += 1;
        } else {
            while let Some(c) = chars
                .get(end)
                .filter(|c| !c.is_whitespace() && !";|&()<>".contains(**c))
            {
                if *c != '\\' {
                    word.push(*c);
                }
                end += 1;
            }
        }
        if !word.starts_with(|c: char| c.is_alphabetic() || c == '_') {
            return None;
        }
        self.queued.push((word, dash));
        Some(Open::Skip(end.min(chars.len()) - at))
    }

    fn open(&mut self, chars: &[char], at: usize, code: &str) -> Option<Open> {
        let c = *chars.get(at)?;
        let next = chars.get(at + 1).copied();
        let before = at.checked_sub(1).and_then(|b| chars.get(b)).copied();
        let slashes = matches!(self.lang, Lang::C | Lang::Go | Lang::Js | Lang::Rust);
        if slashes && c == '/' && next == Some('/') {
            return Some(Open::Line);
        }
        if slashes && c == '/' && next == Some('*') {
            return Some(Open::Block);
        }
        match self.lang {
            Lang::Rust => rust_open(chars, at),
            Lang::C => c_open(chars, at),
            Lang::Go => match c {
                '"' | '\'' => quoted(1, &c.to_string(), true, false),
                '`' => quoted(1, "`", false, true),
                _ => None,
            },
            Lang::Js => match c {
                '"' | '\'' => quoted(1, &c.to_string(), true, false),
                '`' => quoted(1, "`", true, true),
                '/' => {
                    let trimmed = code.trim_end();
                    let previous = trimmed.chars().last().or(self.last);
                    (previous.is_none_or(|p| "(,=:[!&|?{};+-*%<>~^".contains(p))
                        || trimmed.ends_with("return"))
                    .then_some(Open::Regex)
                }
                _ => None,
            },
            Lang::Python => match c {
                '#' => Some(Open::Line),
                '"' | '\'' => {
                    let triple = c.to_string().repeat(3);
                    if starts(chars, at, &triple) {
                        quoted(3, &triple, true, true)
                    } else {
                        quoted(1, &c.to_string(), true, false)
                    }
                }
                _ => None,
            },
            Lang::Shell => match c {
                '#' if before.is_none_or(|b| b.is_whitespace() || ";|&()".contains(b)) => {
                    Some(Open::Line)
                }
                '\\' => Some(Open::Skip(2)),
                '\'' => quoted(1, "'", before == Some('$'), true),
                '"' => quoted(1, "\"", true, true),
                '<' if next == Some('<')
                    && before != Some('<')
                    && chars.get(at + 2) != Some(&'<') =>
                {
                    self.heredoc(chars, at)
                }
                _ => None,
            },
            Lang::Yaml => match c {
                '#' if before.is_none_or(char::is_whitespace) => Some(Open::Line),
                '"' if yaml_scalar_start(code) => quoted(1, "\"", true, true),
                '\'' if yaml_scalar_start(code) => quoted(1, "'", false, true),
                _ => None,
            },
        }
    }

    fn line(&mut self, text: &str) -> Line {
        if let Some((delim, dash)) = self.heredocs.first() {
            let body = if *dash {
                text.trim_start_matches('\t')
            } else {
                text
            };
            if body == delim {
                self.heredocs.remove(0);
                return Line {
                    code: text.to_owned(),
                    comments: Vec::new(),
                };
            }
            return Line::default();
        }
        if let Some(column) = self.scalar {
            if text.trim().is_empty() || indent(text) > column {
                return Line::default();
            }
            self.scalar = None;
        }
        let chars: Vec<char> = text.chars().collect();
        let mut line = Line::default();
        let mut comment = String::new();
        let mut at = 0;
        while let Some(&c) = chars.get(at) {
            match self.state {
                State::Block { depth } => {
                    if self.lang == Lang::Rust && starts(&chars, at, "/*") {
                        comment.push_str("/*");
                        self.state = State::Block { depth: depth + 1 };
                        at += 2;
                    } else if starts(&chars, at, "*/") {
                        comment.push_str("*/");
                        at += 2;
                        self.state = if depth == 1 {
                            line.comments.push(std::mem::take(&mut comment));
                            State::Code
                        } else {
                            State::Block { depth: depth - 1 }
                        };
                    } else {
                        comment.push(c);
                        at += 1;
                    }
                }
                State::Quoted {
                    ref close, escapes, ..
                } => {
                    if escapes && c == '\\' {
                        at += 2;
                    } else if starts(&chars, at, close) {
                        line.code.push_str(close);
                        at += close.chars().count();
                        self.state = State::Code;
                    } else {
                        at += 1;
                    }
                }
                State::Regex { class } => {
                    at += if c == '\\' { 2 } else { 1 };
                    if c == '/' && !class {
                        line.code.push('/');
                        self.state = State::Code;
                    } else if c == '[' || c == ']' {
                        self.state = State::Regex { class: c == '[' };
                    }
                }
                State::Code => match self.open(&chars, at, &line.code) {
                    Some(Open::Line) => {
                        line.comments
                            .push(chars.get(at..).unwrap_or_default().iter().collect());
                        at = chars.len();
                    }
                    Some(Open::Block) => {
                        comment.push_str("/*");
                        self.state = State::Block { depth: 1 };
                        at += 2;
                    }
                    Some(Open::Quoted {
                        open,
                        close,
                        escapes,
                        multiline,
                    }) => {
                        line.code
                            .extend(chars.get(at..at + open).unwrap_or_default());
                        at += open;
                        self.state = State::Quoted {
                            close,
                            escapes,
                            multiline,
                        };
                    }
                    Some(Open::Regex) => {
                        line.code.push('/');
                        at += 1;
                        self.state = State::Regex { class: false };
                    }
                    Some(Open::Skip(len)) => {
                        let end = (at + len).min(chars.len());
                        line.code.extend(chars.get(at..end).unwrap_or_default());
                        at = end;
                    }
                    None => {
                        line.code.push(c);
                        at += 1;
                    }
                },
            }
        }
        match self.state {
            State::Block { .. } if !comment.trim().is_empty() => line.comments.push(comment),
            State::Quoted {
                multiline: false, ..
            }
            | State::Regex { .. } => self.state = State::Code,
            State::Code if self.lang == Lang::Yaml => {
                self.scalar = block_scalar(text, &line.code);
            }
            State::Block { .. } | State::Quoted { .. } | State::Code => {}
        }
        self.heredocs.append(&mut self.queued);
        self.last = line.code.trim_end().chars().last().or(self.last);
        line
    }
}

fn scan(lang: Lang, source: &str) -> Vec<Line> {
    let mut scanner = Scanner {
        lang,
        state: State::Code,
        queued: Vec::new(),
        heredocs: Vec::new(),
        scalar: None,
        last: None,
    };
    source.lines().map(|text| scanner.line(text)).collect()
}

fn directive(comment: &str) -> bool {
    let body = comment
        .trim_start_matches(['/', '*', '#', '!', ' ', '\t'])
        .trim_end();
    comment.contains("LCOV_EXCL_")
        || body == "keep"
        || DIRECTIVES.iter().any(|marker| body.starts_with(marker))
}

fn is_doc(comment: &str) -> bool {
    comment.starts_with("///") && !comment.starts_with("////")
}

fn pinned(code: &str) -> bool {
    code.contains("uses:")
        && code
            .split('@')
            .skip(1)
            .any(|rest| rest.chars().take_while(char::is_ascii_hexdigit).count() == 40)
}

fn license_header(lines: &[Line]) -> usize {
    let shebang = lines
        .first()
        .is_some_and(|line| line.comments.iter().any(|c| c.starts_with("#!")));
    let start = usize::from(shebang);
    let end = start
        + lines
            .iter()
            .skip(start)
            .take_while(|line| line.code.trim().is_empty() && !line.comments.is_empty())
            .count();
    let licensed = lines
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .flat_map(|line| &line.comments)
        .any(|comment| LICENSE.iter().any(|marker| comment.contains(marker)));
    if licensed { end } else { 0 }
}

fn attribute_or_doc(line: &Line) -> bool {
    let code = line.code.trim();
    code.starts_with("#[") || (code.is_empty() && line.comments.iter().any(|c| is_doc(c)))
}

fn item_end(lines: &[Line], from: usize) -> usize {
    let mut depth = 0usize;
    let mut opened = false;
    for (at, line) in lines.iter().enumerate().skip(from) {
        for c in line.code.chars() {
            match c {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    if opened && depth == 0 {
                        return at;
                    }
                }
                ';' if !opened => return at,
                _ => {}
            }
        }
    }
    lines.len().saturating_sub(1)
}

fn clap_lines(lines: &[Line]) -> Vec<bool> {
    let mut keep = vec![false; lines.len()];
    for (at, line) in lines.iter().enumerate() {
        if !line.code.trim_start().starts_with("#[derive(") {
            continue;
        }
        let derive_end = (at..lines.len())
            .find(|&end| lines.get(end).is_some_and(|l| l.code.contains(']')))
            .unwrap_or(at);
        let derive: Vec<&str> = lines
            .get(at..=derive_end)
            .unwrap_or_default()
            .iter()
            .map(|l| l.code.as_str())
            .collect();
        let clap = derive
            .join(" ")
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .any(|word| CLAP_DERIVES.contains(&word));
        if !clap {
            continue;
        }
        let start = (0..at)
            .rev()
            .take_while(|&before| lines.get(before).is_some_and(attribute_or_doc))
            .last()
            .unwrap_or(at);
        let end = item_end(lines, at);
        for flag in keep.get_mut(start..=end).unwrap_or_default() {
            *flag = true;
        }
    }
    keep
}

pub fn comment_lines(path: &str, source: &str) -> usize {
    let Some(lang) = lang(path) else {
        return 0;
    };
    let lines = scan(lang, source);
    let generated = lines
        .iter()
        .take(5)
        .flat_map(|line| &line.comments)
        .any(|comment| GENERATED.iter().any(|marker| comment.contains(marker)));
    if generated {
        return 0;
    }
    let header = license_header(&lines);
    let clap = if lang == Lang::Rust {
        clap_lines(&lines)
    } else {
        Vec::new()
    };
    lines
        .iter()
        .enumerate()
        .filter(|&(at, line)| {
            at >= header
                && !(lang == Lang::Yaml && pinned(&line.code))
                && line.comments.iter().any(|comment| {
                    !(directive(comment)
                        || (at == 0 && comment.starts_with("#!"))
                        || (is_doc(comment) && clap.get(at) == Some(&true)))
                })
        })
        .count()
}

pub fn scanned(mut files: BTreeMap<String, String>) -> BTreeMap<String, String> {
    files.retain(|path, _| lang(path).is_some() && !skipped(path));
    files
}

pub fn found(files: &BTreeMap<String, String>) -> Counts {
    let mut counts = Counts::new();
    for (path, text) in files {
        add(&mut counts, path, KIND, comment_lines(path, text));
    }
    counts
}

fn budget_of(budget: &Counts, key: &(String, String)) -> usize {
    budget.get(key).copied().unwrap_or(0)
}

pub fn over_budget(found: &Counts, budget: &Counts) -> Vec<String> {
    found
        .iter()
        .filter(|&(key, count)| *count > budget_of(budget, key))
        .map(|(key, count)| {
            format!(
                "{}: {count} comment lines, budget {}",
                key.0,
                budget_of(budget, key)
            )
        })
        .collect()
}

pub fn under_budget(found: &Counts, budget: &Counts) -> Vec<String> {
    budget
        .iter()
        .filter_map(|(key, limit)| {
            let count = budget_of(found, key);
            (count < *limit).then(|| format!("{}: {count} comment lines, budget {limit}", key.0))
        })
        .collect()
}

pub fn lowered(found: &Counts, budget: &Counts) -> Counts {
    let mut counts = Counts::new();
    for (key, limit) in budget {
        add(
            &mut counts,
            &key.0,
            &key.1,
            budget_of(found, key).min(*limit),
        );
    }
    counts
}
