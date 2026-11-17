//! Every `# gazelle:ignore` in a `<lang>/tests/conformance/BUILD.bazel` says why on the next line.
//!
//! A blanket ignore hides the file from `dx generate`, so drift between the suite and what gazelle
//! would write goes unnoticed. Prefer a narrow `# gazelle:exclude <file>` plus `# keep` on the
//! hand-written fixture targets; when gazelle cannot produce the file at all, name the reason.

const IGNORE: &str = "# gazelle:ignore";

/// Returns one line per reasonless ignore in `body`, labelled with `name`.
fn problems(name: &str, body: &str) -> Vec<String> {
    let lines: Vec<&str> = body.lines().map(str::trim).collect();
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if *line != IGNORE {
            continue;
        }
        let reason = lines
            .get(index + 1)
            .and_then(|next| next.strip_prefix('#'))
            .map(str::trim)
            .filter(|text| !text.is_empty() && !text.starts_with("gazelle:"));
        if reason.is_none() {
            out.push(format!(
                "{name}:{}: '{IGNORE}' needs a '# <reason>' comment on the next line",
                index + 1
            ));
        }
    }
    out
}

#[test]
fn conformance_gazelle_ignores_name_a_reason() {
    let keys = std::env::var("DX_CONFORMANCE_BUILD_FILES").expect("DX_CONFORMANCE_BUILD_FILES must be set");
    let keys: Vec<&str> = keys.split_whitespace().collect();
    assert!(!keys.is_empty(), "DX_CONFORMANCE_BUILD_FILES must name the conformance BUILD files");
    let mut out = Vec::new();
    for key in keys {
        let path = dx_testing::rlocation(key).expect("BUILD file must resolve from runfiles");
        let body = std::fs::read_to_string(path).expect("BUILD file must be readable");
        out.extend(problems(key, &body));
    }
    assert!(out.is_empty(), "reasonless gazelle:ignore lines:\n{}", out.join("\n"));
}

#[test]
fn flags_a_bare_ignore() {
    assert_eq!(problems("a", "# gazelle:ignore\nload(\"//x:y.bzl\", \"z\")\n").len(), 1);
    assert_eq!(problems("a", "# gazelle:ignore\n# gazelle:exclude x.go\n").len(), 1);
    assert_eq!(problems("a", "# gazelle:ignore\n#\n").len(), 1);
    assert_eq!(problems("a", "# gazelle:ignore").len(), 1);
}

#[test]
fn accepts_an_ignore_with_a_reason() {
    assert!(problems("a", "# gazelle:ignore\n# Why: gazelle merges the two tests.\n").is_empty());
    assert!(problems("a", "load(\"//x:y.bzl\", \"z\")\n").is_empty());
}
