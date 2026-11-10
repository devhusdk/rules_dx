//! `pending.bzl` is sorted, unique and names only known languages, tests and platforms.

use serde_json::Value;

const TESTS: [&str; 3] = ["build_test", "fail_test", "pass_test"];

fn read(var: &str) -> String {
    let key = std::env::var(var).unwrap_or_else(|_| panic!("{var} must be set"));
    let path = dx_testing::rlocation(&key).expect("file must resolve from runfiles");
    std::fs::read_to_string(path).expect("file must be readable")
}

#[test]
fn pending_lines_are_valid() {
    let source = read("DX_CONFORMANCE_PENDING");
    let body = source.split("\"\"\"").nth(3).expect("PENDING string");
    let manifest: Value = serde_json::from_str(&read("DX_CONFORMANCE_MANIFEST")).unwrap();
    let platforms = manifest["platforms"].as_object().expect("platforms map");

    let lines: Vec<&str> = body.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut sorted = lines.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(lines, sorted, "pending.bzl lines must be sorted and unique");

    for line in lines {
        let fields: Vec<&str> = line.split(' ').collect();
        assert_eq!(fields.len(), 3, "want '<lang> <test> <platform>': {line}");
        let (lang, test, platform) = (fields[0], fields[1], fields[2]);
        let known = platforms
            .get(lang)
            .unwrap_or_else(|| panic!("unknown language in: {line}"))
            .as_array()
            .unwrap();
        assert!(TESTS.contains(&test), "unknown test in: {line}");
        assert!(
            platform == "*" || known.iter().any(|p| p == platform),
            "unknown platform in: {line}"
        );
        let listed = manifest["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["lang"] == lang && e["capability"] == test);
        assert!(listed || known.is_empty(), "{lang} {test} is not in the manifest");
    }
}
