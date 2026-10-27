//! Fails when `docs/support-matrix.md` differs from what the generator writes.

use support_matrix::runfiles_source::RunfilesSource;
use support_matrix::{diff, generate, Source, MATRIX_PATH, UPDATE_COMMAND};

#[test]
fn support_matrix_is_current() {
    let source = RunfilesSource::create().expect("runfiles");
    let facts = source
        .read("tools/docs/support_matrix/facts.json")
        .expect("facts");
    let matrix = generate(&facts, &source, dx_adopt::new::SUPPORTED_NEW_LANGUAGES)
        .unwrap_or_else(|error| panic!("support matrix: {error}"));
    let have = source.read(MATRIX_PATH).expect("checked-in page");
    if let Some(diff) = diff(&have, &matrix.markdown) {
        panic!("{MATRIX_PATH} is stale; run `{UPDATE_COMMAND}`:\n{diff}");
    }
}
