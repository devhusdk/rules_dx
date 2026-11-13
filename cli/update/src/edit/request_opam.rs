use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Str,
    Op,
    Open,
    Close,
    Other(char),
}

struct Token {
    kind: Kind,
    start: usize,
    end: usize,
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let mut end = start + c.len_utf8();
        let kind = match c {
            _ if c.is_whitespace() => continue,
            '#' => {
                for (_, rest) in chars.by_ref() {
                    if rest == '\n' {
                        break;
                    }
                }
                continue;
            }
            '"' => {
                while let Some((at, rest)) = chars.next() {
                    end = at + rest.len_utf8();
                    match rest {
                        '\\' => {
                            chars.next();
                        }
                        '"' => break,
                        _ => {}
                    }
                }
                Kind::Str
            }
            '=' | '<' | '>' | '!' => {
                while let Some(&(at, rest)) = chars.peek() {
                    if !matches!(rest, '=' | '<' | '>' | '!') {
                        break;
                    }
                    end = at + rest.len_utf8();
                    chars.next();
                }
                Kind::Op
            }
            '{' => Kind::Open,
            '}' => Kind::Close,
            other => Kind::Other(other),
        };
        out.push(Token { kind, start, end });
    }
    out
}

/// Rewrites the version of the one constraint on `package` in the `depends:` list of an opam file.
pub(super) fn plan_opam(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Opam(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected an opam version".to_owned(),
        });
    };
    let unsupported = |reason: &str| EditError::UnsupportedManifest {
        manifest: manifest.to_owned(),
        reason: reason.to_owned(),
    };
    let not_found = || EditError::NotFound {
        manifest: manifest.to_owned(),
        package: package.to_owned(),
    };
    let start = content.find("depends:").ok_or_else(not_found)?;
    let base = start + "depends:".len();
    let all = tokens(content.get(base..).ok_or_else(not_found)?);
    if all.first().map(|t| t.kind) != Some(Kind::Other('[')) {
        return Err(unsupported("depends is not a list"));
    }
    let mut depth = 0;
    let mut list_end = all.len();
    for (i, token) in all.iter().enumerate() {
        match token.kind {
            Kind::Open => depth += 1,
            Kind::Close => depth -= 1,
            Kind::Other(']') if depth == 0 => {
                list_end = i;
                break;
            }
            _ => {}
        }
    }
    let list = all
        .get(1..list_end)
        .ok_or_else(|| unsupported("depends is not a list"))?;
    let text_of = |t: &Token| content.get(base + t.start..base + t.end);
    let mut hits = Vec::new();
    let mut depth = 0;
    for (i, token) in list.iter().enumerate() {
        match token.kind {
            Kind::Open => depth += 1,
            Kind::Close => depth -= 1,
            Kind::Str
                if depth == 0
                    && text_of(token).is_some_and(|text| text.trim_matches('"') == package) =>
            {
                hits.push(i)
            }
            _ => {}
        }
    }
    let index = match hits.as_slice() {
        [] => return Err(not_found()),
        [index] => *index,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let block: Vec<&Token> = if list.get(index + 1).map(|t| t.kind) == Some(Kind::Open) {
        let mut depth = 0;
        let mut block = Vec::new();
        for token in list.iter().skip(index + 1) {
            match token.kind {
                Kind::Open => depth += 1,
                Kind::Close => depth -= 1,
                _ => {}
            }
            block.push(token);
            if depth == 0 {
                break;
            }
        }
        block
    } else {
        Vec::new()
    };
    let constraints: Vec<&Token> = block
        .windows(3)
        .filter(|w| {
            matches!(
                w,
                [prev, op, val]
                    if op.kind == Kind::Op
                        && matches!(prev.kind, Kind::Open | Kind::Other('&' | '|' | '('))
                        && val.kind == Kind::Str
            )
        })
        .filter_map(|w| w.get(2).copied())
        .collect();
    let [constraint] = constraints.as_slice() else {
        return Err(unsupported(
            "the dependency needs exactly one literal version constraint",
        ));
    };
    let mut edited = content.to_owned();
    edited.replace_range(base + constraint.start + 1..base + constraint.end - 1, new);
    Ok(edited)
}
