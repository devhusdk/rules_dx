//! Validates the consumer workflow inputs and renders the job outputs the matrix reads.

use std::collections::BTreeMap;

pub const PLATFORMS_ENV: &str = "DX_PLATFORMS";
pub const DEPLOYABLES_ENV: &str = "DX_DEPLOYABLES";
pub const DISABLED_ENV: &str = "DX_DISABLED_CHECKS";
pub const MIN_COVERAGE_ENV: &str = "DX_MIN_COVERAGE";
pub const MIN_CHANGED_ENV: &str = "DX_MIN_CHANGED_LINE_COVERAGE";
pub const INVOCATION_ENV: &str = "DX_INVOCATION_ID";

pub const PLATFORMS: [(&str, &str); 5] = [
    ("linux_x86_64", "ubuntu-latest"),
    ("linux_arm64", "ubuntu-24.04-arm"),
    ("macos_arm64", "macos-15"),
    ("windows_x86_64", "windows-latest"),
    ("windows_arm64", "windows-11-arm"),
];

pub const DEFAULT_PLATFORM: &str = "linux_x86_64";

pub const CHECKS: [&str; 9] = [
    "lint",
    "typecheck",
    "format",
    "generate",
    "security-audit",
    "license-audit",
    "test",
    "build",
    "coverage",
];

pub struct MatrixCheck {
    pub id: &'static str,
    pub args: &'static [&'static str],
    pub reports: &'static [&'static str],
    pub passthrough: bool,
    pub timeout: u32,
}

pub const MATRIX_CHECKS: [MatrixCheck; 7] = [
    MatrixCheck {
        id: "lint",
        args: &["lint"],
        reports: &["sarif=lint.sarif"],
        passthrough: true,
        timeout: 30,
    },
    MatrixCheck {
        id: "typecheck",
        args: &["typecheck"],
        reports: &["sarif=typecheck.sarif"],
        passthrough: true,
        timeout: 30,
    },
    MatrixCheck {
        id: "format",
        args: &["format"],
        reports: &[],
        passthrough: true,
        timeout: 30,
    },
    MatrixCheck {
        id: "generate",
        args: &["generate"],
        reports: &[],
        passthrough: true,
        timeout: 30,
    },
    MatrixCheck {
        id: "license-audit",
        args: &["license"],
        reports: &[],
        passthrough: false,
        timeout: 30,
    },
    MatrixCheck {
        id: "test",
        args: &["test", "--profile"],
        reports: &[
            "junit=junit.xml",
            "failures=failures.json",
            "results=results.json",
        ],
        passthrough: true,
        timeout: 60,
    },
    MatrixCheck {
        id: "build",
        args: &["build", "--profile"],
        reports: &["failures=failures.json"],
        passthrough: true,
        timeout: 60,
    },
];

pub const CACHE_SAVERS: [&str; 1] = ["checks"];
const COVERAGE_TIMEOUT: u32 = 120;
const MAX_TIMEOUT: u32 = 360;

const SNAPSHOT_ARTIFACT: &str = "advisory-snapshots";

fn check_line(check: &MatrixCheck) -> String {
    format!(
        "{}|{}|{}|{}",
        check.id,
        check.args.join(" "),
        check.reports.join(" "),
        check.passthrough
    )
}

pub struct Validated {
    pub outputs: BTreeMap<String, String>,
    pub summary: Vec<String>,
}

fn accepted(values: &[&str]) -> String {
    values.join(", ")
}

fn platform_names() -> Vec<&'static str> {
    PLATFORMS.iter().map(|(name, _)| *name).collect()
}

fn platform_hint() -> String {
    format!(
        "Accepted: {}. Omit it to run {DEFAULT_PLATFORM}.",
        accepted(&platform_names())
    )
}

enum Json {
    Null,
    Bool,
    Number,
    Str(String),
    Array(Vec<Json>),
    Object,
}

