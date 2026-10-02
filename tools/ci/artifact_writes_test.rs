use dx_testing::read_runfiles;

/// Binaries whose whole job is to encode one validated artifact to an output path.
const WRITERS: [&str; 4] = [
    "env/env_shard/src/main.rs",
    "generation/codegen_shard/src/main.rs",
    "quality/evaluator/src/main.rs",
    "quality/runner/src/main.rs",
];

#[test]
fn every_artifact_writer_commits_its_output_atomically() {
    for writer in WRITERS {
        let text = read_runfiles(writer);
        assert!(
            text.contains("dx_atomic_fs::write_atomic("),
            "{writer} does not commit its output with dx_atomic_fs::write_atomic"
        );
        assert!(
            !text.contains("std::fs::write("),
            "{writer} writes its output in place, so a failed run truncates a good artifact"
        );
    }
}
