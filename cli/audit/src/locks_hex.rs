use crate::vuln::LockedPackage;
use dx_error::DxError;

#[derive(Debug, Eq, PartialEq)]
enum Token {
    Str(String),
    Atom(String),
    Open,
    Close,
    Other,
}

/// Splits Elixir or Erlang terms into strings, atoms and brackets; everything else is `Other`.
fn tokenize(text: &str, name: &str) -> Result<Vec<Token>, DxError> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '#' => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '"' => {
                let (value, next) = read_string(&chars, i, name)?;
                tokens.push(Token::Str(value));
                i = next;
            }
            ':' if chars.get(i + 1) == Some(&'"') => {
                let (value, next) = read_string(&chars, i + 1, name)?;
                tokens.push(Token::Atom(value));
                i = next;
            }
            ':' => {
                let start = i + 1;
                i = start;
                while i < chars.len() && (chars[i].is_alphanumeric() || "_@?!".contains(chars[i])) {
                    i += 1;
                }
                if i > start {
                    tokens.push(Token::Atom(chars[start..i].iter().collect()));
                }
            }
            '{' | '[' => {
                depth += 1;
                tokens.push(Token::Open);
                i += 1;
            }
            '}' | ']' => {
                depth = depth.checked_sub(1).ok_or_else(|| {
                    DxError::failure(format!("invalid {name}: unbalanced brackets"))
                })?;
                tokens.push(Token::Close);
                i += 1;
            }
            c => {
                if !c.is_whitespace() && c != ',' {
                    tokens.push(Token::Other);
                }
                i += 1;
            }
        }
    }
    if depth != 0 {
        return Err(DxError::failure(format!(
            "invalid {name}: unbalanced brackets"
        )));
    }
    Ok(tokens)
}

fn read_string(chars: &[char], quote: usize, name: &str) -> Result<(String, usize), DxError> {
    let mut value = String::new();
    let mut i = quote + 1;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                i += 1;
                if let Some(c) = chars.get(i) {
                    value.push(*c);
                }
            }
            '"' => return Ok((value, i + 1)),
            c => value.push(c),
        }
        i += 1;
    }
    Err(DxError::failure(format!(
        "invalid {name}: unterminated string"
    )))
}

/// Reads the `{:hex, :name, "version", ...}` entries of a mix.lock; git and path entries are skipped.
pub fn parse_mix_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let tokens = tokenize(text, "mix.lock")?;
    if !text.trim_start().starts_with("%{") {
        return Err(DxError::failure("invalid mix.lock: expected a map"));
    }
    let mut out = Vec::new();
    for window in tokens.windows(4) {
        if let [
            Token::Open,
            Token::Atom(kind),
            Token::Atom(package),
            Token::Str(version),
        ] = window
            && kind == "hex"
        {
            out.push(LockedPackage::new(
                "hex",
                package.as_str(),
                version.as_str(),
            ));
        }
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}

/// Reads the `{pkg, <<"name">>, <<"version">>}` entries of a rebar.lock.
pub fn parse_rebar_lock(text: &str) -> Result<Vec<LockedPackage>, DxError> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("{pkg,") {
        rest = &rest[at + "{pkg,".len()..];
        let mut names = Vec::new();
        let mut cursor = rest;
        while names.len() < 2 {
            let open = cursor.find("<<\"").ok_or_else(|| {
                DxError::failure("invalid rebar.lock: pkg entry without name and version")
            })?;
            let body = &cursor[open + 3..];
            let close = body
                .find("\">>")
                .ok_or_else(|| DxError::failure("invalid rebar.lock: unterminated binary"))?;
            names.push(body[..close].to_owned());
            cursor = &body[close + 3..];
        }
        out.push(LockedPackage::new(
            "hex",
            names[0].as_str(),
            names[1].as_str(),
        ));
    }
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(out)
}
