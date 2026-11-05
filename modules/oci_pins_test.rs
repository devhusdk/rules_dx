//! Every base image in the root MODULE.bazel is pinned by digest for both Linux CPUs.

use dx_module_bazel::ModuleFile;

const BASES: [(&str, &str); 5] = [
    ("distroless_static", "gcr.io/distroless/static-debian12"),
    ("distroless_cc", "gcr.io/distroless/cc-debian12"),
    ("distroless_python", "gcr.io/distroless/python3-debian12"),
    ("distroless_java", "gcr.io/distroless/java21-debian12"),
    ("distroless_nodejs", "gcr.io/distroless/nodejs22-debian12"),
];

fn is_digest(text: &str) -> bool {
    text.strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[test]
fn bases_are_pinned_by_digest_for_amd64_and_arm64() {
    let module =
        ModuleFile::parse(&dx_testing::read_runfiles("MODULE.bazel")).expect("MODULE.bazel");
    let pulls: Vec<_> = module.calls_named("oci.pull").collect();
    assert_eq!(pulls.len(), BASES.len(), "one oci.pull per base");
    for (name, image) in BASES {
        let pull = pulls
            .iter()
            .find(|call| call.str("name") == Some(name))
            .unwrap_or_else(|| panic!("no oci.pull named {name}"));
        assert_eq!(pull.str("image"), Some(image), "{name} image");
        let digest = pull.str("digest").unwrap_or_default();
        assert!(
            is_digest(digest),
            "{name} digest {digest:?} is not sha256:<64 hex>"
        );
        assert_eq!(
            pull.str_list("platforms"),
            ["linux/amd64", "linux/arm64"],
            "{name} platforms"
        );
    }
}

#[test]
fn digest_check_rejects_tags_and_short_hashes() {
    assert!(is_digest(&format!("sha256:{}", "a".repeat(64))));
    assert!(!is_digest("latest"));
    assert!(!is_digest("sha256:abc"));
}
