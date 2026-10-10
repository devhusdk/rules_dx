use std::path::Path;

use super::AdoptError;

pub const DX_VERSION: &str = "0.0.0";
pub const MODULE_VERSION: &str = "0.0.0";
pub const PREVIOUS_VERSION: &str = "0.0.0";

pub fn version_pin_matches_module(dx_version: &str, module_version: &str) -> bool {
    if dx_version.is_empty() || module_version.is_empty() {
        return false;
    }
    let dx = match semver::Version::parse(dx_version) {
        Ok(dx) => dx,
        Err(_) => return false,
    };
    let module = match semver::Version::parse(module_version) {
        Ok(module) => module,
        Err(_) => return false,
    };
    dx == module
}

pub fn rollback_re_pins_previous(current: &str, target: &str, known_previous: &str) -> bool {
    !known_previous.is_empty() && target == known_previous && target != current
}

pub fn read_version_pin(root: &Path) -> Result<String, AdoptError> {
    let raw = std::fs::read_to_string(root.join(".dx/version")).map_err(|e| {
        AdoptError::ReadVersionPin {
            detail: e.to_string(),
        }
    })?;
    Ok(raw.trim().to_owned())
}

pub fn write_version_pin(root: &Path, version: &str) -> Result<(), AdoptError> {
    if version.is_empty() {
        return Err(AdoptError::EmptyVersion);
    }
    let dir = root.join(".dx");
    std::fs::create_dir_all(&dir).map_err(|e| AdoptError::CreateDxDir {
        detail: e.to_string(),
    })?;
    dx_atomic_fs::write_atomic(&dir.join("version"), format!("{version}\n").as_bytes()).map_err(
        |e| AdoptError::WriteVersionPin {
            detail: e.to_string(),
        },
    )?;
    Ok(())
}

pub const VERSION_HISTORY_REL: &str = ".dx/version-history.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PinRecord {
    pub previous: Option<String>,
    pub current: Option<String>,
}

fn push_record_string(out: &mut String, value: &str) {
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn push_record_maybe(out: &mut String, value: &Option<String>) {
    match value {
        Some(value) => push_record_string(out, value),
        None => out.push_str("null"),
    }
}

pub fn render_pin_record(record: &PinRecord) -> String {
    let mut out = String::from("{\"previous\":");
    push_record_maybe(&mut out, &record.previous);
    out.push_str(",\"current\":");
    push_record_maybe(&mut out, &record.current);
    out.push('}');
    out
}

struct RecordCursor<'a> {
    text: &'a [u8],
    at: usize,
}

