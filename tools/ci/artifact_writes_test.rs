use dx_testing::read_runfiles;

/// Sources whose whole job is to encode one validated artifact to an output path.
const WRITERS: [&str; 6] = [
    "appimage/private/tools/packer/src/lib.rs",
    "apple/private/tools/ops/src/app_check.rs",
    "apple/private/tools/ops/src/xcframework_check.rs",
    "cli/shard_writer/src/lib.rs",
    "core/quality/runner/src/entry.rs",
    "tools/docs/ir/ir/src/tool.rs",
];

/// Bazel re-checks action outputs by digest, so they need the atomic replace but not the syncs.
const ATOMIC: &str = "dx_atomic_fs::write_scratch(";
const IN_PLACE: &str = "fs::write(";
const HAND_RENAME: &str = "fs::rename(";

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
    for writer in WRITERS {
        let text = production(writer);
        assert!(
            text.contains(ATOMIC),
            "{writer} does not commit its output with dx_atomic_fs::write_scratch"
        );
        assert!(
            !text.contains(IN_PLACE),
            "{writer} writes its output in place, so a failed run truncates a good artifact"
        );
        assert!(
            !text.contains(HAND_RENAME),
            "{writer} renames its own temporary file instead of using dx_atomic_fs::write_scratch"
        );
    }
}
