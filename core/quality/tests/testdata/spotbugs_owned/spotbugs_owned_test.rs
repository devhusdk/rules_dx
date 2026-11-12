//! SpotBugs results belong to the target that owns the source, including test and compile-only deps.

use dx_testing::{contains_bytes, read_runfile};

const DIR: &str = "core/quality/tests/testdata/spotbugs_owned";

fn result(name: &str) -> Vec<u8> {
    read_runfile(&format!("{DIR}/{name}-real-lint-jvm.pb"))
}

fn has(bytes: &[u8], text: &[u8]) -> bool {
    contains_bytes(bytes, text)
}

#[test]
fn findings_stay_with_their_owner() {
    let app = result("app");
    let dep = result("dep");
    assert!(has(&dep, b"NP_ALWAYS_NULL"), "dep finding missing");
    assert!(
        !has(&app, b"NP_ALWAYS_NULL") && !has(&app, b"Dirty.java"),
        "app result contains the dependency finding"
    );
    assert!(has(&app, b"Clean.java"), "app result missing");
    assert!(has(&dep, b"SF_DEAD_STORE_DUE_TO_SWITCH_FALLTHROUGH"), "multiline finding missing");
    let placed = b"SF_DEAD_STORE_DUE_TO_SWITCH_FALLTHROUGH*5core/quality/tests/testdata/spotbugs_owned/Dirty.java0\xdb\x028\xdb\x02";
    assert!(has(&dep, placed), "multiline finding not placed");
    assert!(has(&result("app_test"), b"CleanTest.java"), "test result missing");
    assert!(has(&result("implicit_test"), b"ImplicitTest.java"), "implicit junit test result missing");
    assert!(has(&result("uses_api"), b"UsesApi.java"), "compile-only dep result missing");
    assert!(has(&result("collide"), b"Collide.java"), "same-named compile-only dep result missing");
}