impl<'a> RecordCursor<'a> {
    fn skip_ws(&mut self) {
        while self.at < self.text.len() && self.text[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn expect_byte(&mut self, want: u8, what: &str) -> Result<(), AdoptError> {
        if self.at < self.text.len() && self.text[self.at] == want {
            self.at += 1;
            return Ok(());
        }
        Err(AdoptError::PinRecordUnreadable {
            detail: format!("expected {what} at byte {}", self.at),
        })
    }

    fn expect_word(&mut self, word: &str) -> Result<(), AdoptError> {
        self.skip_ws();
        let end = self.at + word.len();
        if end <= self.text.len() && &self.text[self.at..end] == word.as_bytes() {
            self.at = end;
            return Ok(());
        }
        Err(AdoptError::PinRecordUnreadable {
            detail: format!("expected {word} at byte {}", self.at),
        })
    }

    fn parse_value(&mut self) -> Result<Option<String>, AdoptError> {
        self.skip_ws();
        if self.at < self.text.len() && self.text[self.at] == b'"' {
            return self.parse_string().map(Some);
        }
        self.expect_word("null")?;
        Ok(None)
    }

    fn parse_string(&mut self) -> Result<String, AdoptError> {
        self.expect_byte(b'"', "string")?;
        let mut out = String::new();
        loop {
            if self.at >= self.text.len() {
                return Err(AdoptError::PinRecordUnreadable {
                    detail: format!("unterminated string at byte {}", self.at),
                });
            }
            let byte = self.text[self.at];
            self.at += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    if self.at >= self.text.len() {
                        return Err(AdoptError::PinRecordUnreadable {
                            detail: format!("dangling escape at byte {}", self.at),
                        });
                    }
                    let escape = self.text[self.at];
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            if self.at + 4 > self.text.len() {
                                return Err(AdoptError::PinRecordUnreadable {
                                    detail: format!("short escape at byte {}", self.at),
                                });
                            }
                            let digits = std::str::from_utf8(&self.text[self.at..self.at + 4])
                                .map_err(|_| AdoptError::PinRecordUnreadable {
                                    detail: format!("bad escape at byte {}", self.at),
                                })?;
                            let scalar = u32::from_str_radix(digits, 16).map_err(|_| {
                                AdoptError::PinRecordUnreadable {
                                    detail: format!("bad escape at byte {}", self.at),
                                }
                            })?;
                            let scalar = char::from_u32(scalar).ok_or_else(|| {
                                AdoptError::PinRecordUnreadable {
                                    detail: format!("bad escape at byte {}", self.at),
                                }
                            })?;
                            self.at += 4;
                            out.push(scalar);
                        }
                        _ => {
                            return Err(AdoptError::PinRecordUnreadable {
                                detail: format!("bad escape at byte {}", self.at),
                            });
                        }
                    }
                }
                0x00..=0x1F => {
                    return Err(AdoptError::PinRecordUnreadable {
                        detail: format!("bare control at byte {}", self.at),
                    });
                }
                _ => {
                    let rest = std::str::from_utf8(&self.text[self.at - 1..]).map_err(|_| {
                        AdoptError::PinRecordUnreadable {
                            detail: format!("bad utf8 at byte {}", self.at),
                        }
                    })?;
                    let c = rest
                        .chars()
                        .next()
                        .ok_or_else(|| AdoptError::PinRecordUnreadable {
                            detail: format!("bad utf8 at byte {}", self.at),
                        })?;
                    self.at += c.len_utf8() - 1;
                    out.push(c);
                }
            }
        }
    }
}

pub fn parse_pin_record(text: &str) -> Result<PinRecord, AdoptError> {
    let mut cursor = RecordCursor {
        text: text.as_bytes(),
        at: 0,
    };
    cursor.skip_ws();
    cursor.expect_byte(b'{', "'{'")?;
    cursor.expect_word("\"previous\"")?;
    cursor.skip_ws();
    cursor.expect_byte(b':', "':'")?;
    let previous = cursor.parse_value()?;
    cursor.skip_ws();
    cursor.expect_byte(b',', "','")?;
    cursor.expect_word("\"current\"")?;
    cursor.skip_ws();
    cursor.expect_byte(b':', "':'")?;
    let current = cursor.parse_value()?;
    cursor.skip_ws();
    cursor.expect_byte(b'}', "'}'")?;
    cursor.skip_ws();
    if cursor.at != cursor.text.len() {
        return Err(AdoptError::PinRecordUnreadable {
            detail: format!("trailing text at byte {}", cursor.at),
        });
    }
    Ok(PinRecord { previous, current })
}

pub fn record_pin_operation(
    root: &Path,
    previous: Option<&str>,
    current: Option<&str>,
) -> Result<(), AdoptError> {
    let record = PinRecord {
        previous: previous.map(str::to_owned),
        current: current.map(str::to_owned),
    };
    let dir = root.join(".dx");
    std::fs::create_dir_all(&dir).map_err(|e| AdoptError::CreateDxDir {
        detail: e.to_string(),
    })?;
    dx_atomic_fs::write_atomic(
        &dir.join("version-history.json"),
        format!("{}\n", render_pin_record(&record)).as_bytes(),
    )
    .map_err(|e| AdoptError::WritePinRecord {
        detail: e.to_string(),
    })?;
    Ok(())
}

