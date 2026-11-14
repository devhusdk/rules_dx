//! `pending.bzl` is sorted, unique, names only known languages, tests and platforms, and has no stale line.

use serde_json::Value;

const TESTS: [&str; 11] = [
    "build_test",
    "coverage_test",
    "cross_test",
    "fail_test",
    "format_error_test",
    "format_test",
    "lint_error_test",
    "lint_test",
    "lock_test",
    "pass_test",
    "runfiles_test",
];

fn read(var: &str) -> String {
    let key = std::env::var(var).unwrap_or_else(|_| panic!("{var} must be set"));
    let path = dx_testing::rlocation(&key).expect("file must resolve from runfiles");
    std::fs::read_to_string(path).expect("file must be readable")
}

fn problems(pending_body: &str, manifest: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let platforms = manifest["platforms"].as_object().expect("platforms map");
    let entries = manifest["entries"].as_array().expect("entries list");

    let lines: Vec<&str> = pending_body.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut sorted = lines.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if lines != sorted {
        out.push("pending.bzl lines must be sorted and unique".to_string());
    }

    for line in &lines {
        let fields: Vec<&str> = line.split(' ').collect();
        if fields.len() != 3 {
            out.push(format!("want '<lang> <test> <platform>': {line}"));
            continue;
        }
        let (lang, test, platform) = (fields[0], fields[1], fields[2]);
        let Some(known) = platforms.get(lang).and_then(Value::as_array) else {
            out.push(format!("unknown language in: {line}"));
            continue;
        };
        if !TESTS.contains(&test) {
            out.push(format!("unknown test in: {line}"));
            continue;
        }
        if platform != "*" && !known.iter().any(|p| p == platform) {
            out.push(format!("unknown platform in: {line}"));
            continue;
        }
        let pending = entries.iter().any(|e| {
            e["lang"] == lang
                && e["capability"] == test
                && e["status"] == "pending"
                && (platform == "*" || e["platform"] == platform)
        });
        if !pending {
            out.push(format!("stale line, no pending manifest row: {line}"));
        }
        if platform != "*" && lines.contains(&format!("{lang} {test} *").as_str()) {
            out.push(format!("redundant, shadowed by '*' line: {line}"));
        }
    }
    out
}

#[test]
fn pending_lines_are_valid() {
    let source = read("DX_CONFORMANCE_PENDING");
    let body = source.split("\"\"\"").nth(3).expect("PENDING string");
    let manifest: Value = serde_json::from_str(&read("DX_CONFORMANCE_MANIFEST")).unwrap();
    assert_eq!(problems(body, &manifest), Vec::<String>::new());
}

#[cfg(test)]
mod unit {
    use super::problems;
    use serde_json::json;

    fn manifest() -> serde_json::Value {
        json!({
            "platforms": {"go": ["linux_x86_64", "macos_arm64"], "qt": []},
            "entries": [
                {"lang": "go", "capability": "pass_test", "platform": "linux_x86_64", "status": "pending"},
                {"lang": "go", "capability": "pass_test", "platform": "macos_arm64", "status": "run"},
                {"lang": "go", "capability": "coverage_test", "platform": "linux_x86_64", "status": "pending"},
                {"lang": "go", "capability": "coverage_test", "platform": "macos_arm64", "status": "pending"},
                {"lang": "go", "capability": "fail_test", "platform": "linux_x86_64", "status": "n/a"},
                {"lang": "go", "capability": "fail_test", "platform": "macos_arm64", "status": "n/a"},
                {"lang": "qt", "capability": "build_test", "platform": "*", "status": "pending"},
            ],
        })
    }

    fn bad(body: &str) -> bool {
        !problems(body, &manifest()).is_empty()
    }

    #[test]
    fn valid_lines_pass() {
        assert_eq!(problems("\ngo pass_test linux_x86_64\n", &manifest()).len(), 0);
    }

    #[test]
    fn coverage_test_line_passes() {
        assert_eq!(problems("\ngo coverage_test *\n", &manifest()).len(), 0);
    }

    #[test]
    fn star_line_for_empty_platform_language_passes() {
        assert_eq!(problems("\nqt build_test *\n", &manifest()).len(), 0);
    }

    #[test]
    fn invalid_lines_fail() {
        assert!(bad("\ngo pass_test macos_arm64\ngo pass_test linux_x86_64\n"));
        assert!(bad("\ngo pass_test linux_x86_64\ngo pass_test linux_x86_64\n"));
        assert!(bad("\nzig pass_test *\n"));
        assert!(bad("\ngo run_test *\n"));
        assert!(bad("\ngo pass_test windows_x86_64\n"));
        assert!(bad("\ngo fail_test linux_x86_64\n"));
        assert!(bad("\ngo pass_test *\ngo pass_test linux_x86_64\n"));
    }
}
