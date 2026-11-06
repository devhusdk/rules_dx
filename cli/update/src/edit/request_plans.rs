use super::*;

pub(super) fn plan_bazelversion(
    content: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(EditError::UnsupportedManifest {
                manifest: ".bazelversion".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return Err(EditError::UnsupportedManifest {
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
/// A tested update edits the `single_version_override` version when one has a literal
/// version, else the `bazel_dep`; it refuses to drop below the `bazel_dep` floor. A floor
/// update edits the `bazel_dep` and its `# floor: ` comment; it refuses to rise above the
/// override's tested version.
pub(super) fn plan_module_bazel(
    content: &str,
    package: &str,
    version: &WidenVersion,
    target: &DepVersion,
) -> Result<String, EditError> {
    use dx_module_bazel::ModuleError;
    use std::cmp::Ordering;
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(EditError::UnsupportedManifest {
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
                return Err(EditError::TestedBelowFloor {
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
                    return Err(EditError::FloorAboveTested {
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

/// Sets the `digest` of the `oci.pull` named `package` in MODULE.bazel.
pub(super) fn plan_oci_pull(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Digest(digest) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: "MODULE.bazel".to_owned(),
            reason: "expected a sha256 digest".to_owned(),
        });
    };
    let module = dx_module_bazel::ModuleFile::parse(content).map_err(module_error(package))?;
    let pulls: Vec<_> = module
        .calls_named("oci.pull")
        .filter(|call| call.str("name") == Some(package))
        .collect();
    let call = match pulls.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: "MODULE.bazel".to_owned(),
                package: package.to_owned(),
            })
        }
        [call] => call,
        many => {
            return Err(EditError::Ambiguous {
                manifest: "MODULE.bazel".to_owned(),
                package: package.to_owned(),
                count: many.len(),
            })
        }
    };
    let Some(dx_module_bazel::Value::Str(literal)) = call.arg("digest") else {
        return Err(EditError::UnsupportedManifest {
            manifest: "MODULE.bazel".to_owned(),
            reason: format!("oci.pull {package} has no literal digest"),
        });
    };
    module
        .replace(literal, digest)
        .map_err(module_error(package))
}

fn module_error(package: &str) -> impl Fn(dx_module_bazel::ModuleError) -> EditError + '_ {
    move |error| match error {
        dx_module_bazel::ModuleError::NotFound { .. } => EditError::NotFound {
            manifest: "MODULE.bazel".to_owned(),
            package: package.to_owned(),
        },
        dx_module_bazel::ModuleError::Ambiguous { count, .. } => EditError::Ambiguous {
            manifest: "MODULE.bazel".to_owned(),
            package: package.to_owned(),
            count,
        },
        other => EditError::UnsupportedManifest {
            manifest: "MODULE.bazel".to_owned(),
            reason: other.to_string(),
        },
    }
}

pub(super) fn plan_package_json(
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(EditError::UnsupportedManifest {
                manifest: "package.json".to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let unsupported = |reason: String| EditError::UnsupportedManifest {
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
        0 => Err(EditError::NotFound {
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
        count => Err(EditError::Ambiguous {
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
    fn bad() -> EditError {
        EditError::UnsupportedManifest {
            manifest: "package.json".to_owned(),
            reason: "manifest is not valid JSON".to_owned(),
        }
    }

    pub(crate) fn skip_ws(&mut self) {
        while self.text[self.at..].starts_with([' ', '\t', '\n', '\r']) {
            self.at += 1;
        }
    }

    pub(crate) fn string_end(&mut self) -> Result<usize, EditError> {
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

    pub(crate) fn skip_value(&mut self) -> Result<(), EditError> {
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

    pub(crate) fn members(&mut self) -> Result<Vec<(String, usize)>, EditError> {
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
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let new = match version {
        WidenVersion::Semver(version) => format!("v{version}"),
        _ => {
            return Err(EditError::UnsupportedManifest {
                manifest: manifest.to_owned(),
                reason: "expected exact semver".to_owned(),
            });
        }
    };
    let manifest = manifest.to_owned();
    dx_gomod::set_require_version(content, package, &new).map_err(|error| match error {
        dx_gomod::GoEditError::NotFound => EditError::NotFound {
            manifest,
            package: package.to_owned(),
        },
        dx_gomod::GoEditError::Ambiguous(count) => EditError::Ambiguous {
            manifest,
            package: package.to_owned(),
            count,
        },
        dx_gomod::GoEditError::Invalid(reason) => {
            EditError::UnsupportedManifest { manifest, reason }
        }
    })
}

/// Rewrites the version on the one `gem "<package>"` line of a Gemfile, keeping its operator.
///
/// Only a single literal requirement is rewritten; other lines are returned untouched.
pub(super) fn plan_gemfile(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Gem(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected a gem version".to_owned(),
        });
    };
    let unsupported = |reason: &str| EditError::UnsupportedManifest {
        manifest: manifest.to_owned(),
        reason: reason.to_owned(),
    };
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let hits: Vec<usize> = (0..lines.len())
        .filter(|&i| {
            literals(lines[i], "gem ")
                .first()
                .is_some_and(|l| l.text == package)
        })
        .collect();
    let index = match hits.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
            });
        }
        [index] => *index,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let line = lines[index];
    let requirements: Vec<Literal<'_>> = literals(line, "gem ")
        .into_iter()
        .skip(1)
        .take_while(|l| {
            l.text.starts_with(['>', '<', '=', '~'])
                || l.text.starts_with(|c: char| c.is_ascii_digit())
        })
        .collect();
    let [requirement] = requirements.as_slice() else {
        return Err(unsupported(
            "the gem line needs exactly one literal version requirement",
        ));
    };
    let number = requirement
        .text
        .trim_start_matches(['>', '<', '=', '~', ' ']);
    let operator = &requirement.text[..requirement.text.len() - number.len()];
    let mut edited = lines.concat();
    let offset: usize = lines[..index].iter().map(|l| l.len()).sum();
    edited.replace_range(
        offset + requirement.start..offset + requirement.end,
        &format!("{operator}{new}"),
    );
    Ok(edited)
}

/// Rewrites the constraint of a package in the `require` or `require-dev` of a composer.json.
///
/// The operator stays; only a single constraint such as `^1.2` or `~1.2.3` is rewritten.
pub(super) fn plan_composer_json(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Gem(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected a composer version".to_owned(),
        });
    };
    let unsupported = |reason: &str| EditError::UnsupportedManifest {
        manifest: manifest.to_owned(),
        reason: reason.to_owned(),
    };
    let json: serde_json::Value =
        serde_json::from_str(content).map_err(|error| unsupported(&error.to_string()))?;
    let sections: Vec<&str> = ["require", "require-dev"]
        .into_iter()
        .filter(|section| json[*section].get(package).is_some())
        .collect();
    let section = match sections.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
            });
        }
        [section] => *section,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let constraint = json[section][package]
        .as_str()
        .ok_or_else(|| unsupported("the constraint is not a string"))?;
    let number = constraint.trim_start_matches(['^', '~', '>', '<', '=', ' ']);
    let literal = number.strip_prefix('v').unwrap_or(number);
    let single = !literal.is_empty()
        && literal
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
    if !single {
        return Err(unsupported(
            "the constraint needs exactly one literal version",
        ));
    }
    let operator = &constraint[..constraint.len() - number.len()];
    let key_start = content
        .find(&format!("\"{section}\""))
        .and_then(|at| {
            let rest = &content[at..];
            let open = at + rest.find('{')?;
            Some(open)
        })
        .ok_or_else(|| unsupported("cannot find the section"))?;
    let section_end = content[key_start..]
        .find('}')
        .map_or(content.len(), |end| key_start + end);
    let key = format!("\"{package}\"");
    let found = content[key_start..section_end]
        .find(&key)
        .ok_or_else(|| unsupported("cannot find the package in its section"))?;
    let after_key = key_start + found + key.len();
    let value_start = after_key
        + content[after_key..section_end]
            .find('"')
            .ok_or_else(|| unsupported("cannot find the constraint"))?
        + 1;
    let value_end = value_start + constraint.len();
    if content.get(value_start..value_end) != Some(constraint) {
        return Err(unsupported("the constraint has escapes"));
    }
    let mut edited = content.to_owned();
    edited.replace_range(value_start..value_end, &format!("{operator}{new}"));
    Ok(edited)
}

struct Literal<'a> {
    text: &'a str,
    /// Byte range of the text inside the quotes, relative to the line.
    start: usize,
    end: usize,
}

/// The leading quoted strings of a line that starts with `keyword`, in order; empty when it is not one.
fn literals<'a>(line: &'a str, keyword: &str) -> Vec<Literal<'a>> {
    let Some(rest) = line.trim_start().strip_prefix(keyword) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut cursor = line.len() - rest.len();
    loop {
        let after = line[cursor..].trim_start_matches([' ', ',', '=', '>']);
        let start = line.len() - after.len();
        let Some(quote) = after.chars().next().filter(|c| matches!(c, '"' | '\'')) else {
            return found;
        };
        let Some(close) = after[1..].find(quote) else {
            return found;
        };
        found.push(Literal {
            text: &after[1..1 + close],
            start: start + 1,
            end: start + 1 + close,
        });
        cursor = start + close + 2;
    }
}

/// Rewrites the version of the one `requires '<Module>', '<op> <version>'` line of a cpanfile.
pub(super) fn plan_cpanfile(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Cpan(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected a cpan version".to_owned(),
        });
    };
    let lines: Vec<&str> = content.split_inclusive('\n').collect();
    let hits: Vec<usize> = (0..lines.len())
        .filter(|&i| {
            literals(lines[i], "requires ")
                .first()
                .is_some_and(|l| l.text == package)
        })
        .collect();
    let index = match hits.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
            });
        }
        [index] => *index,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let found = literals(lines[index], "requires ");
    let requirement = match found.as_slice() {
        [_, requirement] if requirement.text.contains(|c: char| c.is_ascii_digit()) => requirement,
        _ => {
            return Err(EditError::UnsupportedManifest {
                manifest: manifest.to_owned(),
                reason: "the requires line needs exactly one literal version requirement"
                    .to_owned(),
            });
        }
    };
    let number = requirement
        .text
        .trim_start_matches(['>', '<', '=', '!', ' ']);
    let operator = &requirement.text[..requirement.text.len() - number.len()];
    let mut edited = lines.concat();
    let offset: usize = lines[..index].iter().map(|l| l.len()).sum();
    edited.replace_range(
        offset + requirement.start..offset + requirement.end,
        &format!("{operator}{new}"),
    );
    Ok(edited)
}

