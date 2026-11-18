use dx_testing::read_runfiles;

/// Bazel re-checks action outputs by digest, so they need the atomic replace but not the syncs.
const SCRATCH: &str = "dx_atomic_fs::write_scratch(";
const SHARD: &str = "documentation_ir::tool::write_shard(";
const SYNCED: &str = "dx_atomic_fs::write_atomic(";
const IN_PLACE: &str = "fs::write(";
const HAND_RENAME: &str = "fs::rename(";

/// Sources that write a whole file, with the dx_atomic_fs call each must commit it through.
const WRITERS: [(&str, &str); 9] = [
    ("appimage/private/tools/packer/src/lib.rs", SCRATCH),
    ("apple/private/tools/ops/src/app_check.rs", SCRATCH),
    ("apple/private/tools/ops/src/xcframework_check.rs", SCRATCH),
    ("cli/shard_writer/src/lib.rs", SCRATCH),
    ("core/quality/runner/src/entry.rs", SCRATCH),
    ("tools/docs/ir/ir/src/tool.rs", SCRATCH),
    ("tools/docs/rust/extract/src/lib.rs", SHARD),
    ("tools/docs/support_matrix/src/main.rs", SYNCED),
    ("tools/testing/hermetic/appcontainer/user.rs", SYNCED),
];

/// Returns a writer's production half: everything above its test module.
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
