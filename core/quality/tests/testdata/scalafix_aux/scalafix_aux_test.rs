//! Scalafix results name each source and carry a patch, with semantic rules resolving types.

use dx_testing::{contains_bytes, read_runfile};

const DIR: &str = "core/quality/tests/testdata/scalafix_aux";
const FAILURES: [&str; 6] = [
    "unexpected error",
    "parse error",
    "command line error",
    "SemanticDB not found",
    "stale SemanticDB",
    "no files to check",
];

fn result(name: &str) -> Vec<u8> {
    read_runfile(&format!("{DIR}/{name}-real-lint-scala.pb"))
}

fn has(bytes: &[u8], text: &str) -> bool {
    contains_bytes(bytes, text.as_bytes())
}

fn check_semantic(name: &str, source: &str, ty: &str) {
    let bytes = result(name);
    assert!(has(&bytes, &format!("{DIR}/{source}")), "{name}: {source} result missing");
    assert!(
        has(&bytes, "ExplicitResultTypes") || has(&bytes, "Patch"),
        "{name}: ExplicitResultTypes patch missing"
    );
    assert!(has(&bytes, &format!(": {ty}")), "{name}: patch does not name {ty}");
    for failure in FAILURES {
        assert!(!has(&bytes, failure), "{name}: scalafix failed with {failure}");
    }
}

#[test]
fn explicit_result_types_resolve_dependency_types() {
    check_semantic("uses_api", "UsesApi.scala", "Api");
    check_semantic("core_test", "CoreTest.scala", "Box");
}

#[test]
fn remove_unused_patches_scala_2_and_3() {
    let unused = result("unused_lib");
    assert!(has(&unused, &format!("{DIR}/Unused.scala")), "unused_lib: Unused.scala result missing");
    assert!(has(&unused, "Patch"), "unused_lib: patch missing");
    let unused3 = result("unused3_lib");
    assert!(has(&unused3, &format!("{DIR}/Unused3.scala")), "unused3_lib: Unused3.scala result missing");
    assert!(has(&unused3, "Patch"), "unused3_lib: RemoveUnused patch missing on Scala 3");
}
