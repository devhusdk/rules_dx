use yaml_serde::Value;

use dx_testing::read_runfiles;

const PIPELINES: &[&str] = &["ci/gitlab/dx.gitlab-ci.yml"];

const KEYWORDS: &[&str] = &["default", "variables", "workflow", "stages", "include"];

const TEMPLATE: &str = ".dx";

const FINISH: &str = ".dx_finish";

/// Returns the `spec` header, if any, and the pipeline, each of which must be a YAML mapping.
fn documents(path: &str) -> (Option<Value>, Value) {
    let text = read_runfiles(path);
    let (header, body) = match text.split_once("\n---\n") {
        Some((header, body)) => (Some(header), body),
        None => (None, text.as_str()),
    };
    let parse = |part: &str| -> Value {
        let doc: Value = yaml_serde::from_str(part)
            .unwrap_or_else(|error| panic!("{path} is not valid YAML: {error}"));
        assert!(doc.is_mapping(), "{path} is not a YAML mapping");
        doc
    };
    (header.map(parse), parse(body))
}

/// Returns the parsed pipeline after any `spec` header.
fn document(path: &str) -> Value {
    documents(path).1
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

/// Returns the single report a job script writes, with `--report` or by redirecting dx.
fn report_destination(path: &str, job: &str, body: &Value) -> String {
    let lines = strings(path, &body["script"]);
    let mut reports = Vec::new();
    for line in &lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        for pair in words.windows(2) {
            if pair[0] == "--report" {
                let (_, file) = pair[1]
                    .split_once('=')
                    .unwrap_or_else(|| panic!("{path}: job {job} report has no destination"));
                reports.push(file.to_owned());
            } else if pair[0] == ">" && pair[1].starts_with("reports/") {
                reports.push(pair[1].to_owned());
            }
        }
    }
    assert_eq!(reports.len(), 1, "{path}: job {job} must write one report");
    reports.remove(0)
}

#[test]
fn every_job_extends_the_dx_template_and_keeps_reports_on_failure() {
    for path in PIPELINES {
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
    for path in PIPELINES {
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
    for path in PIPELINES {
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
    for path in PIPELINES {
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
    let file = PIPELINES[0];
    let text = read_runfiles(file);
    for banned in ["GITHUB_", "GH_TOKEN", "secrets.", "CI_JOB_TOKEN"] {
        assert!(!text.contains(banned), "{file} must not use {banned}");
    }
}

#[test]
fn the_image_is_digest_pinned() {
    for path in PIPELINES {
        let doc = document(path);
        let image = doc["default"]["image"]
            .as_str()
            .unwrap_or_else(|| panic!("{path} sets no default image"));
        let (_, digest) = image
            .split_once("@sha256:")
            .unwrap_or_else(|| panic!("{path}: image {image} is not digest-pinned"));
        assert_eq!(digest.len(), 64, "{path}: image digest must be a SHA-256");
    }
}

#[test]
fn the_template_reads_every_input_it_declares_and_no_other() {
    let path = PIPELINES[0];
    let (Some(header), _) = documents(path) else {
        panic!("{path} has no spec header");
    };
    let mut declared: Vec<String> = header["spec"]["inputs"]
        .as_mapping()
        .unwrap_or_else(|| panic!("{path} declares no spec.inputs"))
        .keys()
        .map(|name| key(path, name))
        .collect();
    declared.sort();
    let text = read_runfiles(path);
    let mut used: Vec<String> = text
        .split("$[[ inputs.")
        .skip(1)
        .map(|rest| {
            rest.split_once(" ]]")
                .unwrap_or_else(|| panic!("{path}: unclosed input reference"))
                .0
                .to_owned()
        })
        .collect();
    used.sort();
    used.dedup();
    assert_eq!(declared, used, "{path}: declared and read inputs differ");
}

#[cfg(unix)]
#[test]
fn setting_only_dx_remote_cache_turns_on_the_remote_cache() {
    let path = PIPELINES[0];
    let doc = document(path);
    let line = strings(path, &hidden(path, &doc, TEMPLATE)["before_script"])
        .into_iter()
        .find(|line| line.contains("DX_REMOTE_CACHE"))
        .unwrap_or_else(|| panic!("{path}: before_script never reads DX_REMOTE_CACHE"));
    let run = |env: &[(&str, &str)]| -> String {
        let scratch = dx_test_scratch::scratch("gitlab-remote-cache");
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", &line])
            .current_dir(scratch.path())
            .env_remove("DX_REMOTE_CACHE")
            .env_remove("DX_REMOTE_CACHE_HEADER");
        for (name, value) in env {
            command.env(name, value);
        }
        let done = command.status().expect("sh runs");
        assert!(done.success(), "{path}: {line}");
        std::fs::read_to_string(scratch.path().join("user.bazelrc")).unwrap_or_default()
    };
    assert_eq!(run(&[]), "");
    assert_eq!(
        run(&[("DX_REMOTE_CACHE", "grpcs://cache.example.com")]),
        "common --remote_cache=grpcs://cache.example.com\n"
    );
    assert_eq!(
        run(&[
            ("DX_REMOTE_CACHE", "grpcs://cache.example.com"),
            ("DX_REMOTE_CACHE_HEADER", "x-token=abc"),
        ]),
        "common --remote_cache=grpcs://cache.example.com\ncommon --remote_header=x-token=abc\n"
    );
}
