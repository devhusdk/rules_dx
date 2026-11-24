use crate::proto::{DxCodegenEntry, DxCodegenShard};
use dx_shard_writer::{ShardKind, WriterError};

struct CodegenKind;

impl ShardKind for CodegenKind {
    const PROGRAM: &'static str = "codegen_shard_writer";
    const GROUP_FLAG: &'static str = "language";
    const USAGE: &'static str = "usage: codegen_shard_writer --producer LABEL --language LANG --entry LOGICAL|ROOT|NAMESPACE[|EXEC] [--entry ...] --output OUT";
    const ENTRY_SHAPE: &'static str = crate::ENTRY_SHAPE;

    type Entry = DxCodegenEntry;
    type Shard = DxCodegenShard;
    type Error = crate::Error;

    fn parse_entry(raw: &str) -> Option<DxCodegenEntry> {
        crate::parse_entry(raw)
    }

    fn shard(producer: String, group: String, entries: Vec<DxCodegenEntry>) -> DxCodegenShard {
        crate::shard(producer, group, entries)
    }

    fn encode(shard: &DxCodegenShard) -> Result<Vec<u8>, crate::Error> {
        crate::encode_validated(shard)
    }
}

pub fn utf8_args(args: &[std::ffi::OsString]) -> Result<Vec<String>, WriterError> {
    args.iter()
        .map(|arg| {
            arg.to_str().map(str::to_owned).ok_or_else(|| {
                WriterError::Usage(format!(
                    "argument {arg:?} is not valid UTF-8\n{}",
                    CodegenKind::USAGE
                ))
            })
        })
        .collect()
}

// LCOV_EXCL_START - reason: process shim, issue: 9370, policy: docs/cli/commands/build-test-coverage.md
#[must_use]
pub fn run_process(args: &[String]) -> i32 {
    dx_shard_writer::run_process::<CodegenKind>(args)
}

