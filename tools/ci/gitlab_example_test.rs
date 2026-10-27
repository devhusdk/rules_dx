use yaml_serde::Value;

use dx_testing::read_runfiles;

const PIPELINES: &[(&str, &str)] = &[(
    "examples/gitlab-ci/.gitlab-ci.yml",
    "examples/gitlab-ci/README.md",
)];

const KEYWORDS: &[&str] = &["default", "variables", "workflow", "stages", "include"];

const TEMPLATE: &str = ".dx";

const FINISH: &str = ".dx_finish";

/// Returns the parsed pipeline, which must be valid YAML.
fn document(path: &str) -> Value {
    let doc: Value = yaml_serde::from_str(&read_runfiles(path))
        .unwrap_or_else(|error| panic!("{path} is not valid YAML: {error}"));
    assert!(doc.is_mapping(), "{path} is not a YAML mapping");
    doc
}

/// Returns the key of one mapping entry as a string.
fn key(path: &str, name: &Value) -> String {
    name.as_str()
        .unwrap_or_else(|| panic!("{path}: non-string top-level key {name:?}"))
        .to_owned()
}

/// Returns every job in the pipeline, skipping keywords and hidden keys.
fn jobs(path: &str, doc: &Value) -> Vec<(String, Value)> {
    let got: Vec<(String, Value)> = doc
        .as_mapping()
        .expect("checked in document")
        .iter()
        .map(|(name, body)| (key(path, name), body))
        .filter(|(name, _)| !name.starts_with('.') && !KEYWORDS.contains(&name.as_str()))
        .map(|(name, body)| {
            assert!(body.is_mapping(), "{path}: job {name} is not a mapping");
            (name, body.clone())
        })
        .collect();
    assert!(!got.is_empty(), "{path} declares no job");
    got
}

/// Returns one hidden template from the pipeline.
fn hidden<'a>(path: &str, doc: &'a Value, name: &str) -> &'a Value {
    doc.get(name)
        .unwrap_or_else(|| panic!("{path} declares no {name} template"))
}

/// Returns the string items of one YAML list.
fn strings(path: &str, value: &Value) -> Vec<String> {
    value
        .as_sequence()
        .unwrap_or_else(|| panic!("{path}: expected a list, got {value:?}"))
        .iter()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect()
}

/// Returns the single dx report destination a job script writes.
fn report_destination(path: &str, job: &str, body: &Value) -> String {
    let lines = strings(path, &body["script"]);
    let mut reports = Vec::new();
    for line in &lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        for pair in words.windows(2) {
            if pair[0] == "--report" {
                reports.push(pair[1].to_owned());
            }
        }
    }
    assert_eq!(reports.len(), 1, "{path}: job {job} must write one report");
    reports[0]
        .split_once('=')
        .unwrap_or_else(|| panic!("{path}: job {job} report has no destination"))
        .1
        .to_owned()
}

/// Returns the text of one fenced block that follows a heading line.
fn block_after(text: &str, marker: &str) -> String {
    let start = text
        .find(marker)
        .unwrap_or_else(|| panic!("README never says {marker}"));
    text[start..]
        .split("```")
        .nth(1)
        .unwrap_or_else(|| panic!("README has no block after {marker}"))
        .to_owned()
}

