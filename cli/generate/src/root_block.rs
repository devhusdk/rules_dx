//! The marked `exports_files` block in the root BUILD file that lets `//.dx` read root configs.

use std::collections::BTreeSet;

use dx_merge::block;

use crate::{GenerateError, RootBuild};

pub const BEGIN: &str = "# dx:begin exports";
pub const END: &str = "# dx:end exports";

/// Visibilities that already let `//.dx` read an exported file.
const SEEN_BY_DX: &[&str] = &[
    "//visibility:public",
    "//:__subpackages__",
    "//.dx:__pkg__",
    "//.dx:__subpackages__",
];

/// The root BUILD path and its text, or `None` when it needs no block and has none.
pub fn render(
    build: Option<&RootBuild>,
    wanted: &BTreeSet<&str>,
) -> Result<Option<(String, String)>, GenerateError> {
    let Some(build) = build else {
        if wanted.is_empty() {
            return Ok(None);
        }
        return Ok(Some(("BUILD.bazel".to_owned(), block_text(wanted))));
    };
    let span = block::span(&build.text, BEGIN, END);
    let outside = match span {
        Some((start, stop)) => format!("{}{}", &build.text[..start], &build.text[stop..]),
        None => build.text.clone(),
    };
    let mut missing: BTreeSet<&str> = wanted.clone();
    for export in exports(&build.name, &outside)? {
        let seen = export.visibility.as_ref().is_none_or(|visibility| {
            visibility
                .iter()
                .any(|label| SEEN_BY_DX.contains(&label.as_str()))
        });
        for file in &export.files {
            if !missing.remove(file.as_str()) || seen {
                continue;
            }
            return Err(GenerateError::NarrowExport {
                build: build.name.clone(),
                file: file.clone(),
                visibility: export.visibility.clone().unwrap_or_default().join(", "),
            });
        }
    }
    let text = if missing.is_empty() {
        match span {
            Some(_) => outside,
            None => return Ok(None),
        }
    } else {
        block::merge(&build.text, BEGIN, END, &block_text(&missing)).map_err(|source| {
            GenerateError::Merge {
                build: build.name.clone(),
                source,
            }
        })?
    };
    Ok(Some((build.name.clone(), text)))
}

fn block_text(files: &BTreeSet<&str>) -> String {
    let files: Vec<String> = files.iter().map(|file| (*file).to_owned()).collect();
    format!(
        "{BEGIN}\nexports_files(\n    {},\n    visibility = [\"//.dx:__pkg__\"],\n)\n{END}\n",
        crate::list(&files, 4)
    )
}

/// One literal `exports_files` call.
struct Export {
    files: Vec<String>,
    /// `None` when the call sets no visibility, which exports publicly.
    visibility: Option<Vec<String>>,
}

#[derive(Debug, PartialEq)]
enum Token {
    Str(String),
    Word(String),
    Punct(char),
}

/// The tokens of a BUILD file, without comments.
fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '"' || c == '\'' {
            let triple = chars[i..].starts_with(&[c, c, c]);
            let quote = if triple { 3 } else { 1 };
            i += quote;
            let mut value = String::new();
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    value.push(chars[i + 1]);
                    i += 2;
                } else if chars[i..].starts_with(&vec![c; quote]) {
                    i += quote;
                    break;
                } else {
                    value.push(chars[i]);
                    i += 1;
                }
            }
            out.push(Token::Str(value));
        } else if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            out.push(Token::Word(chars[start..i].iter().collect()));
        } else {
            if !c.is_whitespace() {
                out.push(Token::Punct(c));
            }
            i += 1;
        }
    }
    out
}

fn exports(build: &str, text: &str) -> Result<Vec<Export>, GenerateError> {
    let tokens = tokens(text);
    let mut out = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        if *token != Token::Word("exports_files".to_owned())
            || tokens.get(at + 1) != Some(&Token::Punct('('))
        {
            continue;
        }
        out.push(call(build, &tokens[at + 2..])?);
    }
    Ok(out)
}

/// Parses the arguments of one call, up to its closing parenthesis.
fn call(build: &str, tokens: &[Token]) -> Result<Export, GenerateError> {
    let non_literal = |at: usize| GenerateError::NonLiteralExport {
        build: build.to_owned(),
        argument: match tokens.get(at) {
            Some(Token::Word(word)) => word.clone(),
            Some(Token::Str(value)) => format!("\"{value}\""),
            Some(Token::Punct(c)) => c.to_string(),
            None => "end of file".to_owned(),
        },
    };
    let mut export = Export {
        files: Vec::new(),
        visibility: None,
    };
    let mut at = 0;
    loop {
        match tokens.get(at) {
            Some(Token::Punct(')')) => return Ok(export),
            Some(Token::Punct('[')) => {
                let (files, next) = strings(tokens, at).ok_or_else(|| non_literal(at))?;
                export.files = files;
                at = next;
            }
            Some(Token::Word(key)) if tokens.get(at + 1) == Some(&Token::Punct('=')) => {
                let (values, next) = strings(tokens, at + 2).ok_or_else(|| non_literal(at + 2))?;
                match key.as_str() {
                    "srcs" => export.files = values,
                    "visibility" => export.visibility = Some(values),
                    _ => {}
                }
                at = next;
            }
            _ => return Err(non_literal(at)),
        }
        match tokens.get(at) {
            Some(Token::Punct(',')) => at += 1,
            Some(Token::Punct(')')) => {}
            _ => return Err(non_literal(at)),
        }
    }
}

/// A literal list of strings starting at `at`, and the index after it.
fn strings(tokens: &[Token], at: usize) -> Option<(Vec<String>, usize)> {
    if tokens.get(at) != Some(&Token::Punct('[')) {
        return None;
    }
    let mut values = Vec::new();
    let mut at = at + 1;
    loop {
        match tokens.get(at)? {
            Token::Punct(']') => return Some((values, at + 1)),
            Token::Str(value) => values.push(value.clone()),
            _ => return None,
        }
        at += 1;
        match tokens.get(at)? {
            Token::Punct(',') => at += 1,
            Token::Punct(']') => {}
            _ => return None,
        }
    }
}
