//! Reports whether MODULE.bazel and the files it includes declare rules_dx at one expected version.

use std::collections::HashSet;
use std::path::Path;

const INCLUDE_SUFFIX: &str = ".MODULE.bazel";

struct Call {
    line: usize,
    name: &'static str,
    arguments: String,
}

struct Found {
    file: String,
    line: usize,
    name: &'static str,
    arguments: String,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn is_blank(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

fn skip_blanks(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && is_blank(bytes[at]) {
        at += 1;
    }
    at
}

pub fn strip_comments(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut kept = Vec::with_capacity(bytes.len());
    let mut quote = 0u8;
    let mut index = 0;
    while index < bytes.len() {
        let c = bytes[index];
        if quote != 0 {
            if c == b'\\' && index + 1 < bytes.len() {
                kept.extend_from_slice(&bytes[index..index + 2]);
                index += 2;
                continue;
            }
            kept.push(c);
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
            kept.push(c);
        } else if c == b'#' {
            index = bytes[index..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |p| index + p);
            continue;
        } else {
            kept.push(c);
        }
        index += 1;
    }
    String::from_utf8_lossy(&kept).into_owned()
}

fn call_arguments(text: &str, opening: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut quote = 0u8;
    let mut index = opening;
    while index < bytes.len() {
        let c = bytes[index];
        if quote != 0 {
            if c == b'\\' && index + 1 < bytes.len() {
                index += 2;
                continue;
            }
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(&text[opening + 1..index]);
            }
        }
        index += 1;
    }
    None
}

fn call_at_line_start(text: &str, start: usize) -> Option<(&'static str, usize)> {
    let bytes = text.as_bytes();
    for name in ["bazel_dep", "module", "include"] {
        if bytes[start..].starts_with(name.as_bytes()) {
            let at = skip_blanks(bytes, start + name.len());
            if bytes.get(at) == Some(&b'(') {
                return Some((name, at));
            }
        }
    }
    None
}

fn declarations(text: &str) -> Vec<Call> {
    let mut found = Vec::new();
    let mut start = 0;
    let mut line = 1;
    while start <= text.len() {
        if let Some((name, opening)) = call_at_line_start(text, start) {
            if let Some(arguments) = call_arguments(text, opening) {
                found.push(Call {
                    line,
                    name,
                    arguments: arguments.to_string(),
                });
            }
        }
        match text[start..].find('\n') {
            Some(p) => {
                start += p + 1;
                line += 1;
            }
            None => break,
        }
    }
    found
}

fn word_starts<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    text.match_indices(word)
        .map(|(i, _)| i)
        .filter(|&i| !text[..i].chars().next_back().is_some_and(is_word))
}

fn names_rules_dx(arguments: &str) -> bool {
    let bytes = arguments.as_bytes();
    word_starts(arguments, "name").any(|i| {
        let at = skip_blanks(bytes, i + "name".len());
        bytes.get(at) == Some(&b'=')
            && bytes[skip_blanks(bytes, at + 1)..].starts_with(b"\"rules_dx\"")
    })
}

fn version_of(arguments: &str) -> Option<&str> {
    let bytes = arguments.as_bytes();
    for i in word_starts(arguments, "version") {
        let at = skip_blanks(bytes, i + "version".len());
        if bytes.get(at) != Some(&b'=') {
            continue;
        }
        let value = skip_blanks(bytes, at + 1);
        let rest = &arguments[value..];
        if let Some(inner) = rest.strip_prefix('"') {
            if let Some(close) = inner.find('"') {
                return Some(&rest[..close + 2]);
            }
        }
        let end = rest
            .find(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')'))
            .unwrap_or(rest.len());
        if end > 0 {
            return Some(&rest[..end]);
        }
    }
    None
}

fn literal_label(arguments: &str) -> Option<&str> {
    let mut rest = arguments.trim_start();
    if let Some(after) = rest.strip_prefix("label") {
        if let Some(after) = after.trim_start().strip_prefix('=') {
            rest = after.trim_start();
        }
    }
    let quote = rest.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let close = rest[1..].find(quote)? + 1;
    let tail = rest[close + 1..].trim_start();
    let tail = tail.strip_prefix(',').unwrap_or(tail);
    tail.trim().is_empty().then(|| &rest[1..close])
}

