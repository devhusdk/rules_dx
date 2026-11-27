use std::io::BufRead;
use std::path::{Path, PathBuf};

use serde_json::Value;

const TEST_OUTPUT_START: &str = "==================== Test output for ";
const SIGNATURES: &[&str] = &["remote cache", "does not exist remotely", "cachenotfound"];
const MISSING_FETCH_TARGET: [&str; 2] = ["failed to fetch blobs", "(no such file or directory)"];
const DANGLING_OUTPUT: &str = "is a dangling symbolic link";
const SYMLINK_MNEMONIC: &str = "Symlink";
const EVENT_KEYS: [&str; 3] = ["\"progress\"", "\"localExecRoot\"", "\"action\""];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheError {
    Remote,
    LostOutputs { exec_root: PathBuf },
}

pub fn stream_cache_error(path: &Path) -> std::io::Result<Option<CacheError>> {
    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    Ok(cache_error(reader))
}

pub fn cache_error(reader: impl BufRead) -> Option<CacheError> {
    let mut text = String::new();
    let mut exec_root = None;
    let mut symlink_failed = false;
    for line in crate::readable_lines(reader) {
        if !EVENT_KEYS.iter().any(|key| line.contains(key)) {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(chunk) = event.pointer("/progress/stderr").and_then(Value::as_str) {
            text.push_str(chunk);
        }
        if let Some(root) = event
            .pointer("/workspaceInfo/localExecRoot")
            .and_then(Value::as_str)
        {
            exec_root = Some(PathBuf::from(root));
        }
        if let Some(action) = event.get("action") {
            symlink_failed |= action.get("success").and_then(Value::as_bool) != Some(true)
                && action.get("type").and_then(Value::as_str) == Some(SYMLINK_MNEMONIC);
        }
    }
    let lost = bazel_lines(&text).any(|line| lost_output(line, symlink_failed));
    match exec_root {
        Some(exec_root) if lost => Some(CacheError::LostOutputs { exec_root }),
        _ => bazel_lines(&text)
            .any(is_cache_error)
            .then_some(CacheError::Remote),
    }
}

fn bazel_lines(text: &str) -> impl Iterator<Item = &str> {
    let mut in_test_output = false;
    text.lines().filter(move |line| {
        if line.starts_with(TEST_OUTPUT_START) {
            in_test_output = true;
            return false;
        }
        if in_test_output {
            if line.len() >= 40 && line.bytes().all(|byte| byte == b'=') {
                in_test_output = false;
            }
            return false;
        }
        true
    })
}

fn error_text(line: &str) -> Option<String> {
    line.trim_start()
        .strip_prefix("ERROR:")
        .map(str::to_ascii_lowercase)
}

fn is_cache_error(line: &str) -> bool {
    error_text(line).is_some_and(|rest| SIGNATURES.iter().any(|signature| rest.contains(signature)))
}

fn lost_output(line: &str, symlink_failed: bool) -> bool {
    error_text(line).is_some_and(|rest| {
        MISSING_FETCH_TARGET.iter().all(|part| rest.contains(part))
            || (symlink_failed && rest.contains(DANGLING_OUTPUT))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(stderr: &str) -> String {
        let event = serde_json::json!({"id": {"progress": {}}, "progress": {"stderr": stderr}});
        format!("{event}\n")
    }

    fn found(stderr: &str) -> bool {
        cache_error(stream(stderr).as_bytes()) == Some(CacheError::Remote)
    }

    fn workspace(exec_root: &str) -> String {
        let event = serde_json::json!({"id": {"workspace": {}}, "workspaceInfo": {"localExecRoot": exec_root}});
        format!("{event}\n")
    }

    fn failed_action(mnemonic: &str) -> String {
        let event = serde_json::json!({"id": {"actionCompleted": {}}, "action": {"success": false, "type": mnemonic}});
        format!("{event}\n")
    }

    const ENOENT_FETCH: &str = "ERROR: /w/BUILD:1:1: Compiling a.c failed: Failed to fetch blobs because of a remote cache error.: [unix_jni.cc:297] /ob/execroot/_main/bazel-out/k8-fastbuild-exec/bin/tool (No such file or directory)\n";
    const DANGLING: &str = "ERROR: /ob/external/x/BUILD.bazel:1:1: output 'external/x/crt1.o' is a dangling symbolic link\n";

    #[test]
    fn error_lines_naming_the_remote_cache_count() {
        assert!(found("ERROR: Remote Cache: connection refused\n"));
        assert!(found(
            "INFO: x\nERROR: /w/BUILD:1:1: Compiling x failed: Remote Cache: gone\n"
        ));
        assert!(found(
            "ERROR: Failed to fetch file with hash 'ab' because it does not exist remotely.\n"
        ));
    }

    #[test]
    fn other_lines_do_not() {
        assert!(!found(""));
        assert!(!found("INFO: Remote Cache hit rate 90%\n"));
        assert!(!found("ERROR: /w/BUILD:1:1: Compiling x failed\n"));
        assert!(!cache_error(&b"not json\n{\"id\":{}}\n"[..]).is_some());
    }

    #[test]
    fn test_output_is_skipped() {
        let eq = "=".repeat(80);
        let noisy = format!(
            "{TEST_OUTPUT_START}//a:t\nERROR: Remote Cache: from the test\n{eq}\nINFO: done\n"
        );
        assert!(!found(&noisy));
        let after = format!("{noisy}ERROR: Remote Cache: real\n");
        assert!(found(&after));
    }

    #[test]
    fn chunks_join_across_events() {
        let mut text = stream("ERROR: Remote ");
        text.push_str(&stream("Cache: down\n"));
        assert_eq!(cache_error(text.as_bytes()), Some(CacheError::Remote));
    }

    #[test]
    fn missing_file_is_not_found() {
        let error = stream_cache_error(Path::new("/nonexistent/bep.json")).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn a_non_utf8_line_does_not_hide_a_later_cache_error() {
        let mut bytes = b"\xff\xfe\n".to_vec();
        bytes.extend_from_slice(stream("ERROR: remote cache down\n").as_bytes());
        assert_eq!(cache_error(bytes.as_slice()), Some(CacheError::Remote));
    }

    #[test]
    fn a_fetch_into_a_missing_directory_is_a_lost_output() {
        let lost = Some(CacheError::LostOutputs {
            exec_root: PathBuf::from("/ob/execroot/_main"),
        });
        let text = format!(
            "{}{}",
            workspace("/ob/execroot/_main"),
            stream(ENOENT_FETCH)
        );
        assert_eq!(cache_error(text.as_bytes()), lost);
        assert!(found(ENOENT_FETCH));
    }

    #[test]
    fn only_a_failed_symlink_action_makes_a_dangling_output_lost() {
        let root = workspace("/ob/execroot/_main");
        let symlink = format!("{root}{}{}", failed_action("Symlink"), stream(DANGLING));
        assert!(matches!(
            cache_error(symlink.as_bytes()),
            Some(CacheError::LostOutputs { .. })
        ));
        let genrule = format!("{root}{}{}", failed_action("Genrule"), stream(DANGLING));
        assert_eq!(cache_error(genrule.as_bytes()), None);
    }
}
