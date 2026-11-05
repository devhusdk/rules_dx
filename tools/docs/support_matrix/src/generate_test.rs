//! Generates the matrix from the real facts. A `ci_tests` label the reader does not understand fails here, and so does a committed page that is out of date.

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{
    diff, generate, parse_facts, unsupported_json, Source, MATRIX_PATH, UNSUPPORTED_PATH,
    UPDATE_COMMAND,
};

fn generated() -> (String, RunfilesSource) {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let matrix = generate(&facts, &source).expect("generator understands every ci_tests label");
    (matrix, source)
}

#[test]
fn every_ci_test_label_is_understood() {
    generated();
}

#[test]
fn committed_page_matches_the_source_data() {
    let (matrix, source) = generated();
    let committed = source.read(MATRIX_PATH).expect("committed page");
    if let Some(changes) = diff(&committed, &matrix) {
        panic!("{MATRIX_PATH} is out of date (-committed, +generated):\n{changes}\nrun `{UPDATE_COMMAND}`");
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
        panic!("{UNSUPPORTED_PATH} is out of date (-committed, +generated):\n{changes}\nrun `{UPDATE_COMMAND}`");
    }
}
