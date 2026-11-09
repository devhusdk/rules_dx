use std::io::BufRead;
use std::path::Path;

use serde_json::Value;

const TEST_OUTPUT_START: &str = "==================== Test output for ";
const SIGNATURES: &[&str] = &["remote cache", "does not exist remotely", "cachenotfound"];

/// Reports whether Bazel's own output in the stream at `path` holds a remote cache error line.
///
/// Test output blocks are skipped. A missing file is an `io::ErrorKind::NotFound` error.
pub fn stream_remote_cache_error(path: &Path) -> std::io::Result<bool> {
    let reader = std::io::BufReader::new(std::fs::File::open(path)?);
    Ok(has_remote_cache_error(reader))
}

/// Reports whether the progress output of a build event stream holds a remote cache error line.
pub fn has_remote_cache_error(reader: impl BufRead) -> bool {
    let mut text = String::new();
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if !line.contains("\"progress\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(chunk) = event.pointer("/progress/stderr").and_then(Value::as_str) {
            text.push_str(chunk);
        }
    }
    bazel_lines(&text).any(is_cache_error)
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

fn is_cache_error(line: &str) -> bool {
    let Some(rest) = line.trim_start().strip_prefix("ERROR:") else {
        return false;
    };
    let rest = rest.to_ascii_lowercase();
    SIGNATURES.iter().any(|signature| rest.contains(signature))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(stderr: &str) -> String {
        let event = serde_json::json!({"id": {"progress": {}}, "progress": {"stderr": stderr}});
        format!("{event}\n")
    }

    fn found(stderr: &str) -> bool {
        has_remote_cache_error(stream(stderr).as_bytes())
    }

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
        assert!(!has_remote_cache_error(&b"not json\n{\"id\":{}}\n"[..]));
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
        assert!(has_remote_cache_error(text.as_bytes()));
    }

    #[test]
    fn missing_file_is_not_found() {
        let error = stream_remote_cache_error(Path::new("/nonexistent/bep.json")).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }
}
