//! Validates the evidence workflow dispatch inputs.

use serde_json::{Value, json};

pub const PLATFORMS: [(&str, &str); 5] = [
    ("linux_x86_64", "ubuntu-latest"),
    ("linux_arm64", "ubuntu-24.04-arm"),
    ("macos_arm64", "macos-15"),
    ("windows_x86_64", "windows-latest"),
    ("windows_arm64", "windows-11-arm"),
];

const TASKS: [&str; 3] = ["test", "build", "coverage"];

const DISPLAYS: [&str; 4] = ["xcb", "wayland", "xcb,wayland", "wayland,xcb"];

pub struct Validated {
    pub outputs: Vec<(String, String)>,
    pub summary: String,
}

fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.+@=,/-".contains(c)
}

fn is_pattern(text: &str) -> bool {
    let rest = match text.strip_prefix('@') {
        Some(external) => match external.split_once("//") {
            Some((repo, rest)) if repo.starts_with("rules_dx_") && repo.chars().all(name_char) => {
                rest
            }
            _ => return false,
        },
        None => match text.strip_prefix("//") {
            Some(rest) => rest,
            None => return false,
        },
    };
    let (path, target) = match rest.split_once(':') {
        Some((path, target)) => (path, Some(target)),
        None => (rest, None),
    };
    path.chars().all(name_char)
        && target.is_none_or(|t| t == "*" || (!t.is_empty() && t.chars().all(name_char)))
}

fn is_flag(text: &str) -> bool {
    if let Some(name) = text.strip_prefix("--config=") {
        return !name.is_empty() && name.chars().all(name_char);
    }
    match text
        .strip_prefix("--//")
        .and_then(|rest| rest.split_once('='))
    {
        Some((label, value)) => {
            !label.is_empty()
                && label.chars().all(|c| name_char(c) || c == ':')
                && !value.is_empty()
                && value.chars().all(name_char)
        }
        None => false,
    }
}

fn json_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn quoted(items: &[String]) -> String {
    let items: Vec<String> = items.iter().map(|item| format!("'{item}'")).collect();
    format!("[{}]", items.join(", "))
}

/// Returns the workflow outputs for valid inputs, or the reason one input is rejected.
pub fn validate(get: impl Fn(&str) -> String) -> Result<Validated, String> {
    let scope_text = get("DX_SCOPE");
    let patterns: Vec<&str> = scope_text.split_whitespace().collect();
    if patterns.is_empty() {
        return Err("scope must name at least one target pattern".into());
    }
    for pattern in &patterns {
        if !is_pattern(pattern) {
            return Err(format!(
                "scope holds '{pattern}', which is not a target pattern; \
                 write patterns such as //pkg/... or //pkg:target"
            ));
        }
    }
    let scope = patterns.join(" ");
    let task = get("DX_TASK");
    if !TASKS.contains(&task.as_str()) {
        let tasks: Vec<String> = TASKS.iter().map(|t| t.to_string()).collect();
        return Err(format!(
            "task must be one of {}, got '{task}'",
            quoted(&tasks)
        ));
    }
    let Ok(parsed) = serde_json::from_str::<Value>(&get("DX_PLATFORMS")) else {
        return Err("platforms must be a JSON array".into());
    };
    let Value::Array(list) = &parsed else {
        return Err(format!(
            "platforms must be a JSON array, got a JSON {}",
            json_type(&parsed)
        ));
    };
    if list.is_empty() {
        return Err("platforms must be a non-empty JSON array".into());
    }
    let mut platforms = Vec::new();
    for item in list {
        match item.as_str() {
            Some(name) => platforms.push(name.to_string()),
            None => {
                return Err(format!(
                    "platforms must hold strings, got a JSON {}",
                    json_type(item)
                ));
            }
        }
    }
    let known: Vec<String> = PLATFORMS.iter().map(|(name, _)| name.to_string()).collect();
    let unknown: Vec<String> = platforms
        .iter()
        .filter(|name| !known.contains(name))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "unknown platforms {}; accepted {}",
            quoted(&unknown),
            quoted(&known)
        ));
    }
    for (index, name) in platforms.iter().enumerate() {
        if platforms[..index].contains(name) {
            return Err(format!(
                "platforms lists '{name}' twice; list each platform once"
            ));
        }
    }
    let displays = get("DX_DISPLAYS");
    if !displays.is_empty() && !DISPLAYS.contains(&displays.as_str()) {
        let shown: Vec<String> = DISPLAYS.iter().map(|d| d.to_string()).collect();
        return Err(format!(
            "displays must be one of {}, got '{displays}'",
            quoted(&shown)
        ));
    }
    let timeout = get("DX_TEST_TIMEOUT");
    if timeout.is_empty() || !timeout.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!(
            "test_timeout must be whole seconds, got '{timeout}'"
        ));
    }
    let flags_text = get("DX_FLAGS");
    let flag_list: Vec<&str> = flags_text.split_whitespace().collect();
    for flag in &flag_list {
        if !is_flag(flag) {
            return Err(format!(
                "flags holds '{flag}'; only --//label=value and --config=name are accepted"
            ));
        }
    }
    let flags = flag_list.join(" ");
    let relocate = get("DX_RELOCATE");
    if !["", "true", "false"].contains(&relocate.as_str()) {
        return Err(format!("relocate must be true or false, got '{relocate}'"));
    }
    if relocate == "true" && task == "coverage" {
        return Err("relocate supports test and build".to_string());
    }
    let relocate = if relocate == "true" { "true" } else { "false" };
    let runners: serde_json::Map<String, Value> = PLATFORMS
        .iter()
        .map(|(name, runner)| (name.to_string(), json!(runner)))
        .collect();
    let platforms_json = json!(platforms).to_string();
    Ok(Validated {
        outputs: vec![
            ("scope".into(), scope.clone()),
            ("task".into(), task.clone()),
            ("platforms".into(), platforms_json.clone()),
            ("runners".into(), Value::Object(runners).to_string()),
            ("test_timeout".into(), timeout),
            ("flags".into(), flags),
            ("relocate".into(), relocate.into()),
        ],
        summary: format!("evidence inputs accepted: {task} {scope} on {platforms_json}"),
    })
}
