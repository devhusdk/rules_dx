use crate::vuln::LockedPackage;
use dx_error::DxError;

#[derive(Debug, Eq, PartialEq)]
enum Token {
    Text(String),
    Word(String),
    Open(char),
    Close(char),
}

/// Reads the `depends:` list of an opam.locked file, keeping each package's `= "version"` pin.
/// The OCaml compiler packages and `base-*` packages are skipped.
pub fn parse_opam_locked(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let tokens = tokenize(text)?;
    let mut index = depends_start(&tokens)
        .ok_or_else(|| DxError::failure("invalid opam.locked: missing depends"))?;
    let mut out = Vec::new();
    while let Some(token) = tokens.get(index) {
        let name = match token {
            Token::Close(']') => break,
            Token::Text(name) => name,
            _ => {
                index += 1;
                continue;
            }
        };
        index += 1;
        let mut version = None;
        if tokens.get(index) == Some(&Token::Open('{')) {
            let mut depth = 0;
            let mut after_equals = false;
            while let Some(token) = tokens.get(index) {
                match token {
                    Token::Open('{') => depth += 1,
                    Token::Close('}') => depth -= 1,
                    Token::Text(found) if after_equals && version.is_none() => {
                        version = Some(found.clone());
                    }
                    _ => {}
                }
                after_equals = *token == Token::Word("=".to_owned());
                index += 1;
                if depth == 0 {
                    break;
                }
            }
        }
        if is_toolchain(name) {
            continue;
        }
        let version = version.ok_or_else(|| {
            DxError::failure(format!("invalid opam.locked: {name} has no = version"))
        })?;
        out.push(LockedPackage::new("opam", name, &version));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|b, a| a.name == b.name && a.version == b.version);
    Ok(out)
}

fn is_toolchain(name: &str) -> bool {
    name == "ocaml"
        || name.starts_with("base-")
        || [
            "ocaml-base-compiler",
            "ocaml-variants",
            "ocaml-system",
            "ocaml-config",
            "ocaml-options-",
        ]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// The index of the first token after the top-level `depends: [`.
fn depends_start(tokens: &[Token]) -> Option<usize> {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Open(_) => depth += 1,
            Token::Close(_) => depth -= 1,
            Token::Word(word) if depth == 0 && word == "depends:" => {
                return (tokens.get(index + 1) == Some(&Token::Open('['))).then_some(index + 2);
            }
            _ => {}
        }
    }
    None
}

fn tokenize(text: &str) -> Result<Vec<Token>, DxError> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(char) = chars.next() {
        match char {
            '#' => while chars.next_if(|next| *next != '\n').is_some() {},
            '(' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                loop {
                    let next = chars.next().ok_or_else(|| {
                        DxError::failure("invalid opam.locked: unterminated comment")
                    })?;
                    if previous == '*' && next == ')' {
                        break;
                    }
                    previous = next;
                }
            }
            '"' => {
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => value.extend(chars.next()),
                        Some(next) => value.push(next),
                        None => {
                            return Err(DxError::failure(
                                "invalid opam.locked: unterminated string",
                            ));
                        }
                    }
                }
                out.push(Token::Text(value));
            }
            '[' | '{' => out.push(Token::Open(char)),
            ']' | '}' => out.push(Token::Close(char)),
            char if char.is_whitespace() => {}
            first => {
                let mut word = String::from(first);
                while let Some(next) = chars.next_if(|next| {
                    !next.is_whitespace() && !matches!(next, '"' | '[' | ']' | '{' | '}' | '#')
                }) {
                    word.push(next);
                }
                out.push(Token::Word(word));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(text: &str) -> Vec<(String, String)> {
        parse_opam_locked(text)
            .expect("parsed")
            .into_iter()
            .map(|package| (package.name, package.version))
            .collect()
    }

    #[test]
    fn plain_pins_are_read() {
        let text = "opam-version: \"2.0\"\ndepends: [\n  \"astring\" {= \"0.8.5\"}\n  \"fmt\" {= \"0.9.0\"}\n]\n";
        assert_eq!(
            pins(text),
            [
                ("astring".to_owned(), "0.8.5".to_owned()),
                ("fmt".to_owned(), "0.9.0".to_owned())
            ]
        );
    }

    #[test]
    fn filtered_pins_comments_and_compiler_packages() {
        let text = "depends: [\n  # a line comment \"x\" {= \"9\"}\n  (* block\n  \"y\" {= \"8\"} *)\n  \"ocaml\" {= \"5.2.0\"}\n  \"base-threads\" {= \"base\"}\n  \"ocaml-base-compiler\" {= \"5.2.0\"}\n  \"zarith\" {= \"1.13\" & os != \"win32\"}\n  \"alcotest\" {with-test & = \"1.7.0\"}\n  \"lwt\" {>= \"5.0\" & = \"5.7.0\"}\n]\n";
        assert_eq!(
            pins(text),
            [
                ("alcotest".to_owned(), "1.7.0".to_owned()),
                ("lwt".to_owned(), "5.7.0".to_owned()),
                ("zarith".to_owned(), "1.13".to_owned())
            ]
        );
    }

    #[test]
    fn a_pinned_package_with_a_known_advisory_is_reported() {
        let locked =
            parse_opam_locked("depends: [ \"opam-client\" {= \"2.1.4\"} ]\n").expect("parsed");
        let snapshot = r#"[{"id":"OSV-TEST-opam","modified":"2026-09-18T00:00:00Z","affected":[{"package":{"ecosystem":"opam","name":"opam-client"},"ranges":[{"type":"ECOSYSTEM","events":[{"introduced":"0"},{"fixed":"2.1.5"}]}]}]}]"#;
        let advisories = crate::vuln::parse_snapshot(snapshot).expect("snapshot");
        let (findings, _) = crate::vuln::match_packages(&locked, &advisories);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn missing_depends_is_an_error() {
        let error = parse_opam_locked("opam-version: \"2.0\"\n").expect_err("error");
        assert!(error.message().contains("missing depends"), "{error}");
    }

    #[test]
    fn an_unpinned_package_is_an_error() {
        let error = parse_opam_locked("depends: [ \"fmt\" ]\n").expect_err("error");
        assert!(error.message().contains("fmt has no = version"), "{error}");
    }
}