impl Json {
    fn type_name(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool => "boolean",
            Json::Number => "number",
            Json::Str(_) => "string",
            Json::Array(_) => "array",
            Json::Object => "object",
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn blanks(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at position {}", self.at))
    }

    fn eat(&mut self, byte: u8) -> Result<(), String> {
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            self.fail(&format!("expected '{}'", byte as char))
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            self.fail("unexpected value")
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.blanks();
        match self.bytes.get(self.at) {
            None => self.fail("unexpected end of input"),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.word("true", Json::Bool),
            Some(b'f') => self.word("false", Json::Bool),
            Some(b'n') => self.word("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => self.fail("unexpected character"),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        while matches!(
            self.bytes.get(self.at),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.at += 1;
        }
        let text = String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned();
        if text.parse::<f64>().is_ok() {
            Ok(Json::Number)
        } else {
            self.fail("invalid number")
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            let Some(&byte) = self.bytes.get(self.at) else {
                return self.fail("unterminated string");
            };
            self.at += 1;
            match byte {
                b'"' => return Ok(String::from_utf8_lossy(&out).into_owned()),
                b'\\' => {
                    let Some(&escape) = self.bytes.get(self.at) else {
                        return self.fail("unterminated string");
                    };
                    self.at += 1;
                    match escape {
                        b'"' | b'\\' | b'/' => out.push(escape),
                        b'b' => out.push(8),
                        b'f' => out.push(12),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let hex = self.bytes.get(self.at..self.at + 4).unwrap_or_default();
                            let code = std::str::from_utf8(hex)
                                .ok()
                                .and_then(|text| u32::from_str_radix(text, 16).ok());
                            let Some(code) = code else {
                                return self.fail("invalid unicode escape");
                            };
                            self.at += 4;
                            let ch = char::from_u32(code).unwrap_or('\u{fffd}');
                            let mut buffer = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
                        }
                        _ => return self.fail("invalid escape"),
                    }
                }
                0..=31 => return self.fail("invalid control character"),
                _ => out.push(byte),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.blanks();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.blanks();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return self.fail("expected ',' or ']'"),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.eat(b'{')?;
        self.blanks();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(Json::Object);
        }
        loop {
            self.blanks();
            self.string()?;
            self.blanks();
            self.eat(b':')?;
            self.value()?;
            self.blanks();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object);
                }
                _ => return self.fail("expected ',' or '}'"),
            }
        }
    }
}

fn parse_json(text: &str) -> Result<Json, String> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        at: 0,
    };
    let value = parser.value()?;
    parser.blanks();
    if parser.at != parser.bytes.len() {
        return parser.fail("extra data");
    }
    Ok(value)
}

pub fn validate_platforms(text: &str) -> Result<Vec<String>, String> {
    let hint = platform_hint();
    if text.is_empty() {
        return Ok(vec![DEFAULT_PLATFORM.to_string()]);
    }
    let parsed = parse_json(text)
        .map_err(|error| format!("{PLATFORMS_ENV} is not a JSON array: {error}. {hint}"))?;
    let Json::Array(items) = parsed else {
        return Err(format!(
            "{PLATFORMS_ENV} is a JSON {}, not an array. {hint}",
            parsed.type_name()
        ));
    };
    if items.is_empty() {
        return Err(format!("{PLATFORMS_ENV} is an empty array. {hint}"));
    }
    let mut labels = Vec::new();
    for item in &items {
        match item {
            Json::Str(label) => labels.push(label.clone()),
            other => {
                return Err(format!(
                    "{PLATFORMS_ENV} holds a JSON {}, not a platform label string. {hint}",
                    other.type_name()
                ))
            }
        }
    }
    let names = platform_names();
    for label in &labels {
        if !names.contains(&label.as_str()) {
            return Err(format!(
                "{PLATFORMS_ENV} names unknown platform '{label}'. Accepted: {}.",
                accepted(&names)
            ));
        }
    }
    for (index, label) in labels.iter().enumerate() {
        if labels[..index].contains(label) {
            return Err(format!(
                "{PLATFORMS_ENV} lists '{label}' twice. List every platform once."
            ));
        }
    }
    Ok(labels)
}

