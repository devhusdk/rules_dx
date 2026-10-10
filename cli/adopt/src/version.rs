use std::path::Path;

use super::AdoptError;

pub const DX_VERSION: &str = "0.0.0";
pub const MODULE_VERSION: &str = "0.0.0";
pub const PREVIOUS_VERSION: &str = "0.0.0";

pub const VERSION_RECOVERY_REL: &str = ".dx/version-recovery.json";

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleIdentity {
    pub version: String,
    pub source: String,
}

fn stanza_attr(stanza: &str, key: &str) -> Option<String> {
    let mut rest = stanza;
    while let Some(pos) = rest.find(key) {
        let after_key = &rest[pos + key.len()..];
        let after_eq = after_key.trim_start().strip_prefix('=')?.trim_start();
        if !rest[..pos].trim_end().ends_with('(') && !rest[..pos].trim_end().ends_with(',') {
            rest = after_eq;
            continue;
        }
        let mut chars = after_eq.chars();
        if chars.next() != Some('"') {
            rest = after_eq;
            continue;
        }
        let mut value = String::new();
        let mut escaped = false;
        for c in chars {
            if escaped {
                value.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                return Some(value);
            } else {
                value.push(c);
            }
        }
        return None;
    }
    None
}

fn stanza_targets_rules_dx(kind: &str, stanza: &str) -> bool {
    if kind == "module" {
        return stanza_attr(stanza, "name").as_deref() == Some("rules_dx");
    }
    stanza_attr(stanza, "module_name").as_deref() == Some("rules_dx")
        || stanza_attr(stanza, "name").as_deref() == Some("rules_dx")
}

fn split_stanzas(text: &str, kind: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find(kind) {
        let before = &rest[..pos];
        let tail = before.trim_end_matches([' ', '\t']);
        let line_start = match tail.rfind('\n') {
            Some(i) => &tail[i + 1..],
            None => tail,
        };
        let starts_line = (tail.is_empty() || tail.ends_with('\n')) && !line_start.starts_with('#');
        let after = &rest[pos + kind.len()..];
        if !starts_line || !after.trim_start().starts_with('(') {
            rest = after;
            continue;
        }
        let mut depth = 0i32;
        let mut end = None;
        let mut in_string = false;
        let mut escaped = false;
        for (i, c) in after.char_indices() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    in_string = false;
                }
            } else if c == '"' {
                in_string = true;
            } else if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
        }
        match end {
            Some(i) => {
                out.push(after[..=i].to_owned());
                rest = &after[i + 1..];
            }
            None => break,
        }
    }
    out
}

pub fn parse_module_identity(text: &str) -> ModuleIdentity {
    let mut dep_version = None;
    for stanza in split_stanzas(text, "bazel_dep") {
        if stanza_targets_rules_dx("bazel_dep", &stanza) {
            dep_version = stanza_attr(&stanza, "version");
            break;
        }
    }
    for kind in [
        "single_version_override",
        "git_override",
        "local_path_override",
        "archive_override",
    ] {
        for stanza in split_stanzas(text, kind) {
            if stanza_targets_rules_dx(kind, &stanza) {
                let version = stanza_attr(&stanza, "version")
                    .or(dep_version.clone())
                    .unwrap_or_else(|| MODULE_VERSION.to_owned());
                return ModuleIdentity {
                    version,
                    source: kind.to_owned(),
                };
            }
        }
    }
    if let Some(version) = dep_version {
        return ModuleIdentity {
            version,
            source: "registry".to_owned(),
        };
    }
    for stanza in split_stanzas(text, "module") {
        if stanza_targets_rules_dx("module", &stanza) {
            return ModuleIdentity {
                version: stanza_attr(&stanza, "version")
                    .unwrap_or_else(|| MODULE_VERSION.to_owned()),
                source: "module".to_owned(),
            };
        }
    }
    ModuleIdentity {
        version: MODULE_VERSION.to_owned(),
        source: "binary".to_owned(),
    }
}

pub fn resolve_module_identity(root: &Path) -> ModuleIdentity {
    let text = match std::fs::read_to_string(root.join("MODULE.bazel")) {
        Ok(text) => text,
        Err(_) => {
            return ModuleIdentity {
                version: MODULE_VERSION.to_owned(),
                source: "binary".to_owned(),
            };
        }
    };
    parse_module_identity(&text)
}