/// Rewrites the bound of one package in the `build-depends` fields of a `.cabal` file.
///
/// A single `^>=`, `>=` or `==` bound takes the new version; a `>=L && <U` range keeps its
/// lower bound and gets the upper bound of the new major.
pub(super) fn plan_cabal(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Gem(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected a cabal version".to_owned(),
        });
    };
    let unsupported = |reason: &str| EditError::UnsupportedManifest {
        manifest: manifest.to_owned(),
        reason: reason.to_owned(),
    };
    let hits: Vec<_> = crate::discover::cabal::build_depends(content)
        .into_iter()
        .filter(|depend| depend.name == package)
        .collect();
    let depend = match hits.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
            });
        }
        [depend] => depend,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let bound = &content[depend.bound.clone()];
    let parts: Vec<&str> = bound.split("&&").map(str::trim).collect();
    let replaced = match parts.as_slice() {
        [single] => single_bound(single, new),
        [lower, upper]
            if lower.starts_with(">=") && upper.starts_with('<') && !upper.starts_with("<=") =>
        {
            let major = next_major(new)
                .ok_or_else(|| unsupported("the new version is not dotted numbers"))?;
            Some(format!("{lower} && <{major}"))
        }
        _ => None,
    };
    let Some(replaced) = replaced else {
        return Err(unsupported(
            "the bound needs to be one of ^>=X, >=X, ==X, ==X.* or >=X && <Y",
        ));
    };
    let mut edited = content.to_owned();
    edited.replace_range(depend.bound.clone(), &replaced);
    Ok(edited)
}