/// Validates the release workflow `deployables` input: empty, or a non-empty JSON array of Bazel labels.
pub fn validate_deployables(text: &str) -> Result<Vec<String>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let hint = "Pass a JSON array of labels such as [\"//pkg:npm\"], or leave it empty to publish every dx_deployment tagged release.";
    let parsed = parse_json(text)
        .map_err(|error| format!("{DEPLOYABLES_ENV} is not a JSON array: {error}. {hint}"))?;
    let Json::Array(items) = parsed else {
        return Err(format!(
            "{DEPLOYABLES_ENV} is a JSON {}, not an array. {hint}",
            parsed.type_name()
        ));
    };
    if items.is_empty() {
        return Err(format!("{DEPLOYABLES_ENV} is an empty array. {hint}"));
    }
    let mut labels: Vec<String> = Vec::new();
    for item in &items {
        let Json::Str(label) = item else {
            return Err(format!(
                "{DEPLOYABLES_ENV} holds a JSON {}, not a label string. {hint}",
                item.type_name()
            ));
        };
        let (repo, rest) = match label.find("//") {
            Some(at) => label.split_at(at),
            None => ("", label.as_str()),
        };
        let repo_ok = repo.is_empty()
            || repo
                .strip_prefix('@')
                .map(|name| name.trim_start_matches('@'))
                .is_some_and(|name| {
                    name.chars()
                        .all(|c| c.is_ascii_alphanumeric() || "_.~+-".contains(c))
                });
        let target_ok = rest
            .strip_prefix("//")
            .and_then(|r| r.split_once(':'))
            .is_some_and(|(package, name)| {
                !name.is_empty()
                    && package
                        .chars()
                        .chain(name.chars())
                        .all(|c| c.is_ascii_alphanumeric() || "_./+-".contains(c))
            });
        if !repo_ok || !target_ok {
            return Err(format!(
                "{DEPLOYABLES_ENV} holds '{label}', which is not a Bazel label like //pkg:name. {hint}"
            ));
        }
        if labels.contains(label) {
            return Err(format!(
                "{DEPLOYABLES_ENV} lists '{label}' twice. List every deployable once."
            ));
        }
        labels.push(label.clone());
    }
    Ok(labels)
}

pub fn validate_disabled(text: &str) -> Result<Vec<String>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut ids: Vec<String> = Vec::new();
    for part in text.split(',') {
        let name = part.trim();
        if name.is_empty() {
            return Err(format!(
                "{DISABLED_ENV} holds an empty check ID in '{text}'. Accepted: {}.",
                accepted(&CHECKS)
            ));
        }
        if !CHECKS.contains(&name) {
            return Err(format!(
                "{DISABLED_ENV} names unknown check '{name}'. Accepted: {}.",
                accepted(&CHECKS)
            ));
        }
        if ids.iter().any(|id| id == name) {
            return Err(format!(
                "{DISABLED_ENV} lists '{name}' twice. List every check ID once."
            ));
        }
        ids.push(name.to_string());
    }
    Ok(ids)
}

pub fn validate_percent(text: &str, name: &str, omitted: &str) -> Result<String, String> {
    if text.is_empty() {
        return Ok(String::new());
    }
    let whole = text.bytes().all(|byte| byte.is_ascii_digit());
    if !whole || text.parse::<u32>().map_or(true, |value| value > 100) {
        return Err(format!(
            "{name}='{text}' is not a whole percent from 0 to 100. \
             Write a whole number such as 80, or omit it to {omitted}."
        ));
    }
    Ok(text.to_string())
}

pub fn validate_invocation(text: &str) -> Result<String, String> {
    let valid = (1..=40).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
    if !text.is_empty() && !valid {
        return Err(format!(
            "{INVOCATION_ENV}='{text}' is not 1 to 40 letters, digits, underscores \
             or hyphens. Give every call in one parent run its own ID, or omit it \
             for a single call."
        ));
    }
    Ok(text.to_string())
}

fn quoted_list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("\"{item}\"")).collect();
    format!("[{}]", quoted.join(","))
}

fn render_check(check: &MatrixCheck) -> String {
    format!(
        "{{\"id\":\"{}\",\"args\":{},\"reports\":{},\"passthrough\":{},\"timeout\":{}}}",
        check.id,
        quoted_list(check.args),
        quoted_list(check.reports),
        check.passthrough,
        check.timeout
    )
}