pub fn module_incompatible_with_binary(root: &Path) -> Option<String> {
    let identity = resolve_module_identity(root);
    if identity.source == "binary" {
        return None;
    }
    let resolved = match semver::Version::parse(&identity.version) {
        Ok(resolved) => resolved,
        Err(_) => return None,
    };
    let binary = match semver::Version::parse(DX_VERSION) {
        Ok(binary) => binary,
        Err(_) => return None,
    };
    if resolved == binary {
        return None;
    }
    Some(format!(
        "module {} (source {}) is incompatible with binary {DX_VERSION}",
        identity.version, identity.source
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionRecovery {
    pub previous: String,
    pub post: String,
    pub post_bytes: String,
    pub module: String,
    pub operation: String,
}

fn escape_json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn unescape_json_string(raw: &str) -> Option<String> {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            '/' => out.push('/'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                if hex.len() != 4 {
                    return None;
                }
                let code = u32::from_str_radix(&hex, 16).ok()?;
                out.push(char::from_u32(code)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

fn recovery_field(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let mut rest = text;
    while let Some(pos) = rest.find(&needle) {
        let after = rest[pos + needle.len()..].trim_start();
        let after = after.strip_prefix(':')?.trim_start();
        if !after.starts_with('"') {
            rest = &after[1.min(after.len())..];
            continue;
        }
        let mut raw = String::new();
        let mut escaped = false;
        let mut closed = false;
        for c in after[1..].chars() {
            if escaped {
                raw.push('\\');
                raw.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                closed = true;
                break;
            } else {
                raw.push(c);
            }
        }
        if !closed {
            return None;
        }
        if let Some(value) = unescape_json_string(&raw) {
            return Some(value);
        }
        return None;
    }
    None
}

fn recovery_schema_is_one(text: &str) -> bool {
    let needle = "\"schema\"";
    let mut rest = text;
    while let Some(pos) = rest.find(needle) {
        let after = rest[pos + needle.len()..].trim_start();
        let after = match after.strip_prefix(':') {
            Some(after) => after.trim_start(),
            None => {
                rest = &rest[pos + needle.len()..];
                continue;
            }
        };
        if after.starts_with("\"1\"") {
            return true;
        }
        if after.starts_with('1')
            && after[1..]
                .chars()
                .next()
                .is_none_or(|c| c == ',' || c == '}')
        {
            return true;
        }
        rest = &rest[pos + needle.len()..];
        if rest.is_empty() {
            break;
        }
        rest = &rest[1.min(rest.len())..];
    }
    false
}

pub fn render_version_recovery(record: &VersionRecovery) -> String {
    format!(
        "{{\"schema\":1,\"operation\":\"{}\",\"previous\":\"{}\",\"post\":\"{}\",\"post_bytes\":\"{}\",\"module\":\"{}\"}}",
        escape_json_string(&record.operation),
        escape_json_string(&record.previous),
        escape_json_string(&record.post),
        escape_json_string(&record.post_bytes),
        escape_json_string(&record.module),
    )
}

pub fn parse_version_recovery(text: &str) -> Option<VersionRecovery> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }
    if !recovery_schema_is_one(trimmed) {
        return None;
    }
    Some(VersionRecovery {
        operation: recovery_field(trimmed, "operation")?,
        previous: recovery_field(trimmed, "previous")?,
        post: recovery_field(trimmed, "post")?,
        post_bytes: recovery_field(trimmed, "post_bytes")?,
        module: recovery_field(trimmed, "module")?,
    })
}

pub fn write_version_recovery(root: &Path, record: &VersionRecovery) -> Result<(), AdoptError> {
    let dir = root.join(".dx");
    std::fs::create_dir_all(&dir).map_err(|e| AdoptError::CreateDxDir {
        detail: e.to_string(),
    })?;
    dx_atomic_fs::write_atomic(
        &dir.join("version-recovery.json"),
        render_version_recovery(record).as_bytes(),
    )
    .map_err(|e| AdoptError::WriteVersionRecovery {
        detail: e.to_string(),
    })?;
    Ok(())
}

pub fn read_version_recovery(root: &Path) -> Result<VersionRecovery, AdoptError> {
    let raw = std::fs::read_to_string(root.join(VERSION_RECOVERY_REL)).map_err(|e| {
        AdoptError::ReadVersionRecovery {
            detail: e.to_string(),
        }
    })?;
    parse_version_recovery(&raw).ok_or_else(|| AdoptError::MalformedVersionRecovery {
        detail: VERSION_RECOVERY_REL.to_owned(),
    })
}

pub fn clear_version_recovery(root: &Path) -> Result<(), AdoptError> {
    match std::fs::remove_file(root.join(VERSION_RECOVERY_REL)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(AdoptError::WriteVersionRecovery {
            detail: e.to_string(),
        }),
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
    fn module_identity_prefers_overrides_over_registry() {
        let registry =
            parse_module_identity("bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\n");
        assert_eq!(registry.version, "1.2.3");
        assert_eq!(registry.source, "registry");
        let pinned = parse_module_identity(
            "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nsingle_version_override(module_name = \"rules_dx\", version = \"1.2.4\")\n",
        );
        assert_eq!(pinned.version, "1.2.4");
        assert_eq!(pinned.source, "single_version_override");
        let local = parse_module_identity(
            "bazel_dep(name = \"rules_dx\", version = \"1.2.3\")\nlocal_path_override(module_name = \"rules_dx\", path = \"../rules_dx\")\n",
        );
        assert_eq!(local.version, "1.2.3");
        assert_eq!(local.source, "local_path_override");
        let git = parse_module_identity(
            "git_override(module_name = \"rules_dx\", remote = \"https://example.com/rules_dx\", commit = \"abc123\")\n",
        );
        assert_eq!(git.version, MODULE_VERSION);
        assert_eq!(git.source, "git_override");
        let archive = parse_module_identity(
            "archive_override(module_name = \"rules_dx\", urls = [\"https://example.com/dx.zip\"], integrity = \"sha256-abc=\")\n",
        );
        assert_eq!(archive.source, "archive_override");
        let root = parse_module_identity("module(name = \"rules_dx\", version = \"0.0.0\")\n");
        assert_eq!(root.version, "0.0.0");
        assert_eq!(root.source, "module");
        let missing = parse_module_identity("module(name = \"other\", version = \"9.9.9\")\n");
        assert_eq!(missing.version, MODULE_VERSION);
        assert_eq!(missing.source, "binary");
        let commented =
            parse_module_identity("# bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\n");
        assert_eq!(commented.source, "binary");
        let multiline = parse_module_identity(
            "bazel_dep(\n    name = \"rules_dx\",\n    version = \"2.0.0\",\n)\n",
        );
        assert_eq!(multiline.version, "2.0.0");
        assert_eq!(multiline.source, "registry");
    }

    #[test]
    fn module_identity_resolves_from_workspace_or_binary() {
        let scratch = dx_test_scratch::scratch("dx-adopt-identity-");
        let root = scratch.path().to_path_buf();
        let identity = resolve_module_identity(&root);
        assert_eq!(identity.version, MODULE_VERSION);
        assert_eq!(identity.source, "binary");
        assert!(module_incompatible_with_binary(&root).is_none());
        std::fs::write(
            root.join("MODULE.bazel"),
            "module(name = \"rules_dx\", version = \"0.0.0\")\n",
        )
        .expect("module");
        let identity = resolve_module_identity(&root);
        assert_eq!(identity.source, "module");
        assert!(module_incompatible_with_binary(&root).is_none());
        std::fs::write(
            root.join("MODULE.bazel"),
            "bazel_dep(name = \"rules_dx\", version = \"9.9.9\")\n",
        )
        .expect("module");
        let incompatible = module_incompatible_with_binary(&root).expect("incompatible");
        assert!(incompatible.contains("9.9.9"), "{incompatible}");
        assert!(incompatible.contains("registry"), "{incompatible}");
        assert!(incompatible.contains(DX_VERSION), "{incompatible}");
    }

    #[test]
    fn version_recovery_round_trips_bytes_and_refuses_garbage() {
        let record = VersionRecovery {
            previous: "1.2.3".to_owned(),
            post: "1.2.4".to_owned(),
            post_bytes: "1.2.4\n".to_owned(),
            module: "1.2.4".to_owned(),
            operation: "version-pin".to_owned(),
        };
        let text = render_version_recovery(&record);
        assert_eq!(parse_version_recovery(&text), Some(record));
        assert!(parse_version_recovery("not json").is_none());
        assert!(parse_version_recovery("{\"schema\":1}").is_none());
        assert!(parse_version_recovery("{\"schema\":2,\"operation\":\"x\",\"previous\":\"\",\"post\":\"\",\"post_bytes\":\"\",\"module\":\"\"}").is_none());
        let quoted = VersionRecovery {
            previous: "a\"b\\c".to_owned(),
            post: "d".to_owned(),
            post_bytes: "d\n".to_owned(),
            module: "d".to_owned(),
            operation: "version-pin".to_owned(),
        };
        let text = render_version_recovery(&quoted);
        assert_eq!(parse_version_recovery(&text), Some(quoted));
    }

    #[test]
    fn recovery_record_writes_reads_and_clears() {
        let scratch = dx_test_scratch::scratch("dx-adopt-recovery-");
        let root = scratch.path().to_path_buf();
        assert!(read_version_recovery(&root).is_err());
        assert!(clear_version_recovery(&root).is_ok());
        let record = VersionRecovery {
            previous: "1.2.3".to_owned(),
            post: "1.2.4".to_owned(),
            post_bytes: "1.2.4\n".to_owned(),
            module: MODULE_VERSION.to_owned(),
            operation: "version-pin".to_owned(),
        };
        write_version_recovery(&root, &record).expect("write");
        assert_eq!(read_version_recovery(&root).expect("read"), record);
        std::fs::write(root.join(VERSION_RECOVERY_REL), "garbage").expect("garbage");
        let error = read_version_recovery(&root).expect_err("malformed");
        assert_eq!(
            error,
            AdoptError::MalformedVersionRecovery {
                detail: VERSION_RECOVERY_REL.to_owned()
            }
        );
        assert!(clear_version_recovery(&root).is_ok());
        assert!(read_version_recovery(&root).is_err());
    }
}