fn include_path(label: &str) -> Result<String, String> {
    let Some((package, name)) = label
        .strip_prefix("//")
        .and_then(|rest| rest.split_once(':'))
    else {
        return Err("is not a //package:file label".to_string());
    };
    let mut segments: Vec<&str> = if package.is_empty() {
        Vec::new()
    } else {
        package.split('/').collect()
    };
    segments.extend(name.split('/'));
    for segment in &segments {
        if matches!(*segment, "" | "." | "..") || segment.contains('\\') || segment.contains(':') {
            return Err("leaves the workspace".to_string());
        }
    }
    let hidden = segments.last().is_some_and(|s| s.starts_with('.'));
    if !name.ends_with(INCLUDE_SUFFIX) || hidden {
        return Err(format!("must name a {INCLUDE_SUFFIX} file"));
    }
    Ok(segments.join("/"))
}

fn walk(
    root: &Path,
    rel: &str,
    seen: &mut HashSet<String>,
    found: &mut Vec<Found>,
) -> Result<(), Result<String, std::io::Error>> {
    let text = std::fs::read_to_string(root.join(rel)).map_err(Err)?;
    let text = strip_comments(&text);
    for call in declarations(&text) {
        if call.name != "include" {
            if names_rules_dx(&call.arguments) {
                found.push(Found {
                    file: rel.to_string(),
                    line: call.line,
                    name: call.name,
                    arguments: call.arguments,
                });
            }
            continue;
        }
        let line = call.line;
        let Some(label) = literal_label(&call.arguments) else {
            return Err(Ok(format!(
                "{rel}:{line} include() needs a literal \"//package:name.MODULE.bazel\" label"
            )));
        };
        let target =
            include_path(label).map_err(|why| Ok(format!("{rel}:{line} include {label} {why}")))?;
        if !seen.insert(label.to_string()) {
            return Err(Ok(format!("{rel}:{line} includes {label} more than once")));
        }
        match walk(root, &target, seen, found) {
            Err(Err(error)) => {
                return Err(Ok(format!(
                    "{rel}:{line} includes {label}, but {target} cannot be read: {error}"
                )))
            }
            other => other?,
        }
    }
    Ok(())
}

/// Returns the success line, or the diagnostic without its `dx-ci: ` prefix.
pub fn verify(path: &str, pin: &str) -> Result<String, String> {
    if pin.is_empty() || pin.contains(['"', '\\']) || pin.chars().any(char::is_whitespace) {
        return Err(format!(
            "RULES_DX_PIN='{pin}' is not a plain rules_dx version string"
        ));
    }
    let full = Path::new(path);
    let root = match full.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let rel = full
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut found = Vec::new();
    match walk(root, &rel, &mut HashSet::new(), &mut found) {
        Err(Err(error)) => return Err(format!("{path} cannot be read: {error}")),
        Err(Ok(message)) => return Err(message),
        Ok(()) => {}
    }
    if found.is_empty() {
        return Err(format!(
            "{path} declares no bazel_dep(name = \"rules_dx\", version = \"{pin}\")"
        ));
    }
    if found.len() > 1 {
        let places: Vec<String> = found
            .iter()
            .map(|f| format!("{}:{} ({})", f.file, f.line, f.name))
            .collect();
        return Err(format!(
            "{path} declares rules_dx {} times, at {}. Keep one declaration.",
            found.len(),
            places.join(" and ")
        ));
    }
    let Found {
        file,
        line,
        name,
        arguments,
    } = &found[0];
    let Some(version) = version_of(arguments) else {
        return Err(format!(
            "{file}:{line} {name}(name = \"rules_dx\") declares no version. Add version = \"{pin}\"."
        ));
    };
    if !version.starts_with('"') {
        return Err(format!(
            "{file}:{line} {name} version must be a quoted string, found {version}. Write version = \"{pin}\"."
        ));
    }
    if version.get(1..version.len() - 1).unwrap_or("") != pin {
        return Err(format!(
            "{file}:{line} declares rules_dx {version}, the workflow expects \"{pin}\". Pin both to the same version."
        ));
    }
    Ok(format!(
        "{file}:{line} {name}(name = \"rules_dx\", version = \"{pin}\") matches the workflow pin"
    ))
}
