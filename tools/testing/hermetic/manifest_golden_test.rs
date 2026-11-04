use serde_json::Value;

fn list(manifest: &Value, key: &str) -> Vec<String> {
    manifest[key]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn consumer_manifest_matches_its_golden() {
    let rel = std::env::var("DX_HERMETIC_MANIFEST").expect("DX_HERMETIC_MANIFEST is set");
    let golden = std::env::var("DX_GOLDEN").expect("DX_GOLDEN is set");
    let text = std::fs::read_to_string(dx_testing::resolve_runfiles(&rel)).expect("read manifest");
    let manifest: Value = serde_json::from_str(&text).expect("manifest is json");

    let mut lines = Vec::new();
    for language in list(&manifest, "languages") {
        lines.push(format!("language {language}"));
    }
    for pattern in list(&manifest, "patterns") {
        lines.push(format!("pattern {pattern}"));
    }
    let mut dests: Vec<String> = manifest["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file["dest"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    dests.sort_unstable();
    for dest in dests {
        lines.push(format!("file {dest}"));
    }
    if let Some(failures) = manifest["failures"].as_object() {
        let mut targets: Vec<&String> = failures.keys().collect();
        targets.sort_unstable();
        for target in targets {
            lines.push(format!("failure {target}"));
        }
    }
    let fragment = manifest["module_fragment"].as_str().unwrap_or_default();
    if !fragment.is_empty() {
        let body = std::fs::read_to_string(dx_testing::resolve_runfiles(fragment))
            .expect("read module fragment");
        for line in body.lines().filter(|line| line.starts_with("bazel_dep(")) {
            lines.push(format!(
                "bazel_dep {}",
                line.split('"').nth(1).unwrap_or_default()
            ));
        }
    }
    let got = format!("{}\n", lines.join("\n"));
    let want = std::fs::read_to_string(dx_testing::resolve_runfiles(&golden))
        .unwrap_or_else(|error| panic!("read {golden}: {error}"));
    assert_eq!(got, want, "manifest differs from {golden}");
}