fn single_bound(bound: &str, new: &str) -> Option<String> {
    let number_at = bound.find(|c: char| c.is_ascii_digit())?;
    let (operator, number) = bound.split_at(number_at);
    if !matches!(operator, "^>=" | ">=" | "==") {
        return None;
    }
    match number.strip_suffix(".*") {
        Some(prefix) if operator == "==" => {
            let keep = prefix.split('.').count();
            let cut: Vec<&str> = new.split('.').take(keep).collect();
            Some(format!("=={}.*", cut.join(".")))
        }
        Some(_) => None,
        None if number.chars().all(|c| c.is_ascii_digit() || c == '.') => {
            Some(format!("{operator}{new}"))
        }
        None => None,
    }
}

fn next_major(version: &str) -> Option<String> {
    let mut parts = version.split('.').map(|part| part.parse::<u64>().ok());
    let first = parts.next()??;
    Some(match parts.next() {
        Some(second) => format!("{first}.{}", second? + 1),
        None => format!("{}", first + 1),
    })
}

/// Rewrites the constraint of one scalar `package: <constraint>` entry in the
/// `dependencies` or `dev_dependencies` of a pubspec.yaml, keeping its `^` or range form.
pub(super) fn plan_pubspec(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Semver(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected exact semver".to_owned(),
        });
    };
    let unsupported = |reason: &str| EditError::UnsupportedManifest {
        manifest: manifest.to_owned(),
        reason: reason.to_owned(),
    };
    let mut section = "";
    let mut offset = 0;
    let mut hits = Vec::new();
    for line in content.split_inclusive('\n') {
        let text = line.trim_end_matches(['\n', '\r']);
        let trimmed = text.trim_start();
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            if text.len() == trimmed.len() {
                section = trimmed.split(':').next().unwrap_or("").trim();
            } else if matches!(section, "dependencies" | "dev_dependencies") {
                let entry = trimmed
                    .strip_prefix(package)
                    .and_then(|rest| rest.strip_prefix(':'));
                if let Some(value) = entry {
                    hits.push((offset + text.len() - value.len(), value));
                }
            }
        }
        offset += line.len();
    }
    let (at, value) = match hits.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
            });
        }
        [hit] => *hit,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let constraint = match value.find(" #") {
        Some(cut) => &value[..cut],
        None => value,
    };
    let lead = constraint.len() - constraint.trim_start().len();
    let body = constraint.trim();
    let (quote, inner) = match body.chars().next() {
        Some(q @ ('"' | '\'')) if body.len() >= 2 && body.ends_with(q) => {
            (q.to_string(), &body[1..body.len() - 1])
        }
        _ => (String::new(), body),
    };
    let replaced = if let Some(plain) = inner.strip_prefix('^') {
        plain
            .chars()
            .next()
            .filter(char::is_ascii_digit)
            .map(|_| format!("^{new}"))
    } else if inner.starts_with(|c: char| c.is_ascii_digit()) && !inner.contains(' ') {
        Some(new.to_string())
    } else if let Some(range) = inner.strip_prefix(">=") {
        let upper = range.split_once(' ').map_or("", |(_, upper)| upper);
        Some(if upper.is_empty() {
            format!(">={new}")
        } else {
            format!(">={new} {upper}")
        })
    } else {
        None
    };
    let Some(replaced) = replaced else {
        return Err(unsupported(
            "the dependency needs a scalar `^x.y.z`, `x.y.z` or `>=x.y.z <a.b.c` constraint",
        ));
    };
    let mut edited = content.to_owned();
    let start = at + lead;
    edited.replace_range(
        start..start + body.len(),
        &format!("{quote}{replaced}{quote}"),
    );
    Ok(edited)
}

