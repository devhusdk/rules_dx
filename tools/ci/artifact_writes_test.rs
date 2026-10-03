use dx_testing::read_runfiles;

/// Binaries whose whole job is to encode one validated artifact to an output path.
const WRITERS: [&str; 4] = [
    "env/env_shard/src/main.rs",
    "generation/codegen_shard/src/main.rs",
    "quality/evaluator/src/main.rs",
    "quality/runner/src/main.rs",
];

const ATOMIC: &str = "dx_atomic_fs::write_atomic(";
const IN_PLACE: &str = "std::fs::write(";

/// Returns a writer's production half: everything above its test module.
fn production(writer: &str) -> String {
    let text = read_runfiles(writer);
    match text.find("#[cfg(test)]") {
        Some(at) => text[..at].to_owned(),
        None => text,
    }
}

#[test]
fn every_artifact_writer_commits_its_output_atomically() {
    for writer in WRITERS {
        let text = production(writer);
        assert!(
            text.contains(ATOMIC),
            "{writer} does not commit its output with dx_atomic_fs::write_atomic"
        );
        assert!(
            !text.contains(IN_PLACE),
            "{writer} writes its output in place, so a failed run truncates a good artifact"
        );
    }
}
