use crate::proto::{DxEnvEntry, DxEnvShard};
use dx_shard_writer::ShardKind;

struct EnvKind;

impl ShardKind for EnvKind {
    const PROGRAM: &'static str = "env_shard_writer";
    const GROUP_FLAG: &'static str = "integration";
    const USAGE: &'static str = "usage: env_shard_writer --producer LABEL --integration LANG --entry KEY|VALUE[|EXEC] [--entry ...] --output OUT";
    const ENTRY_SHAPE: &'static str = crate::ENTRY_SHAPE;

    type Entry = DxEnvEntry;
    type Shard = DxEnvShard;
    type Error = crate::Error;

    fn parse_entry(raw: &str) -> Option<DxEnvEntry> {
        crate::parse_entry(raw)
    }

    fn shard(producer: String, group: String, entries: Vec<DxEnvEntry>) -> DxEnvShard {
        crate::shard(producer, group, entries)
    }

    fn encode(shard: &DxEnvShard) -> Result<Vec<u8>, crate::Error> {
        crate::encode_validated(shard)
    }
}

// LCOV_EXCL_START - reason: process shim, issue: 9370, policy: docs/cli/commands/build-test-coverage.md
/// Runs the writer on the given arguments and returns the exit code.
pub fn run_process(args: Vec<String>) -> i32 {
    dx_shard_writer::run_process::<EnvKind>(args)
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 9370, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_validated;
    use dx_shard_writer::{WriterError, run};

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn minimal(dir: &tempfile::TempDir, extra: &[&str]) -> Vec<String> {
        let mut argv = args(&[
            "--producer=//env:alpha",
            "--integration=rust",
            "--entry=TOKEN|secret",
        ]);
        argv.extend(args(extra));
        argv.push(format!(
            "--output={}",
            dir.path().join("shard.dxenv.pb").display()
        ));
        argv
    }

    fn written(dir: &tempfile::TempDir) -> DxEnvShard {
        let bytes = std::fs::read(dir.path().join("shard.dxenv.pb")).expect("shard written");
        decode_validated(&bytes).expect("written shard decodes")
    }

    #[test]
    fn both_entry_arities_round_trip_through_the_writer() {
        for (raw, key, value, exec_path) in [
            ("ONE|1", "ONE", "1", ""),
            ("TWO|2|out/two.txt", "TWO", "2", "out/two.txt"),
        ] {
            let dir = tempfile::tempdir().expect("scratch");
            run::<EnvKind>(&minimal(&dir, &[&format!("--entry={raw}")]))
                .unwrap_or_else(|error| panic!("entry {raw:?} must write: {error}"));
            let shard = written(&dir);
            assert_eq!(shard.producer, "//env:alpha");
            assert_eq!(shard.integration, "rust");
            let entry = shard
                .entries
                .into_iter()
                .find(|entry| entry.key == key)
                .unwrap_or_else(|| panic!("entry {raw:?} must survive the round trip"));
            assert_eq!(entry.value, value);
            assert_eq!(entry.exec_path, exec_path);
        }
    }

    #[test]
    fn a_bad_entry_arity_names_the_wanted_shape() {
        let dir = tempfile::tempdir().expect("scratch");
        let error = run::<EnvKind>(&minimal(&dir, &["--entry=only-one-part"]))
            .expect_err("a one-part entry is rejected");
        assert_eq!(
            error.to_string(),
            "bad --entry \"only-one-part\": want KEY|VALUE[|EXEC_PATH]"
        );
    }

    #[test]
    fn a_missing_output_prints_the_env_usage() {
        let error = run::<EnvKind>(&args(&[
            "--producer=//env:alpha",
            "--integration=rust",
            "--entry=TOKEN|secret",
        ]))
        .expect_err("a missing output is rejected");
        assert_eq!(
            error,
            WriterError::Usage(
                "usage: env_shard_writer --producer LABEL --integration LANG --entry KEY|VALUE[|EXEC] [--entry ...] --output OUT"
                    .to_owned()
            )
        );
    }

    #[test]
    fn an_invalid_producer_surfaces_the_codec_error() {
        let dir = tempfile::tempdir().expect("scratch");
        let error = run::<EnvKind>(&minimal(&dir, &["--producer=not-a-label"]))
            .expect_err("a producer without // or @ is rejected");
        assert_eq!(
            error,
            WriterError::Codec("invalid producer \"not-a-label\": want a // or @ label".to_owned())
        );
    }

    #[test]
    fn duplicate_keys_are_rejected_by_the_codec() {
        let dir = tempfile::tempdir().expect("scratch");
        let error = run::<EnvKind>(&minimal(&dir, &["--entry=TOKEN|other"]))
            .expect_err("a duplicate key is rejected");
        assert_eq!(
            error,
            WriterError::Codec("duplicate key for //env:alpha \"TOKEN\"".to_owned())
        );
    }
}