pub fn read_pin_record(root: &Path) -> Result<PinRecord, AdoptError> {
    let raw = std::fs::read_to_string(root.join(VERSION_HISTORY_REL)).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            return AdoptError::PinRecordMissing;
        }
        AdoptError::PinRecordUnreadable {
            detail: e.to_string(),
        }
    })?;
    parse_pin_record(&raw)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleDependency {
    pub workspace_module: Option<(String, Option<String>)>,
    pub declared: Option<String>,
    pub override_kind: Option<String>,
    pub override_source: Option<String>,
}

fn strip_module_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut skipping = false;
    for c in text.chars() {
        if c == '\n' {
            skipping = false;
            out.push(c);
            continue;
        }
        if skipping {
            continue;
        }
        if let Some(open) = quote {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == open {
                quote = None;
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            out.push(c);
            continue;
        }
        if c == '#' {
            skipping = true;
            continue;
        }
        out.push(c);
    }
    out
}

fn take_balanced(text: &[u8], mut at: usize) -> Option<(String, usize)> {
    let mut depth = 1usize;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let start = at;
    while at < text.len() {
        let byte = text[at];
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == open {
                quote = None;
            }
            at += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' => {
                quote = Some(byte);
                at += 1;
            }
            b'(' => {
                depth += 1;
                at += 1;
            }
            b')' => {
                depth -= 1;
                at += 1;
                if depth == 0 {
                    let inner = std::str::from_utf8(&text[start..at - 1]).ok()?;
                    return Some((inner.to_owned(), at));
                }
            }
            _ => {
                at += 1;
            }
        }
    }
    None
}

fn module_arg(args: &str, key: &str) -> Option<String> {
    let bytes = args.as_bytes();
    let mut at = 0usize;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut parts: Vec<&str> = Vec::new();
    let mut part_start = 0usize;
    while at <= bytes.len() {
        let byte = if at < bytes.len() { bytes[at] } else { b',' };
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == open {
                quote = None;
            }
            at += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' => {
                quote = Some(byte);
                at += 1;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                at += 1;
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                at += 1;
            }
            b',' if depth == 0 => {
                parts.push(&args[part_start..at]);
                at += 1;
                part_start = at;
            }
            _ => {
                at += 1;
            }
        }
    }
    for part in parts {
        let part = part.trim();
        if part.len() <= key.len() || !part.starts_with(key) {
            continue;
        }
        let rest = part[key.len()..].trim_start();
        if !rest.starts_with('=') {
            continue;
        }
        let value = rest[1..].trim_start();
        let quote = value.as_bytes().first()?;
        if *quote != b'"' && *quote != b'\'' {
            continue;
        }
        let mut out = String::new();
        let mut chars = value[1..].chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            if c as u8 == *quote {
                closed = true;
                break;
            }
            out.push(c);
        }
        if closed {
            return Some(out);
        }
    }
    None
}

pub fn parse_module_dependency(text: &str) -> ModuleDependency {
    let clean = strip_module_comments(text);
    let bytes = clean.as_bytes();
    let mut at = 0usize;
    let mut dependency = ModuleDependency {
        workspace_module: None,
        declared: None,
        override_kind: None,
        override_source: None,
    };
    while at < bytes.len() {
        if !(bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
            at += 1;
            continue;
        }
        let start = at;
        while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
            at += 1;
        }
        let name = &clean[start..at];
        let mut next = at;
        while next < bytes.len() && bytes[next].is_ascii_whitespace() {
            next += 1;
        }
        if next >= bytes.len() || bytes[next] != b'(' {
            continue;
        }
        let Some((args, end)) = take_balanced(bytes, next + 1) else {
            break;
        };
        at = end;
        if name == "module" {
            if dependency.workspace_module.is_none() {
                if let Some(module_name) = module_arg(&args, "name") {
                    dependency.workspace_module = Some((module_name, module_arg(&args, "version")));
                }
            }
            continue;
        }
        if name == "bazel_dep" {
            if dependency.declared.is_none()
                && module_arg(&args, "name").as_deref() == Some("rules_dx")
            {
                dependency.declared = module_arg(&args, "version");
            }
            continue;
        }
        if !name.ends_with("_override") {
            continue;
        }
        if dependency.override_kind.is_some() {
            continue;
        }
        if module_arg(&args, "module_name").as_deref() != Some("rules_dx") {
            continue;
        }
        let mut source = module_arg(&args, "path").or_else(|| module_arg(&args, "remote"));
        if name == "git_override" {
            if let (Some(remote), Some(commit)) = (source.clone(), module_arg(&args, "commit")) {
                source = Some(format!("{remote}@{commit}"));
            }
        }
        if source.is_none() {
            source = module_arg(&args, "version");
        }
        dependency.override_kind = Some(name.to_owned());
        dependency.override_source = source;
    }
    dependency
}

