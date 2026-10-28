use super::*;

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

/// Widens one MODULE.bazel dep: the tested override by default, or the floor.
///
/// A tested bump edits the `single_version_override` version when one has a literal
/// version, else the `bazel_dep`; it refuses to drop below the `bazel_dep` floor. A floor
/// bump edits the `bazel_dep` and its `# floor: ` comment; it refuses to rise above the
/// override's tested version.
pub(super) fn plan_module_bazel(
    content: &str,
    package: &str,
    version: &WidenVersion,
    target: &DepVersion,
) -> Result<String, BumpError> {
    use dx_module_bazel::ModuleError;
    use std::cmp::Ordering;
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
    let floor = module
        .bazel_dep_version(package)
        .map_err(module_error(package))?
        .value
        .clone();
    let tested = match module.override_version(package) {
        Ok(literal) => Some(literal.value.clone()),
        Err(ModuleError::OverrideNotFound { .. } | ModuleError::OverrideMissingVersion { .. }) => {
            None
        }
        Err(other) => return Err(module_error(package)(other)),
    };
    match (target, tested) {
        (DepVersion::Tested, Some(_)) => {
            if dx_module_bazel::compare_module_versions(&new, &floor) == Ordering::Less {
                return Err(BumpError::TestedBelowFloor {
                    package: package.to_owned(),
                    tested: new,
                    floor,
                });
            }
            module
                .set_override_version(package, &new)
                .map_err(module_error(package))
        }
        (DepVersion::Tested, None) => module
            .set_bazel_dep_version(package, &new)
            .map_err(module_error(package)),
        (DepVersion::Floor { reason }, tested) => {
            if let Some(tested) = tested {
                if dx_module_bazel::compare_module_versions(&new, &tested) == Ordering::Greater {
                    return Err(BumpError::FloorAboveTested {
                        package: package.to_owned(),
                        floor: new,
                        tested,
                    });
                }
            }
            let edited = module
                .set_bazel_dep_version(package, &new)
                .map_err(module_error(package))?;
            dx_module_bazel::ModuleFile::parse(&edited)
                .and_then(|module| module.set_floor_comment(package, reason))
                .map_err(module_error(package))
        }
    }
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

pub(crate) const NPM_SECTIONS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

pub(crate) struct JsonScan<'a> {
    pub(crate) text: &'a str,
    pub(crate) at: usize,
}

