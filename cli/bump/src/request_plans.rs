use super::*;
use regex::Regex;
use std::sync::LazyLock;

pub(super) fn plan_bazelversion(
    content: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: ".bazelversion".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return Err(BumpError::UnsupportedManifest {
            manifest: ".bazelversion".to_owned(),
            reason: "expected one single-line version".to_owned(),
        });
    }
    let newline = content.ends_with('\n');
    if newline {
        Ok(format!("{new}\n"))
    } else {
        Ok(new)
    }
}

pub(super) fn plan_module_bazel(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: "MODULE.bazel".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let module = dx_module_bazel::ModuleFile::parse(content).map_err(module_error(package))?;
    module
        .set_bazel_dep_version(package, &new)
        .map_err(module_error(package))
}

fn module_error(package: &str) -> impl Fn(dx_module_bazel::ModuleError) -> BumpError + '_ {
    move |error| match error {
        dx_module_bazel::ModuleError::NotFound { .. } => BumpError::NotFound {
            manifest: "MODULE.bazel".to_owned(),
            package: package.to_owned(),
        },
        dx_module_bazel::ModuleError::Ambiguous { count, .. } => BumpError::Ambiguous {
            manifest: "MODULE.bazel".to_owned(),
            package: package.to_owned(),
            count,
        },
        other => BumpError::UnsupportedManifest {
            manifest: "MODULE.bazel".to_owned(),
            reason: other.to_string(),
        },
    }
}

pub(super) fn plan_package_json(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: "package.json".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let unsupported = |reason: String| BumpError::UnsupportedManifest {
        manifest: "package.json".to_owned(),
        reason,
    };
    let root: serde_json::Value = serde_json::from_str(content)
        .map_err(|_| unsupported("manifest is not valid JSON".to_owned()))?;
    if !root.is_object() {
        return Err(unsupported("manifest root is not an object".to_owned()));
    }
    let mut scan = JsonScan {
        text: content,
        at: 0,
    };
    scan.skip_ws();
    let mut sections = Vec::new();
    let mut section_names = Vec::new();
    for (key, value_at) in scan.members()? {
        if NPM_SECTIONS.contains(&key.as_str()) {
            if section_names.contains(&key) {
                return Err(unsupported(format!("duplicate {key:?} section")));
            }
            section_names.push(key);
            sections.push(value_at);
        }
    }
    let mut spans = Vec::new();
    for value_at in sections {
        scan.at = value_at;
        if !scan.text[value_at..].starts_with('{') {
            return Err(unsupported(
                "dependency section is not an object".to_owned(),
            ));
        }
        for (key, at) in scan.members()? {
            if key == package {
                spans.push(at);
            }
        }
    }
    match spans.len() {
        0 => Err(BumpError::NotFound {
            manifest: "package.json".to_owned(),
            package: package.to_owned(),
        }),
        1 => {
            scan.at = spans[0];
            if !content[spans[0]..].starts_with('"') {
                return Err(unsupported(format!(
                    "{package:?} requirement is not a string"
                )));
            }
            let end = scan.string_end()?;
            Ok(format!(
                "{}\"{new}\"{}",
                &content[..spans[0]],
                &content[end..]
            ))
        }
        count => Err(BumpError::Ambiguous {
            manifest: "package.json".to_owned(),
            package: package.to_owned(),
            count,
        }),
    }
}

const NPM_SECTIONS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

struct JsonScan<'a> {
    text: &'a str,
    at: usize,
}