pub fn read_module_dependency(root: &Path) -> ModuleDependency {
    let text = std::fs::read_to_string(root.join("MODULE.bazel")).unwrap_or_default();
    parse_module_dependency(&text)
}

pub fn render_module_dependency(dependency: &ModuleDependency) -> String {
    if let Some((name, version)) = &dependency.workspace_module {
        if name == "rules_dx" {
            return match version {
                Some(version) => format!("rules_dx {version} (workspace module)"),
                None => "rules_dx unknown version (workspace module)".to_owned(),
            };
        }
    }
    match (
        &dependency.declared,
        &dependency.override_kind,
        &dependency.override_source,
    ) {
        (Some(version), None, _) => format!("rules_dx {version}"),
        (Some(version), Some(kind), Some(source)) => {
            format!("rules_dx {version} via {kind} {source}")
        }
        (Some(version), Some(kind), None) => format!("rules_dx {version} via {kind}"),
        (None, Some(kind), Some(source)) if kind == "single_version_override" => {
            format!("rules_dx {source} via {kind}")
        }
        (None, Some(kind), Some(source)) => {
            format!("rules_dx unknown version via {kind} {source}")
        }
        (None, Some(kind), None) => format!("rules_dx unknown version via {kind}"),
        (None, None, _) => "unknown (no rules_dx dependency in MODULE.bazel)".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_holds_only_on_equal_nonempty_versions() {
        assert!(version_pin_matches_module("1.2.3", "1.2.3"));
        assert!(!version_pin_matches_module("1.2.3", "1.2.4"));
        assert!(!version_pin_matches_module("", ""));
        assert!(!version_pin_matches_module("1.2.3", ""));
        assert!(!version_pin_matches_module("abc", "abc"));
        assert!(!version_pin_matches_module("v1.2.3", "v1.2.3"));
        assert!(!version_pin_matches_module("1.2", "1.2"));
        assert!(version_pin_matches_module("1.2.3-alpha.1", "1.2.3-alpha.1"));
        assert!(!version_pin_matches_module(
            "1.2.3-alpha.1",
            "1.2.3-alpha.2"
        ));
        assert!(!version_pin_matches_module("1.2.3", "1.2.3-alpha.1"));
    }

    #[test]
    fn rollback_re_pins_only_the_known_previous() {
        assert!(rollback_re_pins_previous("1.2.4", "1.2.3", "1.2.3"));
        assert!(!rollback_re_pins_previous("1.2.3", "1.2.3", "1.2.3"));
        assert!(!rollback_re_pins_previous("1.2.4", "1.2.2", "1.2.3"));
        assert!(!rollback_re_pins_previous("1.2.4", "", ""));
    }

    #[test]
    fn delivered_version_matches_module() {
        assert!(version_pin_matches_module(DX_VERSION, MODULE_VERSION));
        assert!(!rollback_re_pins_previous(
            DX_VERSION,
            PREVIOUS_VERSION,
            PREVIOUS_VERSION
        ));
    }

    #[test]
    fn pin_record_round_trips_through_json() {
        for record in [
            PinRecord {
                previous: None,
                current: Some("0.1.0".to_owned()),
            },
            PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            },
            PinRecord {
                previous: Some("0.1.0".to_owned()),
                current: None,
            },
            PinRecord {
                previous: None,
                current: None,
            },
            PinRecord {
                previous: Some("a\"b\\c\nd\teé".to_owned()),
                current: Some("\u{1}tail".to_owned()),
            },
        ] {
            let rendered = render_pin_record(&record);
            assert_eq!(parse_pin_record(&rendered).expect("round trip"), record);
            assert_eq!(
                parse_pin_record(&format!("  {rendered} \n")).expect("padded"),
                record
            );
        }
        assert_eq!(
            render_pin_record(&PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            }),
            "{\"previous\":\"0.0.0\",\"current\":\"0.1.0\"}"
        );
        assert_eq!(
            render_pin_record(&PinRecord {
                previous: None,
                current: None,
            }),
            "{\"previous\":null,\"current\":null}"
        );
    }

    #[test]
    fn pin_record_rejects_anything_but_its_shape() {
        for text in [
            "",
            "{}",
            "{\"previous\":null}",
            "{\"previous\":null,\"current\":null",
            "{\"previous\":null,\"current\":null}}",
            "{\"previous\":null,\"current\":null} trailing",
            "{\"previous\":\"0.0.0\"}",
            "{\"previous\":0,\"current\":null}",
            "{\"previous\":null,\"current\":1.5}",
            "{\"current\":null,\"previous\":null}",
            "{\"previous\":null,\"CURRENT\":null}",
            "{\"previous\":\"unterminated,\"current\":null}",
            "{\"previous\":\"bad\\escape\",\"current\":null}",
            "{\"previous\":\"bad\\u12\",\"current\":null}",
            "{\"previous\":\"bad\\uzzzz\",\"current\":null}",
            "{\"previous\":nul,\"current\":null}",
            "{\"previous\":null,\"current\":null,\"extra\":1}",
            "null",
        ] {
            assert!(parse_pin_record(text).is_err(), "{text:?}");
        }
        assert_eq!(
            parse_pin_record("{\"previous\":null,\"current\":null}").expect("canonical"),
            PinRecord {
                previous: None,
                current: None,
            }
        );
    }

    #[test]
    fn pin_record_error_strings_name_the_blocker() {
        assert_eq!(
            AdoptError::PinRecordMissing.to_string(),
            "no recorded pin operation: nothing to roll back"
        );
        assert_eq!(
            AdoptError::PinRecordUnreadable {
                detail: "denied".to_owned(),
            }
            .to_string(),
            "unreadable pin record: denied"
        );
        assert_eq!(
            AdoptError::WritePinRecord {
                detail: "denied".to_owned(),
            }
            .to_string(),
            "write pin record: denied"
        );
        assert_eq!(
            AdoptError::NoQualifiedManifest {
                from: "1.2.3".to_owned(),
                to: "2.0.0".to_owned(),
                manifest: "migrate-v1-to-v2.json".to_owned(),
            }
            .to_string(),
            "no qualified migration 1.2.3 -> 2.0.0: manifest migrate-v1-to-v2.json is not shipped"
        );
    }

    #[test]
    fn pin_record_file_round_trips_and_missing_is_explicit() {
        let root =
            std::env::temp_dir().join(format!("dx-adopt-record-{}-roundtrip", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            read_pin_record(&root).unwrap_err(),
            AdoptError::PinRecordMissing
        );
        record_pin_operation(&root, Some("0.0.0"), Some("0.1.0")).expect("record");
        assert_eq!(
            read_pin_record(&root).expect("read back"),
            PinRecord {
                previous: Some("0.0.0".to_owned()),
                current: Some("0.1.0".to_owned()),
            }
        );
        record_pin_operation(&root, None, None).expect("blank record");
        assert_eq!(
            read_pin_record(&root).expect("blank back"),
            PinRecord {
                previous: None,
                current: None,
            }
        );
        std::fs::write(root.join(VERSION_HISTORY_REL), "not json").expect("corrupt");
        assert!(read_pin_record(&root).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn module_dependency_names_self_declared_and_overrides() {
        let me = parse_module_dependency("module(name = \"rules_dx\", version = \"0.0.0\")\n");
        assert_eq!(
            me.workspace_module,
            Some(("rules_dx".to_owned(), Some("0.0.0".to_owned())))
        );
        assert_eq!(
            render_module_dependency(&me),
            "rules_dx 0.0.0 (workspace module)"
        );
        let bare = parse_module_dependency("module(name = \"rules_dx\")\n");
        assert_eq!(
            render_module_dependency(&bare),
            "rules_dx unknown version (workspace module)"
        );
        let consumer = parse_module_dependency(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\n",
        );
        assert_eq!(consumer.declared, Some("1.2.3".to_owned()));
        assert_eq!(consumer.override_kind, None);
        assert_eq!(render_module_dependency(&consumer), "rules_dx 1.2.3");
        let local = parse_module_dependency(
            "module(name = \"demo\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nlocal_path_override(module_name = \"rules_dx\", path = \"../rules_dx\")\n",
        );
        assert_eq!(local.override_kind, Some("local_path_override".to_owned()));
        assert_eq!(local.override_source, Some("../rules_dx".to_owned()));
        assert_eq!(
            render_module_dependency(&local),
            "rules_dx 1.2.3 via local_path_override ../rules_dx"
        );
        let git = parse_module_dependency(
            "module(name = \"demo\")\ngit_override(module_name = \"rules_dx\", remote = \"https://example.test/dx\", commit = \"abc123\")\n",
        );
        assert_eq!(
            render_module_dependency(&git),
            "rules_dx unknown version via git_override https://example.test/dx@abc123"
        );
        let single = parse_module_dependency(
            "module(name = \"demo\")\nsingle_version_override(module_name = \"rules_dx\", version = \"2.0.0\")\n",
        );
        assert_eq!(
            render_module_dependency(&single),
            "rules_dx 2.0.0 via single_version_override"
        );
    }

    #[test]
    fn module_dependency_ignores_noise_and_unknowns() {
        let noisy = parse_module_dependency(
            "# leading comment\nmodule( name = 'demo' ) # trailing\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nbazel_dep(\n    name = \"rules_dx\",\n    version = \"1.2.3\", # pinned\n)\nlocal_path_override(module_name = \"other\", path = \"elsewhere\")\n",
        );
        assert_eq!(noisy.workspace_module, Some(("demo".to_owned(), None)));
        assert_eq!(noisy.declared, Some("1.2.3".to_owned()));
        assert_eq!(noisy.override_kind, None);
        assert_eq!(render_module_dependency(&noisy), "rules_dx 1.2.3");
        let quoted = parse_module_dependency(
            "module(name = \"demo # not a comment\")\nbazel_dep(name = \"rules_dx\", version = \"1.2.3#kept\")\n",
        );
        assert_eq!(quoted.declared, Some("1.2.3#kept".to_owned()));
        let empty = parse_module_dependency("");
        assert_eq!(empty.declared, None);
        assert_eq!(
            render_module_dependency(&empty),
            "unknown (no rules_dx dependency in MODULE.bazel)"
        );
        let broken = parse_module_dependency("bazel_dep(name = \"rules_dx\"");
        assert_eq!(broken.declared, None);
        assert_eq!(
            render_module_dependency(&broken),
            "unknown (no rules_dx dependency in MODULE.bazel)"
        );
        let root =
            std::env::temp_dir().join(format!("dx-adopt-moduledep-{}-missing", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("dir");
        assert_eq!(
            render_module_dependency(&read_module_dependency(&root)),
            "unknown (no rules_dx dependency in MODULE.bazel)"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