/// Rewrites the `from:` or `exact:` version of the one `.package(url:, ...)` whose
/// identity (the last URL path segment without `.git`) is `package`.
pub(super) fn plan_package_swift(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
) -> Result<String, EditError> {
    let WidenVersion::Semver(new) = version else {
        return Err(EditError::UnsupportedManifest {
            manifest: manifest.to_owned(),
            reason: "expected exact semver".to_owned(),
        });
    };
    let unsupported = |reason: &str| EditError::UnsupportedManifest {
        manifest: manifest.to_owned(),
        reason: reason.to_owned(),
    };
    let mut hits = Vec::new();
    let mut from = 0;
    while let Some(found) = content[from..].find(".package(") {
        let start = from + found;
        let end = content[start..]
            .find(')')
            .map_or(content.len(), |n| start + n);
        from = end.max(start + 1);
        let call = &content[start..end];
        let Some(url) = swift_string_after(call, "url:") else {
            continue;
        };
        let identity = url
            .0
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".git")
            .to_ascii_lowercase();
        if identity == package.to_ascii_lowercase() {
            hits.push((start, call));
        }
    }
    let (start, call) = match hits.as_slice() {
        [] => {
            return Err(EditError::NotFound {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
            });
        }
        [hit] => *hit,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    if ["branch:", "revision:", "path:"]
        .iter()
        .any(|label| call.contains(label))
    {
        return Err(unsupported(
            "the package is pinned by branch, revision or path, not a version",
        ));
    }
    let Some((_, at, len)) = ["from:", "exact:"]
        .iter()
        .find_map(|label| swift_string_after(call, label))
    else {
        return Err(unsupported(
            "the package needs a `from:` or `exact:` version requirement",
        ));
    };
    let mut edited = content.to_owned();
    edited.replace_range(start + at..start + at + len, &new.to_string());
    Ok(edited)
}