#[must_use]
pub fn run_os_process(args: &[std::ffi::OsString]) -> i32 {
    match utf8_args(args) {
        Ok(text) => run_process(&text),
        Err(error) => {
            dx_output::write_stderr(
                &mut std::io::stderr(),
                format_args!("{}: {error}\n", CodegenKind::PROGRAM),
            );
            1
        }
    }
}
// LCOV_EXCL_STOP - reason: end process shim, issue: 9370, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode_validated;
    use dx_shard_writer::run;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn out(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join("shard.dxcodegen.pb")
    }

    fn minimal(dir: &tempfile::TempDir, extra: &[&str]) -> Vec<String> {
        let mut argv = args(&[
            "--producer=//gen:alpha",
            "--language=rust",
            "--entry=src/a.rs|src|a",
        ]);
        argv.extend(args(extra));
        argv.push(format!("--output={}", out(dir).display()));
        argv
    }

    fn written(dir: &tempfile::TempDir) -> DxCodegenShard {
        let bytes = std::fs::read(out(dir)).expect("shard written");
        decode_validated(&bytes).expect("written shard decodes")
    }

    #[test]
    fn every_entry_arity_round_trips_through_the_writer() {
        for (raw, logical_path, namespace, exec_path, replaces) in [
            ("src/one.rs|src|one", "src/one.rs", "one", "", ""),
            ("src/b.rs|src|b|out/b.rs", "src/b.rs", "b", "out/b.rs", ""),
            (
                "src/c.rs|src|c|out/c.rs|src/c.rs",
                "src/c.rs",
                "c",
                "out/c.rs",
                "src/c.rs",
            ),
        ] {
            let dir = tempfile::tempdir().expect("scratch");
            run::<CodegenKind>(&minimal(&dir, &[&format!("--entry={raw}")]))
                .unwrap_or_else(|error| panic!("entry {raw:?} must write: {error}"));
            let shard = written(&dir);
            assert_eq!(shard.producer, "//gen:alpha");
            assert_eq!(shard.language, "rust");
            let entry = shard
                .entries
                .into_iter()
                .find(|entry| entry.logical_path == logical_path)
                .unwrap_or_else(|| panic!("entry {raw:?} must survive the round trip"));
            assert_eq!(entry.import_root, "src");
            assert_eq!(entry.namespace, namespace);
            assert_eq!(entry.exec_path, exec_path);
            assert_eq!(entry.replaces, replaces);
            assert!(entry.read_only);
        }
    }

    #[test]
    fn a_bad_entry_arity_names_the_wanted_shape() {
        let dir = tempfile::tempdir().expect("scratch");
        let error = run::<CodegenKind>(&minimal(&dir, &["--entry=only-one-part"]))
            .expect_err("a one-part entry is rejected");
        assert_eq!(
            error.to_string(),
            "bad --entry \"only-one-part\": want LOGICAL_PATH|IMPORT_ROOT|NAMESPACE[|EXEC_PATH[|REPLACES]]"
        );
        assert!(!out(&dir).exists(), "a rejected entry writes no shard");
    }

    #[test]
    fn a_missing_output_prints_the_codegen_usage() {
        let error = run::<CodegenKind>(&args(&[
            "--producer=//gen:alpha",
            "--language=rust",
            "--entry=src/a.rs|src|a",
        ]))
        .expect_err("a missing output is rejected");
        assert_eq!(
            error,
            WriterError::Usage(
                "usage: codegen_shard_writer --producer LABEL --language LANG --entry LOGICAL|ROOT|NAMESPACE[|EXEC] [--entry ...] --output OUT"
                    .to_owned()
            )
        );
    }

    #[test]
    fn an_invalid_producer_surfaces_the_codec_error() {
        let dir = tempfile::tempdir().expect("scratch");
        let error = run::<CodegenKind>(&minimal(&dir, &["--producer=not-a-label"]))
            .expect_err("a producer without // or @ is rejected");
        assert_eq!(
            error,
            WriterError::Codec("invalid producer \"not-a-label\": want a // or @ label".to_owned())
        );
    }

    #[test]
    fn duplicate_logical_paths_are_rejected_by_the_codec() {
        let dir = tempfile::tempdir().expect("scratch");
        let error = run::<CodegenKind>(&minimal(&dir, &["--entry=src/a.rs|src|other"]))
            .expect_err("a duplicate logical path is rejected");
        let expected = crate::Error::DuplicateLogicalPath {
            producer: "//gen:alpha".into(),
            path: "src/a.rs".into(),
        };
        assert_eq!(error, WriterError::Codec(expected.to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_argument_is_rejected() {
        use std::os::unix::ffi::OsStringExt;
        let mut bad = b"--entry=src/".to_vec();
        bad.extend([0xff, 0xfe]);
        bad.extend(b".rs|src|a");
        let argv = vec![
            std::ffi::OsString::from("--producer=//gen:alpha"),
            std::ffi::OsString::from("--language=rust"),
            std::ffi::OsString::from_vec(bad),
        ];
        let error = utf8_args(&argv).expect_err("non-UTF-8 is rejected");
        assert!(
            matches!(&error, WriterError::Usage(text) if text.contains("is not valid UTF-8")),
            "got {error:?}"
        );
    }

    #[test]
    fn an_unwritable_output_names_the_path() {
        let dir = tempfile::tempdir().expect("scratch");
        let blocked = dir.path().join("blocked");
        std::fs::create_dir_all(&blocked).expect("a directory blocks the write");
        let argv = args(&[
            "--producer=//gen:alpha",
            "--language=rust",
            "--entry=src/a.rs|src|a",
            &format!("--output={}", blocked.display()),
        ]);
        let error = run::<CodegenKind>(&argv).expect_err("writing over a directory fails");
        let named = format!("cannot write {}: ", blocked.display());
        assert!(
            matches!(&error, WriterError::Io(detail) if detail.starts_with(&named)),
            "want an io error naming the directory, got {error:?}"
        );
    }
}