impl JsonScan<'_> {
    fn bad() -> BumpError {
        BumpError::UnsupportedManifest {
            manifest: "package.json".to_owned(),
            reason: "manifest is not valid JSON".to_owned(),
        }
    }

    fn skip_ws(&mut self) {
        while self.text[self.at..].starts_with([' ', '\t', '\n', '\r']) {
            self.at += 1;
        }
    }

    fn string_end(&mut self) -> Result<usize, BumpError> {
        let bytes = self.text.as_bytes();
        let mut i = self.at + 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 2,
                b'"' => {
                    self.at = i + 1;
                    return Ok(self.at);
                }
                _ => i += 1,
            }
        }
        Err(Self::bad())
    }

    fn skip_value(&mut self) -> Result<(), BumpError> {
        self.skip_ws();
        match self.text[self.at..].chars().next() {
            Some('"') => self.string_end().map(|_| ()),
            Some(open @ ('{' | '[')) => {
                let close = if open == '{' { '}' } else { ']' };
                self.at += 1;
                loop {
                    self.skip_ws();
                    if self.text[self.at..].starts_with(close) {
                        self.at += 1;
                        return Ok(());
                    }
                    if self.text[self.at..].starts_with(',') {
                        self.at += 1;
                        continue;
                    }
                    if open == '{' {
                        self.skip_value()?;
                        self.skip_ws();
                        if !self.text[self.at..].starts_with(':') {
                            return Err(Self::bad());
                        }
                        self.at += 1;
                    }
                    self.skip_value()?;
                }
            }
            Some(_) => {
                let rest = &self.text[self.at..];
                let len = rest
                    .find([',', '}', ']', ' ', '\t', '\n', '\r'])
                    .unwrap_or(rest.len());
                if len == 0 {
                    return Err(Self::bad());
                }
                self.at += len;
                Ok(())
            }
            None => Err(Self::bad()),
        }
    }

    fn members(&mut self) -> Result<Vec<(String, usize)>, BumpError> {
        self.skip_ws();
        if !self.text[self.at..].starts_with('{') {
            return Err(Self::bad());
        }
        self.at += 1;
        let mut out = Vec::new();
        loop {
            self.skip_ws();
            if self.text[self.at..].starts_with('}') {
                return Ok(out);
            }
            if self.text[self.at..].starts_with(',') {
                self.at += 1;
                continue;
            }
            let start = self.at;
            let end = self.string_end()?;
            let key: String =
                serde_json::from_str(&self.text[start..end]).map_err(|_| Self::bad())?;
            self.skip_ws();
            if !self.text[self.at..].starts_with(':') {
                return Err(Self::bad());
            }
            self.at += 1;
            self.skip_ws();
            let value_at = self.at;
            self.skip_value()?;
            out.push((key, value_at));
        }
    }
}

pub(super) fn plan_go_mod(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => format!("v{version}"),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: "third_party/go/go.mod".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let mut matches = 0usize;
    let mut out = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with('#') {
            out.push_str(line);
            continue;
        }
        if line_contains_module_token(line, package) && line.contains('v') {
            match replace_go_version_token(line, &new) {
                Some(replaced) => {
                    matches += 1;
                    out.push_str(&replaced);
                    continue;
                }
                None => {
                    return Err(BumpError::UnsupportedManifest {
                        manifest: "third_party/go/go.mod".to_owned(),
                        reason: format!("{package:?} has no replaceable v-prefixed version token"),
                    });
                }
            }
        }
        out.push_str(line);
    }
    match matches {
        1 => Ok(out),
        0 => Err(BumpError::NotFound {
            manifest: "third_party/go/go.mod".to_owned(),
            package: package.to_owned(),
        }),
        count => Err(BumpError::Ambiguous {
            manifest: "third_party/go/go.mod".to_owned(),
            package: package.to_owned(),
            count,
        }),
    }
}

pub(super) fn go_version_token_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> =
        LazyLock::new(|| Regex::new(r"v[0-9.]+(?:[-+][^\s]*)?").ok());
    RE.as_ref()
}

pub(super) fn has_version_shape(token: &str) -> bool {
    let mut digits = false;
    let mut dots = false;
    for byte in token.bytes().skip(1) {
        if byte.is_ascii_digit() {
            digits = true;
        } else if byte == b'.' {
            dots = true;
        } else {
            break;
        }
    }
    digits && dots
}

pub(super) fn line_contains_module_token(line: &str, package: &str) -> bool {
    let pattern = format!(r"(?:^|\s){}(?:\s|$)", regex::escape(package));
    match Regex::new(&pattern) {
        Ok(re) => re.is_match(line),
        Err(_) => line.split_whitespace().any(|token| token == package),
    }
}

pub(super) fn replace_go_version_token(line: &str, new: &str) -> Option<String> {
    if let Some(re) = go_version_token_re() {
        let mut last: Option<(usize, usize)> = None;
        for matched in re.find_iter(line) {
            if has_version_shape(matched.as_str()) {
                last = Some((matched.start(), matched.end()));
            }
        }
        if let Some((start, end)) = last {
            let mut replaced = String::with_capacity(line.len());
            replaced.push_str(&line[..start]);
            replaced.push_str(new);
            replaced.push_str(&line[end..]);
            return Some(replaced);
        }
        if re.find_iter(line).next().is_some() {
            return None;
        }
    }
    replace_go_version_token_fallback(line, new)
}

