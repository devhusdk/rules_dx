//! Generates the matrix from the real facts, so a `ci_tests` label the reader does not understand fails here.

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{generate, Source};

#[test]
fn every_ci_test_label_is_understood() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    generate(&facts, &source).expect("generator understands every ci_tests label");
}