#[test]
fn every_job_extends_the_dx_template_and_keeps_reports_on_failure() {
    for (path, _) in PIPELINES {
        let doc = document(path);
        let artifacts = &hidden(path, &doc, TEMPLATE)["artifacts"];
        assert_eq!(
            artifacts["when"].as_str(),
            Some("always"),
            "{path}: {TEMPLATE} artifacts must keep reports on failure"
        );
        assert_eq!(
            strings(path, &artifacts["paths"]),
            ["reports/"],
            "{path}: {TEMPLATE} must keep only reports/"
        );
        for (job, body) in jobs(path, &doc) {
            assert_eq!(
                body.get("extends").and_then(Value::as_str),
                Some(TEMPLATE),
                "{path}: job {job} must extend {TEMPLATE}"
            );
            for field in ["before_script", "cache", "image"] {
                assert!(
                    body.get(field).is_none(),
                    "{path}: job {job} must not override {field}"
                );
            }
            if let Some(own) = body.get("artifacts") {
                for field in ["paths", "when"] {
                    assert!(
                        own.get(field).is_none(),
                        "{path}: job {job} must not override artifacts.{field}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_job_report_matches_its_dx_report_and_artifact() {
    for (path, _) in PIPELINES {
        let doc = document(path);
        for (job, body) in jobs(path, &doc) {
            let destination = report_destination(path, &job, &body);
            assert!(
                destination.starts_with("reports/"),
                "{path}: job {job} report must land under reports/"
            );
            assert_eq!(
                body["variables"]["DX_REPORT"].as_str(),
                Some(destination.as_str()),
                "{path}: job {job} DX_REPORT must name its --report file"
            );
            if let Some(junit) = body
                .get("artifacts")
                .map(|artifacts| &artifacts["reports"]["junit"])
            {
                assert_eq!(
                    junit.as_str(),
                    Some(destination.as_str()),
                    "{path}: job {job} junit artifact must be its --report file"
                );
            }
        }
        let junit = jobs(path, &doc)
            .into_iter()
            .filter(|(_, body)| {
                body.get("artifacts")
                    .is_some_and(|artifacts| !artifacts["reports"]["junit"].is_null())
            })
            .count();
        assert_eq!(
            junit, 1,
            "{path}: exactly one job must publish a JUnit report"
        );
    }
}

#[test]
fn every_job_ends_with_the_finish_gate() {
    for (path, _) in PIPELINES {
        let doc = document(path);
        let finish = strings(path, &hidden(path, &doc, FINISH)["script"]);
        assert_eq!(
            finish.last().map(String::as_str),
            Some("exit \"$status\""),
            "{path}: {FINISH} must end with the kept status"
        );
        for (job, body) in jobs(path, &doc) {
            let script = body["script"]
                .as_sequence()
                .unwrap_or_else(|| panic!("{path}: job {job} has no script list"));
            assert_eq!(
                script.first().and_then(Value::as_str),
                Some("status=0"),
                "{path}: job {job} must start with status=0"
            );
            let Some(Value::Tagged(last)) = script.last() else {
                panic!("{path}: job {job} must end with !reference [{FINISH}, script]");
            };
            assert!(
                last.tag == "reference",
                "{path}: job {job} ends with {} instead of !reference",
                last.tag
            );
            assert_eq!(
                strings(path, &last.value),
                [FINISH, "script"],
                "{path}: job {job} must reference {FINISH} script"
            );
            for line in &script[1..script.len() - 1] {
                let line = line.as_str().unwrap_or_default();
                assert!(
                    line.ends_with("|| status=$?"),
                    "{path}: job {job} must keep the dx status: {line}"
                );
            }
        }
    }
}

#[test]
fn the_cache_is_keyed_on_the_lockfile_and_stays_protected() {
    for (path, _) in PIPELINES {
        let doc = document(path);
        let cache = &hidden(path, &doc, TEMPLATE)["cache"];
        let files = strings(path, &cache["key"]["files"]);
        assert!(
            files.contains(&"MODULE.bazel.lock".to_owned()),
            "{path}: cache key must read MODULE.bazel.lock"
        );
        assert!(
            files.len() <= 2,
            "{path}: GitLab takes at most 2 cache key files"
        );
        assert!(
            cache.get("unprotect").is_none(),
            "{path}: cache must stay split by protection"
        );
        for entry in strings(path, &cache["paths"]) {
            assert!(
                entry.starts_with(".cache/bazel/"),
                "{path}: cache path {entry} leaves .cache/bazel/"
            );
            assert!(
                !entry.contains("remote.bazelrc"),
                "{path}: cache must never hold remote.bazelrc"
            );
        }
        for entry in strings(path, &hidden(path, &doc, TEMPLATE)["artifacts"]["paths"]) {
            assert!(
                !entry.contains(".cache"),
                "{path}: artifacts must never hold .cache"
            );
        }
    }
}

#[test]
fn the_example_needs_no_github_credentials() {
    for (path, readme) in PIPELINES {
        for file in [*path, *readme] {
            let text = read_runfiles(file);
            for banned in ["GITHUB_", "GH_TOKEN", "secrets.", "CI_JOB_TOKEN"] {
                assert!(!text.contains(banned), "{file} must not use {banned}");
            }
        }
    }
}

#[test]
fn the_example_pins_bazelisk_to_the_canonical_version_and_digest() {
    let versions = read_runfiles("modules/versions.bzl");
    let version = versions
        .lines()
        .find_map(|line| line.strip_prefix("BAZELISK_VERSION = "))
        .expect("modules/versions.bzl pins BAZELISK_VERSION")
        .trim_matches('"');
    let dockerfile = read_runfiles(".devcontainer/Dockerfile.prebuilt");
    for (path, readme) in PIPELINES {
        let doc = document(path);
        let variables = &doc["variables"];
        assert_eq!(
            variables["BAZELISK_VERSION"].as_str(),
            Some(version),
            "{path}: BAZELISK_VERSION must match modules/versions.bzl"
        );
        let sha = variables["BAZELISK_SHA256"]
            .as_str()
            .unwrap_or_else(|| panic!("{path} pins no BAZELISK_SHA256"));
        assert_eq!(sha.len(), 64, "{path}: BAZELISK_SHA256 must be a SHA-256");
        assert!(
            dockerfile.contains(&format!("{sha}  /tmp/bazelisk")),
            "{path}: BAZELISK_SHA256 must match .devcontainer/Dockerfile.prebuilt"
        );
        assert!(
            read_runfiles(readme).contains(&format!("Bazelisk {version}")),
            "{readme} must name Bazelisk {version}"
        );
    }
}

#[test]
fn the_image_is_digest_pinned_and_named_in_the_readme() {
    for (path, readme) in PIPELINES {
        let doc = document(path);
        let image = doc["default"]["image"]
            .as_str()
            .unwrap_or_else(|| panic!("{path} sets no default image"));
        let (_, digest) = image
            .split_once("@sha256:")
            .unwrap_or_else(|| panic!("{path}: image {image} is not digest-pinned"));
        assert_eq!(digest.len(), 64, "{path}: image digest must be a SHA-256");
        assert!(
            read_runfiles(readme).contains(&format!("`{image}`")),
            "{readme} must name the image {image}"
        );
    }
}

#[test]
fn the_readme_ignores_every_path_the_pipeline_writes() {
    for (_, readme) in PIPELINES {
        let text = read_runfiles(readme);
        let gitignore = block_after(&text, "`.gitignore`");
        for entry in [".cache/", "reports/", "bazel-*", ".dx/"] {
            assert!(
                gitignore.lines().any(|line| line.trim() == entry),
                "{readme}: .gitignore block must list {entry}"
            );
        }
        let bazelignore = block_after(&text, "`.bazelignore`");
        assert!(
            bazelignore.lines().any(|line| line.trim() == ".cache"),
            "{readme}: .bazelignore block must list .cache"
        );
    }
}
