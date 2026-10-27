//! Nested consumers must take the pinned Maven block from `dx_nested`, never spell their own.

use dx_testing::{read, rlocation};

#[test]
fn consumers_use_the_shared_maven_block() {
    let files = std::env::var("DX_GUARD_FILES").expect("DX_GUARD_FILES");
    let mut seen = 0;
    for key in files.split(' ').filter(|key| !key.is_empty()) {
        let text = read(&rlocation(key).expect("rlocation"));
        let literal = ["maven", ".install("].concat();
        assert!(
            !text.contains(&literal),
            "{key} spells its own maven.install; use nested::CONSUMER_MAVEN"
        );
        seen += 1;
    }
    assert!(seen > 5, "only {seen} consumer tests were checked");
}