fn or_none(text: &str) -> &str {
    if text.is_empty() {
        "(none)"
    } else {
        text
    }
}

/// Validates every input read through `get` and returns the job outputs, or every diagnostic.
pub fn validate(get: impl Fn(&str) -> String) -> Result<Validated, Vec<String>> {
    let platforms = validate_platforms(&get(PLATFORMS_ENV));
    let disabled = validate_disabled(&get(DISABLED_ENV));
    let coverage = validate_percent(
        &get(MIN_COVERAGE_ENV),
        MIN_COVERAGE_ENV,
        "collect without a threshold",
    );
    let changed = validate_percent(
        &get(MIN_CHANGED_ENV),
        MIN_CHANGED_ENV,
        "skip the changed-line check",
    );
    let invocation = validate_invocation(&get(INVOCATION_ENV));
    let (Ok(platforms), Ok(disabled), Ok(coverage), Ok(changed), Ok(invocation)) =
        (&platforms, &disabled, &coverage, &changed, &invocation)
    else {
        let errors = [
            platforms.err(),
            disabled.err(),
            coverage.err(),
            changed.err(),
            invocation.err(),
        ];
        return Err(errors.into_iter().flatten().collect());
    };
    let enabled: Vec<&MatrixCheck> = MATRIX_CHECKS
        .iter()
        .filter(|check| !disabled.iter().any(|id| id == check.id))
        .collect();
    let runners: Vec<String> = PLATFORMS
        .iter()
        .map(|(name, runner)| format!("\"{name}\":\"{runner}\""))
        .collect();
    let platform_list: Vec<&str> = platforms.iter().map(String::as_str).collect();
    let coverage_on = !disabled.iter().any(|id| id == "coverage");
    let mut timeout: u32 = enabled.iter().map(|check| check.timeout).sum();
    if coverage_on {
        timeout += COVERAGE_TIMEOUT;
    }
    let cache_saver = if !enabled.is_empty() || coverage_on {
        CACHE_SAVERS[0]
    } else {
        ""
    };
    let check_lines = enabled
        .iter()
        .map(|check| check_line(check))
        .collect::<Vec<_>>()
        .join("\n");
    let suffix = if invocation.is_empty() {
        String::new()
    } else {
        format!("-{invocation}")
    };
    let mut outputs = BTreeMap::new();
    outputs.insert("platforms".to_string(), quoted_list(&platform_list));
    outputs.insert("runners".to_string(), format!("{{{}}}", runners.join(",")));
    outputs.insert("disabled".to_string(), disabled.join(","));
    outputs.insert(
        "checks".to_string(),
        format!(
            "[{}]",
            enabled
                .iter()
                .map(|check| render_check(check))
                .collect::<Vec<_>>()
                .join(",")
        ),
    );
    outputs.insert("check_lines".to_string(), check_lines);
    outputs.insert(
        "checks_timeout".to_string(),
        timeout.min(MAX_TIMEOUT).to_string(),
    );
    outputs.insert("cache_saver".to_string(), cache_saver.to_string());
    outputs.insert("min_coverage".to_string(), coverage.clone());
    outputs.insert("min_changed_line_coverage".to_string(), changed.clone());
    outputs.insert("artifact_suffix".to_string(), suffix.clone());
    outputs.insert(
        "snapshot_artifact".to_string(),
        format!("{SNAPSHOT_ARTIFACT}{suffix}"),
    );
    let ids: Vec<&str> = enabled.iter().map(|check| check.id).collect();
    let summary = vec![
        format!("{PLATFORMS_ENV} {}", outputs["platforms"]),
        format!("{DISABLED_ENV} {}", or_none(&outputs["disabled"])),
        format!("checks {}", or_none(&ids.join(","))),
        format!("{MIN_COVERAGE_ENV} {}", or_none(coverage)),
        format!("{MIN_CHANGED_ENV} {}", or_none(changed)),
        format!("{INVOCATION_ENV} {}", or_none(invocation)),
    ];
    Ok(Validated { outputs, summary })
}
