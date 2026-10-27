//! A consumer seeded with the rules_dx lock facts resolves rules_dx's extensions offline (#2978).

use dx_nested::Case;

/// The facts key aspect_rules_py's `python_interpreters` extension stores its SHA256SUMS under.
const PYTHON_FACTS: &str = "@@aspect_rules_py+//py:extensions.bzl%python_interpreters";

#[test]
fn seeded_consumer_resolves_python_interpreters_offline() {
    let case = Case::consumer("seed", "");
    let run = case.bazel(&[
        "query",
        "--color=no",
        "--curses=no",
        "@@aspect_rules_py++python_interpreters+python_interpreters//:all",
    ]);
    assert!(run.status.success(), "{}", run.combined());

    let lock = dx_testing::read_json(&case.root().join("MODULE.bazel.lock"))
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        lock["facts"].get(PYTHON_FACTS).is_some(),
        "the consumer lock lost the python_interpreters facts: {lock}"
    );
}
