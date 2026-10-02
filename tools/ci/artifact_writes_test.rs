use std::path::{Path, PathBuf};

/// Binaries whose whole job is to encode one validated artifact to an output path.
const WRITERS: [&str; 4] = [
    "env/env_shard/src/main.rs",
    "generation/codegen_shard/src/main.rs",
    "quality/evaluator/src/main.rs",
    "quality/runner/src/main.rs",
];

fn workspace_root() -> PathBuf {
    let root = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR is set under Bazel");
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE is set under Bazel");
    Path::new(&root).join(workspace)
}

fn read(rel: &str) -> String {
    let path = workspace_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {rel}: {error}"))
}

#[test]
fn every_artifact_writer_commits_its_output_atomically() {
    for writer in WRITERS {
        let text = read(writer);
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