impl JsonScan<'_> {
    fn bad() -> BumpError {
        BumpError::UnsupportedManifest {
            manifest: "package.json".to_owned(),
            reason: "manifest is not valid JSON".to_owned(),
        }
    }

    pub(crate) fn skip_ws(&mut self) {
        while self.text[self.at..].starts_with([' ', '\t', '\n', '\r']) {
            self.at += 1;
        }
    }

    pub(crate) fn string_end(&mut self) -> Result<usize, BumpError> {
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

    pub(crate) fn skip_value(&mut self) -> Result<(), BumpError> {
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

    pub(crate) fn members(&mut self) -> Result<Vec<(String, usize)>, BumpError> {
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
    let manifest = "third_party/go/go.mod".to_owned();
    dx_gomod::set_require_version(content, package, &new).map_err(|error| match error {
        dx_gomod::GoEditError::NotFound => BumpError::NotFound {
            manifest,
            package: package.to_owned(),
        },
        dx_gomod::GoEditError::Ambiguous(count) => BumpError::Ambiguous {
            manifest,
            package: package.to_owned(),
            count,
        },
        dx_gomod::GoEditError::Invalid(reason) => {
            BumpError::UnsupportedManifest { manifest, reason }
        }
    })
}

pub(super) fn plan_maven_module_bazel(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
    install: Option<&str>,
) -> Result<String, BumpError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: manifest.to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let module = dx_module_bazel::ModuleFile::parse(content).map_err(module_error(package))?;
    let artifacts = match install {
        Some(install) => module.maven_artifacts_of(install),
        None => module.maven_artifacts(),
    }
    .map_err(module_error(package))?;
    let mut selected = Vec::new();
    for literal in artifacts {
        let parts: Vec<&str> = literal.value.split(':').collect();
        let identity = parts.len() >= 2 && format!("{}:{}", parts[0], parts[1]) == package;
        if !identity {
            continue;
        }
        if !(3..=5).contains(&parts.len()) || parts.iter().any(|part| part.trim().is_empty()) {
            return Err(BumpError::UnsupportedManifest {
                manifest: manifest.to_owned(),
                reason: format!("malformed Maven coordinate {:?}", literal.value),
            });
        }
        selected.push((literal, parts));
    }
    match selected.as_slice() {
        [] => Err(BumpError::NotFound {
            manifest: manifest.to_owned(),
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
            manifest: manifest.to_owned(),
            package: package.to_owned(),
            count: many.len(),
        }),
    }
}

pub(super) fn plan_paket_dependencies(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, BumpError> {
    let manifest_path = manifest;
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(BumpError::UnsupportedManifest {
                manifest: manifest_path.to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let mut targets = Vec::new();
    let mut offset = 0usize;
    for line in content.split_inclusive('\n') {
        if paket_line_targets_package(line, package) {
            targets.push((offset, line));
        }
        offset += line.len();
    }
    let (at, line) = match targets.as_slice() {
        [] => {
            return Err(BumpError::NotFound {
                manifest: manifest_path.to_owned(),
                package: package.to_owned(),
            });
        }
        [one] => *one,
        many => {
            return Err(BumpError::Ambiguous {
                manifest: manifest_path.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let unsupported = |reason: String| BumpError::UnsupportedManifest {
        manifest: manifest_path.to_owned(),
        reason: format!("{package:?}: {reason}"),
    };
    let decl = parse_paket_declaration(line).map_err(unsupported)?;
    let mut out = String::with_capacity(content.len() + new.len());
    out.push_str(&content[..at + decl.requirement.start]);
    out.push_str(&new);
    out.push_str(&content[at + decl.requirement.end..]);

    let reparsed_at = at;
    let reparsed_line = out[reparsed_at..]
        .split_inclusive('\n')
        .next()
        .unwrap_or_default();
    let reparsed = parse_paket_declaration(reparsed_line).map_err(unsupported)?;
    let kept = reparsed_line[reparsed.requirement.clone()] == *new
        && reparsed.id.eq_ignore_ascii_case(package)
        && reparsed_line[..reparsed.requirement.start] == line[..decl.requirement.start]
        && reparsed_line[reparsed.requirement.end..] == line[decl.requirement.end..];
    if !kept {
        return Err(unsupported("edit did not round-trip".to_owned()));
    }
    Ok(out)
}

struct PaketDeclaration<'a> {
    id: &'a str,
    requirement: std::ops::Range<usize>,
}

pub(crate) fn paket_tokens(line: &str) -> Vec<(usize, usize, &str)> {
    let body = line.trim_end_matches(['\n', '\r']);
    let mut tokens: Vec<(usize, usize, &str)> = Vec::new();
    let mut start = None;
    for (index, ch) in body
        .char_indices()
        .chain(std::iter::once((body.len(), ' ')))
    {
        match (start, ch.is_whitespace()) {
            (None, false) => start = Some(index),
            (Some(from), true) => {
                let token = &body[from..index];
                if token.starts_with('#') || token.starts_with("//") {
                    return tokens;
                }
                if let Some(at) = token.find('#') {
                    tokens.push((from, from + at, &token[..at]));
                    return tokens;
                }
                tokens.push((from, index, token));
                start = None;
            }
            _ => {}
        }
    }
    tokens
}

fn paket_line_targets_package(line: &str, package: &str) -> bool {
    let tokens = paket_tokens(line);
    match tokens.as_slice() {
        [(_, _, kind), (_, _, id), ..] => {
            kind.eq_ignore_ascii_case("nuget") && id.eq_ignore_ascii_case(package)
        }
        _ => false,
    }
}

const PAKET_OPERATORS: [&str; 7] = [">=", "<=", "==", "~>", "@>", ">", "<"];

fn paket_operator_prefix(token: &str) -> Option<&str> {
    PAKET_OPERATORS
        .iter()
        .find(|op| token.starts_with(**op))
        .copied()
        .or_else(|| token.starts_with('=').then_some("="))
}

fn is_paket_version(token: &str) -> bool {
    token.starts_with(|c: char| c.is_ascii_digit())
        && token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_' | '*'))
}

fn parse_paket_declaration(line: &str) -> Result<PaketDeclaration<'_>, String> {
    let tokens = paket_tokens(line);
    let (id, rest) = match tokens.as_slice() {
        [(_, _, kind), (_, _, id), rest @ ..] if kind.eq_ignore_ascii_case("nuget") => (*id, rest),
        _ => return Err("not a nuget declaration".to_owned()),
    };
    let mut index = 0usize;
    let mut first = None;
    let mut last_end = 0usize;
    let mut expect_version = false;
    while let Some(&(from, to, token)) = rest.get(index) {
        let operand = match paket_operator_prefix(token) {
            Some(op) => {
                let operand = &token[op.len()..];
                if operand.is_empty() {
                    if expect_version {
                        return Err("comparison operator without a version".to_owned());
                    }
                    expect_version = true;
                    None
                } else {
                    Some(operand)
                }
            }
            None if expect_version || is_paket_version(token) => Some(token),
            None => break,
        };
        if let Some(operand) = operand {
            if !is_paket_version(operand) {
                return Err(format!("malformed version requirement {token:?}"));
            }
            expect_version = false;
        }
        first.get_or_insert(from);
        last_end = to;
        index += 1;
    }
    if expect_version {
        return Err("comparison operator without a version".to_owned());
    }
    let Some(first) = first else {
        return Err("no version requirement to replace".to_owned());
    };
    for &(_, _, token) in &rest[index..] {
        if token.contains(':') {
            break;
        }
        if !token.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(format!("unsupported token {token:?} after requirement"));
        }
    }
    Ok(PaketDeclaration {
        id,
        requirement: first..last_end,
    })
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
            let unsupported = |reason: String| BumpError::UnsupportedManifest {
                manifest: WORKFLOW_DIR.to_owned(),
                reason,
            };
            let parsed = crate::gha::parse_workflow_pins(content)
                .map_err(|error| unsupported(error.to_string()))?;
            let targets_package =
                |kind, value: &str| crate::gha::pin_targets_package(kind, value, package);
            if parsed.indirect
                && parsed
                    .loose
                    .iter()
                    .any(|value| targets_package(crate::gha::UsesKind::Step, value))
            {
                return Err(unsupported(format!(
                    "{package:?} sits in a workflow with aliases or merge keys"
                )));
            }
            let matched: Vec<usize> = parsed
                .pins
                .iter()
                .enumerate()
                .filter(|(_, pin)| targets_package(pin.kind, &pin.value))
                .map(|(index, _)| index)
                .collect();
            match matched.as_slice() {
                [] => Err(BumpError::NotFound {
                    manifest: WORKFLOW_DIR.to_owned(),
                    package: package.to_owned(),
                }),
                [index] => {
                    let pin = &parsed.pins[*index];
                    let no_pin = || unsupported(format!("{package:?} has no replaceable @SHA pin"));
                    let out =
                        crate::gha::replace_pin_reference(content, pin, sha).ok_or_else(no_pin)?;
                    let reparsed = crate::gha::parse_workflow_pins(&out)
                        .map_err(|error| unsupported(error.to_string()))?;
                    let (action, _) = crate::gha::split_pin(&pin.value).ok_or_else(no_pin)?;
                    let expected = format!("{action}@{sha}");
                    let kept = reparsed.pins.len() == parsed.pins.len()
                        && reparsed.pins.iter().zip(&parsed.pins).enumerate().all(
                            |(at, (after, before))| {
                                if at == *index {
                                    after.value == expected
                                } else {
                                    after.value == before.value
                                }
                            },
                        );
                    if !kept {
                        return Err(unsupported("edit did not round-trip".to_owned()));
                    }
                    Ok(out)
                }
                many => Err(BumpError::Ambiguous {
                    manifest: WORKFLOW_DIR.to_owned(),
                    package: package.to_owned(),
                    count: many.len(),
                }),
            }
        }
        WidenVersion::Semver(_) => Err(BumpError::UnsupportedManifest {
            manifest: WORKFLOW_DIR.to_owned(),
            reason: "github-actions pins are tag/SHA-shaped, not semver".to_owned(),
        }),
    }
}

pub(super) fn is_target_shape(text: &str) -> bool {
    if text.starts_with("//") || text.starts_with('@') {
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
