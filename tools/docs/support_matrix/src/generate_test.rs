use runfiles as _;
use serde as _;
use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{
    CONSUMER_LABELS_PATH, MATRIX_PATH, ROW_LABELS_PATH, Source, SupportMatrixError,
    UNSUPPORTED_PATH, UPDATE_COMMAND, diff, generate, parse_facts, unsupported_json,
};
use yaml_serde as _;

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

#[test]
fn a_row_test_missing_from_every_platform_file_fails() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    struct Renamed<'a>(&'a RunfilesSource);
    impl Source for Renamed<'_> {
        fn read(&self, path: &str) -> Result<String, SupportMatrixError> {
            let text = self.0.read(path)?;
            if path != CONSUMER_LABELS_PATH {
                return Ok(text);
            }
            Ok(text.replace(
                "//tools/testing/hermetic:appimage",
                "//tools/testing/hermetic:not_queried",
            ))
        }
    }
    let error = generate(&facts, &Renamed(&source)).expect_err("an unqueried row test");
    assert!(
        matches!(&error, SupportMatrixError::UnqueriedLabel { label, .. } if label == "//tools/testing/hermetic:not_queried"),
        "{error}"
    );
    assert!(
        error.to_string().contains("support_matrix:row_tests"),
        "{error}"
    );
}

#[test]
fn a_row_tag_naming_no_row_fails() {
    struct WithRow<'a>(&'a RunfilesSource);
    impl Source for WithRow<'_> {
        fn read(&self, path: &str) -> Result<String, SupportMatrixError> {
            let text = self.0.read(path)?;
            if path != ROW_LABELS_PATH {
                return Ok(text);
            }
            let mut labels: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&text).expect("row labels parse");
            labels.insert("no_such_row".to_owned(), serde_json::json!([]));
            Ok(serde_json::to_string(&labels).expect("row labels serialize"))
        }
    }
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let error = generate(&facts, &WithRow(&source)).expect_err("a row tag naming no row");
    assert!(
        matches!(&error, SupportMatrixError::UnknownRow { row, .. } if row == "no_such_row")
            && error.to_string().contains("support_matrix:row_tests"),
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
    let languages: Vec<serde_json::Value> = value["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|row| row["section"] == "languages")
        .map(|row| row["name"].clone())
        .collect();
    let entries = value["conformance"]["entries"]
        .as_array_mut()
        .expect("conformance entries");
    let removed = entries
        .iter()
        .find(|e| {
            e["status"] == "run"
                && e["capability"] == "build_test"
                && languages.contains(&e["lang"])
        })
        .expect("a language runs build_test")["lang"]
        .clone();
    entries.retain(|e| !(e["lang"] == removed && e["capability"] == "build_test"));
    let error = generate(&value.to_string(), &source).expect_err("a removed suite test");
    assert!(error.to_string().contains("pending.bzl"), "{error}");
}
