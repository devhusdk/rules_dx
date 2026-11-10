use crate::vuln::LockedPackage;
use dx_error::DxError;

pub const LUAROCKS_SET: &str = "luarocks";

#[derive(Debug, PartialEq)]
enum Token {
    Name(String),
    Text(String),
    Punct(char),
}

fn tokenize(text: &str) -> Result<Vec<Token>, DxError> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        if c.is_whitespace() {
            at += 1;
        } else if c == '-' && chars.get(at + 1) == Some(&'-') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
        } else if c == '"' || c == '\'' {
            let mut value = String::new();
            at += 1;
            loop {
                match chars.get(at) {
                    None | Some('\n') => return Err(DxError::failure("unterminated string")),
                    Some('\\') => {
                        let next = chars
                            .get(at + 1)
                            .ok_or_else(|| DxError::failure("unterminated string"))?;
                        value.push(match next {
                            'n' => '\n',
                            't' => '\t',
                            other => *other,
                        });
                        at += 2;
                    }
                    Some(&end) if end == c => {
                        at += 1;
                        break;
                    }
                    Some(&other) => {
                        value.push(other);
                        at += 1;
                    }
                }
            }
            tokens.push(Token::Text(value));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
                at += 1;
            }
            tokens.push(Token::Name(chars[start..at].iter().collect()));
        } else if "{}[]=,;".contains(c) {
            tokens.push(Token::Punct(c));
            at += 1;
        } else {
            return Err(DxError::failure(format!("unexpected character {c:?}")));
        }
    }
    Ok(tokens)
}

/// Reads the `dependencies` table of a luarocks.lock (`return { dependencies = { name = "1.0-1" } }`).
pub fn parse_luarocks_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let invalid = |detail: &str| DxError::failure(format!("invalid luarocks.lock: {detail}"));
    let tokens = tokenize(text).map_err(|detail| invalid(&detail.to_string()))?;
    let mut tokens = tokens.iter().peekable();
    if tokens.next() != Some(&Token::Name("return".to_owned()))
        || tokens.next() != Some(&Token::Punct('{'))
    {
        return Err(invalid("expected `return {`"));
    }
    let mut out = Vec::new();
    let mut found = false;
    let mut depth = 1usize;
    let mut table: Option<usize> = None;
    while let Some(token) = tokens.next() {
        match token {
            Token::Punct('{') => depth += 1,
            Token::Punct('}') => {
                depth -= 1;
                if table == Some(depth) {
                    table = None;
                }
                if depth == 0 {
                    if tokens.peek().is_some() {
                        return Err(invalid("trailing content after the table"));
                    }
                    break;
                }
            }
            Token::Name(key)
                if depth == 1
                    && key == "dependencies"
                    && tokens.peek() == Some(&&Token::Punct('=')) =>
            {
                tokens.next();
                if tokens.next() != Some(&Token::Punct('{')) {
                    return Err(invalid("dependencies is not a table"));
                }
                found = true;
                table = Some(depth);
                depth += 1;
            }
            Token::Punct('[') if table == Some(depth - 1) => {
                let Some(Token::Text(name)) = tokens.next() else {
                    return Err(invalid("expected a quoted key"));
                };
                if tokens.next() != Some(&Token::Punct(']')) {
                    return Err(invalid("expected `]`"));
                }
                out.push(entry(name, &mut tokens)?);
            }
            Token::Name(name) if table == Some(depth - 1) => {
                out.push(entry(name, &mut tokens)?);
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(invalid("unterminated table"));
    }
    if !found {
        return Err(invalid("missing dependencies"));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}

fn entry<'a>(
    name: &str,
    tokens: &mut impl Iterator<Item = &'a Token>,
) -> Result<LockedPackage, DxError> {
    let invalid = |detail: String| DxError::failure(format!("invalid luarocks.lock: {detail}"));
    if tokens.next() != Some(&Token::Punct('=')) {
        return Err(invalid(format!("{name} has no version")));
    }
    match tokens.next() {
        Some(Token::Text(version)) if !version.trim().is_empty() && !name.is_empty() => {
            Ok(LockedPackage::new(LUAROCKS_SET, name, version.trim()))
        }
        _ => Err(invalid(format!("{name} has no version"))),
    }
}
