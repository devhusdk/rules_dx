use dx_testing::read_runfiles;

const SCRATCH: &str = "dx_atomic_fs::write_scratch(";
const SHARD: &str = "documentation_ir::tool::write_shard(";
const SYNCED: &str = "dx_atomic_fs::write_atomic(";
const COPY_SCRATCH: &str = "dx_atomic_fs::copy_scratch(";
const COPY_SYNCED: &str = "dx_atomic_fs::copy_atomic(";
const IN_PLACE: &str = "fs::write(";
const HAND_RENAME: &str = "fs::rename(";

const WRITERS: [(&str, &str); 11] = [
    ("android/private/packaging/ops/src/sign.rs", COPY_SYNCED),
    ("appimage/private/tools/packer/src/lib.rs", SCRATCH),
    ("apple/private/tools/ops/src/app_check.rs", SCRATCH),
    ("apple/private/tools/ops/src/xcframework_check.rs", SCRATCH),
    ("cli/shard_writer/src/lib.rs", SCRATCH),
    ("core/quality/runner/src/entry.rs", SCRATCH),
    ("core/quality/runner/src/real/staging.rs", COPY_SCRATCH),
    ("tools/docs/ir/ir/src/tool.rs", SCRATCH),
    ("tools/docs/rust/extract/src/lib.rs", SHARD),
    ("tools/docs/support_matrix/src/main.rs", SYNCED),
    ("tools/testing/hermetic/appcontainer/user.rs", SYNCED),
];

const STAGED_WRITERS: [(&str, &str); 3] = [
    ("cli/shim/src/fetch.rs", "dx_atomic_fs::StagedFile::create("),
    (
        "deploy/private/src/oci.rs",
        "dx_atomic_fs::StagedFile::create(",
    ),
    (
        "deploy/private/src/oci.rs",
        "dx_atomic_fs::commit::publish_tree_swap(",
    ),
];

const NO_HAND_RENAME: [&str; 3] = [
    "cli/audit/src/image_scan.rs",
    "cli/env/src/lib.rs",
    "cli/ide/src/link.rs",
];

fn production(writer: &str) -> String {
    let text = read_runfiles(writer);
    match text.find("#[cfg(test)]") {
        Some(at) => text
            .get(..at)
            .expect("string slice within bounds")
            .to_owned(),
        None => text,
    }
}

#[test]
fn every_artifact_writer_commits_its_output_atomically() {
    for (writer, commit) in WRITERS {
        let text = production(writer);
        assert!(
            text.contains(commit),
            "{writer} does not commit its output with {commit}"
        );
        assert!(
            !text.contains(IN_PLACE),
            "{writer} writes its output in place, so a failed run truncates a good artifact"
        );
        assert!(
            !text.contains(HAND_RENAME),
            "{writer} renames its own temporary file instead of using {commit}"
        );
    }
}

#[test]
fn every_staged_writer_commits_without_its_own_rename() {
    for (writer, commit) in STAGED_WRITERS {
        let text = production(writer);
        assert!(
            text.contains(commit),
            "{writer} does not publish its staged output with {commit}"
        );
        assert!(
            !text.contains(HAND_RENAME),
            "{writer} renames its own temporary file instead of using {commit}"
        );
    }
}

#[test]
fn link_and_tree_swaps_go_through_dx_atomic_fs() {
    for source in NO_HAND_RENAME {
        assert!(
            !production(source).contains(HAND_RENAME),
            "{source} renames by hand instead of using dx_atomic_fs replace_pointer or publish_tree_swap"
        );
    }
}
