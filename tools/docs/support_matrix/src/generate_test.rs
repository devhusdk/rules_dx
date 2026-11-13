//! Generates the matrix from the real facts. A label the reader does not understand fails here, and so does a committed page that is out of date.

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{
    CONSUMER_LABELS_PATH, MATRIX_PATH, ROW_LABELS_PATH, Source, UNSUPPORTED_PATH, UPDATE_COMMAND,
    diff, generate, parse_facts, unsupported_json,
};

fn generated() -> (String, RunfilesSource) {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let matrix = generate(&facts, &source).expect("generator understands every label");
    (matrix, source)
}

#[test]
fn every_label_is_understood() {
    generated();
}

#[test]
fn committed_page_matches_the_source_data() {
    let (matrix, source) = generated();
    let committed = source.read(MATRIX_PATH).expect("committed page");
    if let Some(changes) = diff(&committed, &matrix) {
        panic!(
            "{MATRIX_PATH} is out of date (-committed, +generated):\n{changes}\nrun `{UPDATE_COMMAND}`"
        );
    }
}

#[test]
fn committed_unsupported_json_matches_the_source_data() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let want = unsupported_json(&parse_facts(&facts).expect("facts parse"));
    let committed = source.read(UNSUPPORTED_PATH).expect("committed json");
    if let Some(changes) = diff(&committed, &want) {
        panic!(
            "{UNSUPPORTED_PATH} is out of date (-committed, +generated):\n{changes}\nrun `{UPDATE_COMMAND}`"
        );
    }
}

struct WithoutTag<'a> {
    inner: &'a RunfilesSource,
    label: &'a str,
    tag: &'a str,
}

impl Source for WithoutTag<'_> {
    fn read(&self, path: &str) -> Result<String, String> {
        let text = self.inner.read(path)?;
        if !path.starts_with("tools/docs/support_matrix/row_tests/") {
            return Ok(text);
        }
        let mut tests: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&text).map_err(|error| error.to_string())?;
        if let Some(serde_json::Value::Array(tags)) = tests.get_mut(self.label) {
            tags.retain(|tag| tag != self.tag);
        }
        serde_json::to_string(&tests).map_err(|error| error.to_string())
    }
}

#[test]
fn compiler_consumer_tests_must_be_nested_qualifications() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let golden = "//tools/testing/hermetic:c_fixture_msvc";
    assert!(facts.contains(golden), "the msvc row lists {golden}");
    let untagged = WithoutTag {
        inner: &source,
        label: golden,
        tag: "hermetic-consumer",
    };
    let error = generate(&facts, &untagged).expect_err("a non-qualification consumer test");
    assert!(error.contains("hermetic-consumer"), "{error}");
}

#[test]
fn a_row_test_missing_from_every_platform_file_fails() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    struct Renamed<'a>(&'a RunfilesSource);
    impl Source for Renamed<'_> {
        fn read(&self, path: &str) -> Result<String, String> {
            let text = self.0.read(path)?;
            if path != CONSUMER_LABELS_PATH {
                return Ok(text);
            }
            Ok(text.replace(
                "//tools/testing/hermetic:c_fixture_msvc",
                "//tools/testing/hermetic:not_queried",
            ))
        }
    }
    let error = generate(&facts, &Renamed(&source)).expect_err("an unqueried row test");
    assert!(error.contains("row_tests.sh"), "{error}");
}

#[test]
fn row_labels_match_the_ci_tests_of_every_compiler_and_target_row() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = parse_facts(
        &source
            .read("tools/docs/support_matrix/facts.json")
            .expect("facts"),
    )
    .expect("facts parse");
    let labels: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_str(&source.read(ROW_LABELS_PATH).expect("row labels"))
            .expect("row labels parse");
    for row in facts
        .rows
        .iter()
        .filter(|row| row.section == "compilers" || row.section == "targets")
    {
        let mut want = row.ci_tests.clone();
        want.sort();
        let have = labels.get(&row.name).cloned().unwrap_or_default();
        let missing: Vec<_> = want.iter().filter(|l| !have.contains(l)).collect();
        let extra: Vec<_> = have.iter().filter(|l| !want.contains(l)).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "row {} differs from its row:{} tags; missing the tag: {missing:?}; tagged but not in ci_tests: {extra:?}; run `tools/docs/support_matrix/row_tests.sh <platform>`",
            row.name,
            row.name
        );
    }
}

#[test]
fn consumer_labels_match_the_consumer_tests_of_every_row() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = parse_facts(
        &source
            .read("tools/docs/support_matrix/facts.json")
            .expect("facts"),
    )
    .expect("facts parse");
    let labels: std::collections::BTreeMap<String, Vec<String>> =
        serde_json::from_str(&source.read(CONSUMER_LABELS_PATH).expect("consumer labels"))
            .expect("consumer labels parse");
    for row in &facts.rows {
        let mut want = row.consumer_tests.clone();
        want.sort();
        let have = labels.get(&row.name).cloned().unwrap_or_default();
        let missing: Vec<_> = want.iter().filter(|l| !have.contains(l)).collect();
        let extra: Vec<_> = have.iter().filter(|l| !want.contains(l)).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "row {} differs from its consumer-row:{} tags; missing the tag: {missing:?}; tagged but not in consumer_tests: {extra:?}; run `tools/docs/support_matrix/row_tests.sh <platform>`",
            row.name,
            row.name
        );
    }
}

#[test]
fn a_row_tag_naming_no_row_fails() {
    struct WithRow<'a>(&'a RunfilesSource);
    impl Source for WithRow<'_> {
        fn read(&self, path: &str) -> Result<String, String> {
            let text = self.0.read(path)?;
            if path != ROW_LABELS_PATH {
                return Ok(text);
            }
            let mut labels: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&text).map_err(|error| error.to_string())?;
            labels.insert("no_such_row".to_owned(), serde_json::json!([]));
            serde_json::to_string(&labels).map_err(|error| error.to_string())
        }
    }
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let error = generate(&facts, &WithRow(&source)).expect_err("a row tag naming no row");
    assert!(
        error.contains("no_such_row") && error.contains("row_tests.sh"),
        "{error}"
    );
}

#[test]
fn a_removed_suite_test_fails_without_a_pending_or_not_applicable_entry() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let mut value: serde_json::Value = serde_json::from_str(&facts).expect("facts parse");
    let entries = value["conformance"]["entries"]
        .as_array_mut()
        .expect("conformance entries");
    let removed = entries
        .iter()
        .find(|e| e["status"] == "run" && e["capability"] == "build_test")
        .expect("a language runs build_test")["lang"]
        .clone();
    entries.retain(|e| !(e["lang"] == removed && e["capability"] == "build_test"));
    let error = generate(&value.to_string(), &source).expect_err("a removed suite test");
    assert!(error.contains("pending.bzl"), "{error}");
}