pub(super) fn replace_go_version_token_fallback(line: &str, new: &str) -> Option<String> {
    let mut last_start: Option<usize> = None;
    let mut last_end: Option<usize> = None;
    let bytes = line.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'v' {
            let mut end = index + 1;
            let mut digits = 0usize;
            let mut dots = 0usize;
            while end < bytes.len() {
                let byte = bytes[end];
                if byte.is_ascii_digit() {
                    digits += 1;
                    end += 1;
                } else if byte == b'.' {
                    dots += 1;
                    end += 1;
                } else if byte == b'-' || byte == b'+' {
                    end += 1;
                    while end < bytes.len() && !bytes[end].is_ascii_whitespace() {
                        end += 1;
                    }
                    break;
                } else {
                    break;
                }
            }
            if digits > 0 && dots > 0 {
                last_start = Some(index);
                last_end = Some(end);
            }
            index = end.max(index + 1);
        } else {
            index += 1;
        }
    }
    match (last_start, last_end) {
        (Some(start), Some(end)) => {
            let mut replaced = String::with_capacity(line.len());
            replaced.push_str(&line[..start]);
            replaced.push_str(new);
            replaced.push_str(&line[end..]);
            Some(replaced)
        }
        _ => None,
    }
}

pub(super) fn plan_maven_module_bazel(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: "MODULE.bazel".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let module = dx_module_bazel::ModuleFile::parse(content).map_err(module_error(package))?;
    let artifacts = module.maven_artifacts().map_err(module_error(package))?;
    let mut selected = Vec::new();
    for literal in artifacts {
        let parts: Vec<&str> = literal.value.split(':').collect();
        let identity = parts.len() >= 2 && format!("{}:{}", parts[0], parts[1]) == package;
        if !identity {
            continue;
        }
        if !(3..=5).contains(&parts.len()) || parts.iter().any(|part| part.trim().is_empty()) {
            return Err(BumpError::UnsupportedManifest {
                manifest: "MODULE.bazel".to_owned(),
                reason: format!("malformed Maven coordinate {:?}", literal.value),
            });
        }
        selected.push((literal, parts));
    }
    match selected.as_slice() {
        [] => Err(BumpError::NotFound {
            manifest: "MODULE.bazel".to_owned(),
            package: package.to_owned(),
        }),
        [(literal, parts)] => {
            let mut next = parts[..parts.len() - 1].join(":");
            next.push(':');
            next.push_str(&new);
            module
                .replace(literal, &next)
                .map_err(module_error(package))
        }
        many => Err(BumpError::Ambiguous {
            manifest: "MODULE.bazel".to_owned(),
            package: package.to_owned(),
            count: many.len(),
        }),
    }
}

pub(super) fn plan_paket_dependencies(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: "third_party/dotnet/paket.dependencies".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let mut matches = 0usize;
    let mut out = String::with_capacity(content.len());
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            out.push_str(line);
            continue;
        }
        if paket_line_targets_package(line, package) {
            match replace_paket_version_token(line, &new) {
                Some(replaced) => {
                    matches += 1;
                    out.push_str(&replaced);
                    continue;
                }
                None => {
                    return Err(BumpError::UnsupportedManifest {
                        manifest: "third_party/dotnet/paket.dependencies".to_owned(),
                        reason: format!("{package:?} has no replaceable version token"),
                    });
                }
            }
        }
        out.push_str(line);
    }
    match matches {
        1 => Ok(out),
        0 => Err(BumpError::NotFound {
            manifest: "third_party/dotnet/paket.dependencies".to_owned(),
            package: package.to_owned(),
        }),
        count => Err(BumpError::Ambiguous {
            manifest: "third_party/dotnet/paket.dependencies".to_owned(),
            package: package.to_owned(),
            count,
        }),
    }
}

pub(super) fn paket_line_targets_package(line: &str, package: &str) -> bool {
    let mut tokens = line.split_whitespace();
    match (tokens.next(), tokens.next()) {
        (Some(kind), Some(id)) => kind == "nuget" && id == package,
        _ => false,
    }
}

pub(super) fn replace_paket_version_token(line: &str, new: &str) -> Option<String> {
    let newline = line
        .strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'));
    let (body, ending) = match newline {
        Some(stripped) => (stripped, &line[stripped.len()..]),
        None => (line, ""),
    };
    let (head, comment) = match body.find('#') {
        Some(at) => (&body[..at], &body[at..]),
        None => (body, ""),
    };
    let parts: Vec<&str> = head.split_whitespace().collect();
    if parts.len() < 3 || parts[0] != "nuget" {
        return None;
    }
    let old = parts[2];
    let old_at = head.rfind(old)?;
    let mut replaced = String::with_capacity(line.len());
    replaced.push_str(&head[..old_at]);
    replaced.push_str(new);
    replaced.push_str(&head[old_at + old.len()..]);
    replaced.push_str(comment);
    replaced.push_str(ending);
    Some(replaced)
}

