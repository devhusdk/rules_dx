//! `dx_topiary`: a hermetic OCaml formatter built on topiary-core.
//!
//! Grammars and formatting queries are compiled in, so the tool never reads
//! environment variables or configuration files and never touches the network.
//!
//! Usage: `dx_topiary [--write] [--language ocaml|ocaml_interface] FILE...`
//!
//! Exit codes: 0 = every file is formatted, 1 = check mode found files that
//! would change (one PATH per line on stdout), 2 = a syntax
//! error (`PATH:LINE:COL: message`), an unsupported file, or a tool error.

use std::path::Path;
use std::process::ExitCode;

use topiary_core::{formatter_str, Language, Operation, TopiaryQuery};

/// The single language-to-grammar table. Add new topiary languages here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lang {
    Ocaml,
    OcamlInterface,
}

impl Lang {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "ocaml" => Some(Self::Ocaml),
            "ocaml_interface" => Some(Self::OcamlInterface),
            _ => None,
        }
    }

    fn from_path(path: &str) -> Option<Self> {
        match Path::new(path).extension()?.to_str()? {
            "ml" => Some(Self::Ocaml),
            "mli" => Some(Self::OcamlInterface),
            _ => None,
        }
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Ocaml => tree_sitter_ocaml::LANGUAGE_OCAML.into(),
            Self::OcamlInterface => tree_sitter_ocaml::LANGUAGE_OCAML_INTERFACE.into(),
        }
    }

    fn query(self) -> &'static str {
        match self {
            Self::Ocaml => topiary_queries::ocaml(),
            Self::OcamlInterface => topiary_queries::ocaml_interface(),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ocaml => "ocaml",
            Self::OcamlInterface => "ocaml_interface",
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Clean,
    Changed(String),
    Syntax { line: usize, column: usize },
    Error(String),
}

/// Position (1-based) of the first syntax error or missing node, if any.
fn first_syntax_error(lang: Lang, source: &str) -> Result<Option<(usize, usize)>, String> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lang.grammar())
        .map_err(|err| format!("cannot load {} grammar: {err}", lang.name()))?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| "parser produced no tree".to_owned())?;
    let root = tree.root_node();
    if !root.has_error() {
        return Ok(None);
    }
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.is_error() || node.is_missing() {
            let point = node.start_position();
            return Ok(Some((point.row + 1, point.column + 1)));
        }
        if node.has_error() {
            let mut cursor = node.walk();
            let children: Vec<_> = node.children(&mut cursor).collect();
            stack.extend(children.into_iter().rev());
        }
    }
    Ok(Some((1, 1)))
}

fn format_source(lang: Lang, source: &str) -> Outcome {
    match first_syntax_error(lang, source) {
        Ok(Some((line, column))) => return Outcome::Syntax { line, column },
        Ok(None) => {}
        Err(message) => return Outcome::Error(message),
    }
    let grammar = lang.grammar().into();
    let formatting_query = match TopiaryQuery::new(&grammar, lang.query()) {
        Ok(query) => query,
        Err(err) => return Outcome::Error(format!("invalid formatting query: {err}")),
    };
    let language = Language {
        name: lang.name().to_owned(),
        formatting_query,
        injection_query: None,
        grammar,
        indent: None,
    };
    let operation = Operation::Format {
        skip_idempotence: false,
        tolerate_parsing_errors: false,
        skip_stage: None,
    };
    let mut output = Vec::new();
    if let Err(err) = formatter_str(source, &mut output, &language, operation, None) {
        let message = err
            .to_string()
            .lines()
            .next()
            .unwrap_or("format failed")
            .to_owned();
        return Outcome::Error(message);
    }
    match String::from_utf8(output) {
        Ok(formatted) if formatted == source => Outcome::Clean,
        Ok(formatted) => Outcome::Changed(formatted),
        Err(_) => Outcome::Error("formatter produced invalid UTF-8".to_owned()),
    }
}

struct Args {
    write: bool,
    language: Option<Lang>,
    files: Vec<String>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut parsed = Args {
        write: false,
        language: None,
        files: Vec::new(),
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--write" => parsed.write = true,
            "--language" => {
                let name = args.next().ok_or("--language needs a value")?;
                parsed.language =
                    Some(Lang::from_name(&name).ok_or(format!("unknown language `{name}`"))?);
            }
            flag if flag.starts_with("--") => return Err(format!("unknown flag `{flag}`")),
            _ => parsed.files.push(arg),
        }
    }
    if parsed.files.is_empty() {
        return Err("no input files".to_owned());
    }
    Ok(parsed)
}

fn run(args: Args) -> u8 {
    let mut changed = false;
    let mut failed = false;
    for path in &args.files {
        let Some(lang) = args.language.or_else(|| Lang::from_path(path)) else {
            eprintln!("{path}: error: unsupported file extension (expected .ml or .mli)");
            failed = true;
            continue;
        };
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("{path}: error: {err}");
                failed = true;
                continue;
            }
        };
        match format_source(lang, &source) {
            Outcome::Clean => {}
            Outcome::Changed(formatted) => {
                if args.write {
                    if let Err(err) = std::fs::write(path, formatted) {
                        eprintln!("{path}: error: {err}");
                        failed = true;
                    }
                } else {
                    println!("{path}");
                    changed = true;
                }
            }
            Outcome::Syntax { line, column } => {
                eprintln!("{path}:{line}:{column}: syntax error");
                failed = true;
            }
            Outcome::Error(message) => {
                eprintln!("{path}: error: {message}");
                failed = true;
            }
        }
    }
    if failed {
        2
    } else {
        u8::from(changed)
    }
}

fn main() -> ExitCode {
    match parse_args(std::env::args().skip(1)) {
        Ok(args) => ExitCode::from(run(args)),
        Err(message) => {
            eprintln!("dx_topiary: {message}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_input_is_unchanged() {
        assert_eq!(format_source(Lang::Ocaml, "let x = 1\n"), Outcome::Clean);
    }

    #[test]
    fn dirty_input_is_reformatted() {
        match format_source(Lang::Ocaml, "let   x=1\n") {
            Outcome::Changed(out) => assert_eq!(out, "let x = 1\n"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn interface_extension_selects_interface_grammar() {
        assert_eq!(Lang::from_path("a/b.mli"), Some(Lang::OcamlInterface));
        assert_eq!(Lang::from_path("a/b.ml"), Some(Lang::Ocaml));
        assert_eq!(
            format_source(Lang::OcamlInterface, "val x : int\n"),
            Outcome::Clean
        );
    }

    #[test]
    fn syntax_error_reports_position() {
        match format_source(Lang::Ocaml, "let x = 1\nlet = = 2\n") {
            Outcome::Syntax { line, .. } => assert_eq!(line, 2),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn unknown_extension_is_rejected() {
        assert_eq!(Lang::from_path("a.mll"), None);
        assert_eq!(Lang::from_path("a.mly"), None);
        assert_eq!(Lang::from_path("a"), None);
        assert!(parse_args(
            ["--language".to_owned(), "cobol".to_owned(), "a".to_owned()].into_iter()
        )
        .is_err());
    }
}