/// The string literal after `label` in a call: its text, content offset and length.
fn swift_string_after<'a>(call: &'a str, label: &str) -> Option<(&'a str, usize, usize)> {
    let rest = call.find(label).map(|n| n + label.len())?;
    let after = &call[rest..];
    let open = rest + after.find('"')? + 1;
    if !after[..after.find('"')?].trim().is_empty() {
        return None;
    }
    let len = call[open..].find('"')?;
    Some((&call[open..open + len], open, len))
}

pub(super) fn plan_maven_module_bazel(
    manifest: &str,
    content: &str,
    package: &str,
    version: &WidenVersion,
    install: Option<&str>,
) -> Result<String, EditError> {
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(EditError::UnsupportedManifest {
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
            return Err(EditError::UnsupportedManifest {
                manifest: manifest.to_owned(),
                reason: format!("malformed Maven coordinate {:?}", literal.value),
            });
        }
        selected.push((literal, parts));
    }
    match selected.as_slice() {
        [] => Err(EditError::NotFound {
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
        many => Err(EditError::Ambiguous {
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
) -> Result<String, EditError> {
    let manifest_path = manifest;
    let new = match version {
        WidenVersion::Semver(version) => version.to_string(),
        _ => {
            return Err(EditError::UnsupportedManifest {
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
            return Err(EditError::NotFound {
                manifest: manifest_path.to_owned(),
                package: package.to_owned(),
            });
        }
        [one] => *one,
        many => {
            return Err(EditError::Ambiguous {
                manifest: manifest_path.to_owned(),
                package: package.to_owned(),
                count: many.len(),
            });
        }
    };
    let unsupported = |reason: String| EditError::UnsupportedManifest {
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
    tag: Option<&str>,
) -> Result<String, EditError> {
    match version {
        WidenVersion::GitTag(tag) => Err(EditError::NeedsSha {
            package: package.to_owned(),
            tag: tag.clone(),
        }),
        WidenVersion::GitCommit(sha) => {
            let unsupported = |reason: String| EditError::UnsupportedManifest {
                manifest: WORKFLOW_DIR.to_owned(),
                reason,
            };
            let parsed = crate::edit::gha::parse_workflow_pins(content)
                .map_err(|error| unsupported(error.to_string()))?;
            let targets_package =
                |kind, value: &str| crate::edit::gha::pin_targets_package(kind, value, package);
            if parsed.indirect
                && parsed
                    .loose
                    .iter()
                    .any(|value| targets_package(crate::edit::gha::UsesKind::Step, value))
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
                [] => Err(EditError::NotFound {
                    manifest: WORKFLOW_DIR.to_owned(),
                    package: package.to_owned(),
                }),
                [index] => {
                    let pin = &parsed.pins[*index];
                    let no_pin = || unsupported(format!("{package:?} has no replaceable @SHA pin"));
                    let renamed = match tag {
                        Some(tag) => crate::edit::gha::replace_version_comment(content, pin, tag),
                        None => None,
                    };
                    let base = renamed.as_deref().unwrap_or(content);
                    let out = crate::edit::gha::replace_pin_reference(base, pin, sha)
                        .ok_or_else(no_pin)?;
                    let reparsed = crate::edit::gha::parse_workflow_pins(&out)
                        .map_err(|error| unsupported(error.to_string()))?;
                    let (action, _) = crate::edit::gha::split_pin(&pin.value).ok_or_else(no_pin)?;
                    let expected = format!("{action}@{sha}");
                    let kept = reparsed.pins.len() == parsed.pins.len()
                        && reparsed.pins.iter().zip(&parsed.pins).enumerate().all(
                            |(at, (after, before))| {
                                if at == *index {
                                    after.value == expected
                                        && match (&renamed, tag) {
                                            (Some(_), Some(tag)) => after
                                                .comment
                                                .as_deref()
                                                .is_some_and(|c| c.starts_with(tag)),
                                            _ => after.comment == before.comment,
                                        }
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
                many => Err(EditError::Ambiguous {
                    manifest: WORKFLOW_DIR.to_owned(),
                    package: package.to_owned(),
                    count: many.len(),
                }),
            }
        }
        WidenVersion::Semver(_)
        | WidenVersion::Gem(_)
        | WidenVersion::Cpan(_)
        | WidenVersion::Opam(_)
        | WidenVersion::Digest(_) => Err(EditError::UnsupportedManifest {
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
        && UpdateSet::parse(text.split_once(':').map_or("", |(head, _)| head)).is_none()
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

pub(super) fn validate_package(set: UpdateSet, package: &str) -> Result<(), EditError> {
    let invalid = |reason: &'static str| EditError::InvalidPackage {
        set: set.name(),
        package: package.to_owned(),
        reason,
    };
    let reason = match set {
        UpdateSet::Bazel => {
            if package == ".bazelversion" {
                Ok(())
            } else {
                dx_module_bazel::identity::validate_dotted(package, BAZEL_CHARSET)
            }
        }
        UpdateSet::Cargo => dx_module_bazel::identity::validate_cargo(package),
        UpdateSet::Npm => dx_module_bazel::identity::validate_npm(package),
        UpdateSet::Go => dx_module_bazel::identity::validate_go(package),
        UpdateSet::Maven => dx_module_bazel::identity::validate_maven(package),
        UpdateSet::NuGet => dx_module_bazel::identity::validate_nuget(package),
        UpdateSet::Composer => {
            let part = |text: &str| {
                !text.is_empty()
                    && text
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            };
            match package.split_once('/') {
                Some((vendor, name)) if part(vendor) && part(name) => Ok(()),
                _ => Err("composer packages are vendor/name"),
            }
        }
        UpdateSet::Ruby => {
            if !package.is_empty()
                && package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                Ok(())
            } else {
                Err("gem names use letters, digits, '-', '_' and '.'")
            }
        }
        UpdateSet::Cpan => {
            if !package.is_empty()
                && package.split("::").all(|part| {
                    !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
            {
                Ok(())
            } else {
                Err("cpan module names are words joined by '::'")
            }
        }
        UpdateSet::Cabal => {
            if !package.is_empty()
                && package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-')
            {
                Ok(())
            } else {
                Err("cabal package names use letters, digits and '-'")
            }
        }
        UpdateSet::SwiftPm => {
            if !package.is_empty()
                && package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                Ok(())
            } else {
                Err("swiftpm package identities use letters, digits, '-', '_' and '.'")
            }
        }
        UpdateSet::Pub => {
            if !package.is_empty()
                && package.starts_with(|c: char| c.is_ascii_lowercase())
                && package
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                Ok(())
            } else {
                Err("pub package names use lowercase letters, digits and '_'")
            }
        }
        UpdateSet::Opam => {
            if !package.is_empty()
                && package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+'))
            {
                Ok(())
            } else {
                Err("opam package names use letters, digits, '-', '_' and '+'")
            }
        }
        UpdateSet::Containers => {
            if !package.is_empty()
                && package
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            {
                Ok(())
            } else {
                Err("oci.pull names use letters, digits, '-', '_' and '.'")
            }
        }
        UpdateSet::GithubActions => match package.split_once('/') {
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
                    dx_module_bazel::identity::validate_dotted_pair(
                        owner,
                        repo,
                        GITHUB_ACTIONS_CHARSET,
                    )
                }
            }
        },
    };
    reason.map_err(invalid)
}