pub(super) fn plan_github_workflow(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    match version {
        WidenVersion::GitTag(tag) => Err(BumpError::NeedsSha {
            package: package.to_owned(),
            tag: tag.clone(),
        }),
        WidenVersion::GitCommit(sha) => {
            let mut matches = 0usize;
            let mut out = String::with_capacity(content.len());
            for line in content.split_inclusive('\n') {
                if crate::gha::uses_pin_targets_package(line, package) {
                    let needle = format!("{package}@");
                    match replace_gha_sha(line, &needle, sha) {
                        Some(replaced) => {
                            matches += 1;
                            out.push_str(&replaced);
                            continue;
                        }
                        None => {
                            return Err(BumpError::UnsupportedManifest {
                                manifest: WORKFLOW_DIR.to_owned(),
                                reason: format!("{package:?} has no replaceable @SHA pin"),
                            });
                        }
                    }
                }
                out.push_str(line);
            }
            match matches {
                0 => Err(BumpError::NotFound {
                    manifest: WORKFLOW_DIR.to_owned(),
                    package: package.to_owned(),
                }),
                1 => Ok(out),
                count => Err(BumpError::Ambiguous {
                    manifest: WORKFLOW_DIR.to_owned(),
                    package: package.to_owned(),
                    count,
                }),
            }
        }
        WidenVersion::Semver(_) => Err(BumpError::UnsupportedManifest {
            manifest: WORKFLOW_DIR.to_owned(),
            reason: "github-actions pins are tag/SHA-shaped, not semver".to_owned(),
        }),
    }
}

pub(super) fn replace_gha_sha(line: &str, needle: &str, sha: &str) -> Option<String> {
    let at = line.find(needle)? + needle.len();
    let rest = &line[at..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let mut replaced = String::with_capacity(line.len());
    replaced.push_str(&line[..at]);
    replaced.push_str(sha);
    replaced.push_str(&rest[end..]);
    Some(replaced)
}

pub(super) fn target_prefix_re() -> Option<&'static Regex> {
    static RE: LazyLock<Option<Regex>> = LazyLock::new(|| Regex::new(r"^(//|@)").ok());
    RE.as_ref()
}

pub(super) fn is_target_shape(text: &str) -> bool {
    if let Some(re) = target_prefix_re() {
        if re.is_match(text) {
            return true;
        }
    } else if text.starts_with("//") || text.starts_with('@') {
        return true;
    }
    if text.contains('/') && !text.contains(':') {
        return true;
    }
    if text.contains('/')
        && BumpSet::parse(text.split_once(':').map_or("", |(head, _)| head)).is_none()
    {
        return true;
    }
    text == "..."
        || text == "MODULE.bazel"
        || text == ".bazelversion"
        || text == "package.json"
        || text == "Cargo.toml"
}

const BAZEL_CHARSET: &str = "bazel modules use [A-Za-z0-9_.-] only (or .bazelversion)";
const GITHUB_ACTIONS_CHARSET: &str = "github-actions owner/repo use [A-Za-z0-9_.-] only";

pub(super) fn validate_package(set: BumpSet, package: &str) -> Result<(), BumpError> {
    let invalid = |reason: &'static str| BumpError::InvalidPackage {
        set: set.name(),
        package: package.to_owned(),
        reason,
    };
    let reason = match set {
        BumpSet::Bazel => {
            if package == ".bazelversion" {
                Ok(())
            } else {
                dx_identity::validate_dotted(package, BAZEL_CHARSET)
            }
        }
        BumpSet::Cargo => dx_identity::validate_cargo(package),
        BumpSet::Npm => dx_identity::validate_npm(package),
        BumpSet::Go => dx_identity::validate_go(package),
        BumpSet::Maven => dx_identity::validate_maven(package),
        BumpSet::NuGet => dx_identity::validate_nuget(package),
        BumpSet::GithubActions => match package.split_once('/') {
            None => Err("github-actions identities are owner/repo"),
            Some((owner, repo)) => {
                if owner.is_empty()
                    || repo.is_empty()
                    || repo.contains('/')
                    || repo.contains(' ')
                    || owner.contains(' ')
                {
                    Err("github-actions identities are owner/repo")
                } else {
                    dx_identity::validate_dotted_pair(owner, repo, GITHUB_ACTIONS_CHARSET)
                }
            }
        },
    };
    reason.map_err(invalid)
}
