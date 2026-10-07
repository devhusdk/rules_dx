use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;

use serde_json::Value;

use super::{
    file_uri_to_path, is_bytestream_uri, malformed, missing_test_output, testlog_path, BepError,
    OutputLocations, TestOutputIdentity,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestOutputFile {
    pub label: String,
    pub name: String,
    pub exec_path: PathBuf,
    pub run: u32,
    pub shard: u32,
    pub attempt: u32,
}

struct TestResultId<'a> {
    label: &'a str,
    run: u32,
    shard: u32,
    attempt: u32,
}

impl<'a> TestResultId<'a> {
    fn parse(id: &'a serde_json::Map<String, Value>, line: u64) -> Result<Self, BepError> {
        let label = id
            .get("label")
            .and_then(Value::as_str)
            .ok_or_else(|| malformed(line, "id.testResult.label", "test result without label"))?;
        Ok(TestResultId {
            label,
            run: test_index(id, "run", "id.testResult.run", line)?,
            shard: test_index(id, "shard", "id.testResult.shard", line)?,
            attempt: test_index(id, "attempt", "id.testResult.attempt", line)?,
        })
    }
}

struct TestActionOutput<'a> {
    name: &'a str,
    uri: &'a str,
    path_prefix: Vec<String>,
}

impl<'a> TestActionOutput<'a> {
    fn parse(file: &'a Value, index: usize, line: u64) -> Result<Self, BepError> {
        let base = format!("testResult.testActionOutput[{index}]");
        let entry = file
            .as_object()
            .ok_or_else(|| malformed(line, &base, "test action output must be an object"))?;
        let name = entry.get("name").and_then(Value::as_str).ok_or_else(|| {
            malformed(
                line,
                &format!("{base}.name"),
                "test action output without name",
            )
        })?;
        let uri = entry.get("uri").and_then(Value::as_str).ok_or_else(|| {
            malformed(
                line,
                &format!("{base}.uri"),
                "test action output without uri",
            )
        })?;
        Ok(TestActionOutput {
            name,
            uri,
            path_prefix: path_prefix(entry, line, &base)?,
        })
    }
}

fn path_prefix(
    file: &serde_json::Map<String, Value>,
    line: u64,
    base: &str,
) -> Result<Vec<String>, BepError> {
    let Some(value) = file.get("pathPrefix") else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| malformed(line, &format!("{base}.pathPrefix"), "must be an array"))?;
    values
        .iter()
        .map(|item| {
            item.as_str().map(str::to_owned).ok_or_else(|| {
                malformed(
                    line,
                    &format!("{base}.pathPrefix"),
                    "entries must be strings",
                )
            })
        })
        .collect()
}

struct PendingFile {
    name: String,
    uri: String,
    path_prefix: Vec<String>,
}

struct PendingResult {
    label: String,
    run: u32,
    shard: u32,
    attempt: u32,
    files: Vec<PendingFile>,
}

pub fn collect_test_outputs(
    reader: impl BufRead,
    locations: Option<&OutputLocations>,
) -> Result<Vec<TestOutputFile>, BepError> {
    let mut pending: Vec<PendingResult> = Vec::new();
    let mut shard_counts: HashMap<String, u32> = HashMap::new();
    for (index, line) in reader.lines().enumerate() {
        let line_no = (index + 1) as u64;
        let text = line.map_err(|e| BepError::MalformedEvent {
            line: line_no,
            reason: format!("unreadable stream line: {e}"),
        })?;
        let event: Value = serde_json::from_str(&text).map_err(|e| BepError::MalformedEvent {
            line: line_no,
            reason: format!("invalid JSON: {e}"),
        })?;
        let object = event.as_object().ok_or_else(|| BepError::MalformedEvent {
            line: line_no,
            reason: "event must be a JSON object".to_owned(),
        })?;
        let id = object.get("id").and_then(Value::as_object).ok_or_else(|| {
            BepError::MalformedEvent {
                line: line_no,
                reason: "event without id".to_owned(),
            }
        })?;
        if let Some(summary_id) = id.get("testSummary").and_then(Value::as_object) {
            record_shard_count(summary_id, object, line_no, &mut shard_counts)?;
            continue;
        }
        let result_id = id.get("testResult").and_then(Value::as_object);
        let Some(result_id) = result_id else {
            continue;
        };
        let label = TestResultId::parse(result_id, line_no)?;
        let result = object.get("testResult").and_then(Value::as_object);
        let Some(files) = result.and_then(|result| result.get("testActionOutput")) else {
            continue;
        };
        let files = files.as_array().ok_or_else(|| {
            malformed(
                line_no,
                "testResult.testActionOutput",
                "testActionOutput must be an array",
            )
        })?;
        let mut pending_files = Vec::with_capacity(files.len());
        for (index, file) in files.iter().enumerate() {
            let entry = TestActionOutput::parse(file, index, line_no)?;
            pending_files.push(PendingFile {
                name: entry.name.to_owned(),
                uri: entry.uri.to_owned(),
                path_prefix: entry.path_prefix,
            });
        }
        pending.push(PendingResult {
            label: label.label.to_owned(),
            run: label.run,
            shard: label.shard,
            attempt: label.attempt,
            files: pending_files,
        });
    }
    let run_totals = run_totals(&pending);
    let attempt_totals = attempt_totals(&pending);
    let mut outputs = Vec::new();
    for result in &pending {
        let run_total = run_totals.get(result.label.as_str()).copied().unwrap_or(1);
        let shard_total = shard_counts
            .get(&result.label)
            .copied()
            .unwrap_or(0)
            .max(shard_max(&pending, &result.label))
            .max(1);
        let attempt_total = attempt_totals
            .get(&(result.label.as_str(), result.run, result.shard))
            .copied()
            .unwrap_or(1);
        let identity = TestOutputIdentity {
            run: result.run,
            shard: result.shard,
            attempt: result.attempt,
            run_total,
            shard_total,
            attempt_total,
        };
        for file in &result.files {
            let exec_path = if is_bytestream_uri(&file.uri) {
                resolve_bytestream(locations, &result.label, file, identity)?
            } else {
                file_uri_to_path(&file.uri)?
            };
            outputs.push(TestOutputFile {
                label: result.label.clone(),
                name: file.name.clone(),
                exec_path,
                run: result.run,
                shard: result.shard,
                attempt: result.attempt,
            });
        }
    }
    outputs.sort_by(|a, b| {
        a.label
            .as_bytes()
            .cmp(b.label.as_bytes())
            .then(a.name.as_bytes().cmp(b.name.as_bytes()))
            .then(a.run.cmp(&b.run))
            .then(a.shard.cmp(&b.shard))
            .then(a.attempt.cmp(&b.attempt))
    });
    Ok(outputs)
}

fn resolve_bytestream(
    locations: Option<&OutputLocations>,
    label: &str,
    file: &PendingFile,
    identity: TestOutputIdentity,
) -> Result<PathBuf, BepError> {
    let Some(locations) = locations else {
        return Err(missing_test_output(
            label,
            &file.name,
            identity,
            "no output roots for bytestream test outputs",
        ));
    };
    if !file.path_prefix.is_empty() {
        if let Some(root) = locations.execution_root() {
            let mut path = root.to_path_buf();
            for part in &file.path_prefix {
                path.push(part);
            }
            path.push(&file.name);
            return Ok(path);
        }
    }
    testlog_path(locations, label, &file.name, identity)
}

fn record_shard_count(
    summary_id: &serde_json::Map<String, Value>,
    object: &serde_json::Map<String, Value>,
    line: u64,
    shard_counts: &mut HashMap<String, u32>,
) -> Result<(), BepError> {
    let label = summary_id
        .get("label")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(line, "id.testSummary.label", "test summary without label"))?;
    let summary = object.get("testSummary").and_then(Value::as_object);
    let Some(count) = summary.and_then(|summary| summary.get("shardCount")) else {
        return Ok(());
    };
    let raw = count.as_u64().ok_or_else(|| {
        malformed(
            line,
            "testSummary.shardCount",
            "shard count must be a non-negative integer",
        )
    })?;
    let count = u32::try_from(raw).map_err(|_| {
        malformed(
            line,
            "testSummary.shardCount",
            "shard count must be a non-negative integer",
        )
    })?;
    if count > 0 {
        let seen = shard_counts.entry(label.to_owned()).or_insert(0);
        *seen = (*seen).max(count);
    }
    Ok(())
}

fn run_totals(pending: &[PendingResult]) -> HashMap<&str, u32> {
    let mut totals: HashMap<&str, u32> = HashMap::new();
    for result in pending {
        let seen = totals.entry(result.label.as_str()).or_insert(0);
        *seen = (*seen).max(result.run);
    }
    totals
}

fn attempt_totals(pending: &[PendingResult]) -> HashMap<(&str, u32, u32), u32> {
    let mut totals: HashMap<(&str, u32, u32), u32> = HashMap::new();
    for result in pending {
        let key = (result.label.as_str(), result.run, result.shard);
        let seen = totals.entry(key).or_insert(0);
        *seen = (*seen).max(result.attempt);
    }
    totals
}

fn shard_max(pending: &[PendingResult], label: &str) -> u32 {
    pending
        .iter()
        .filter(|result| result.label == label)
        .map(|result| result.shard)
        .max()
        .unwrap_or(0)
}

fn test_index(
    result_id: &serde_json::Map<String, Value>,
    field: &str,
    path: &str,
    line: u64,
) -> Result<u32, BepError> {
    match result_id.get(field) {
        None => Ok(1),
        Some(value) => {
            let raw = value.as_u64().ok_or_else(|| {
                malformed(
                    line,
                    path,
                    &format!("test result {field} must be a positive integer"),
                )
            })?;
            u32::try_from(raw)
                .ok()
                .filter(|index| *index >= 1)
                .ok_or_else(|| {
                    malformed(
                        line,
                        path,
                        &format!("test result {field} must be a positive integer"),
                    )
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn test_result(label: &str, outputs: &[(&str, &str)]) -> String {
        let entries = outputs
            .iter()
            .map(|(name, uri)| format!(r#"{{"name":{name:?},"uri":{uri:?}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"id":{{"testResult":{{"label":{label:?},"run":1,"shard":1,"attempt":1}}}},"testResult":{{"status":"PASSED","testActionOutput":[{entries}]}}}}"#
        )
    }

    /// A hand-written file URI naming one path under a root this host can resolve.
    fn out_uri(rel: &str) -> String {
        if cfg!(windows) {
            format!("file:///C:/out/{rel}")
        } else {
            format!("file:///out/{rel}")
        }
    }

    /// The host path one fixture root plus its path parts name.
    fn want_path(root: &str, parts: &[&str]) -> PathBuf {
        let mut path = PathBuf::from(root);
        for part in parts {
            path.push(part);
        }
        path
    }

    /// The host path one slash-separated relative name under the fixture root names.
    fn out_path(rel: &str) -> PathBuf {
        let root = if cfg!(windows) { r"C:\out" } else { "/out" };
        want_path(root, &rel.split('/').collect::<Vec<_>>())
    }

    #[test]
    fn test_outputs_collect_sorted_records() {
        let stream = [
            test_result(
                "//z:t",
                &[
                    ("test.xml", out_uri("z/test.xml").as_str()),
                    ("test.log", out_uri("z/test.log").as_str()),
                ],
            ),
            r#"{"id":{"targetCompleted":{"label":"//z:t"}},"completed":{"success":true}}"#
                .to_owned(),
            test_result("//a:t", &[("test.xml", out_uri("a/test.xml").as_str())]),
        ]
        .join("\n");
        let got = collect_test_outputs(Cursor::new(stream), None).expect("collect");
        assert_eq!(
            got,
            vec![
                TestOutputFile {
                    label: "//a:t".to_owned(),
                    name: "test.xml".to_owned(),
                    exec_path: out_path("a/test.xml"),
                    run: 1,
                    shard: 1,
                    attempt: 1,
                },
                TestOutputFile {
                    label: "//z:t".to_owned(),
                    name: "test.log".to_owned(),
                    exec_path: out_path("z/test.log"),
                    run: 1,
                    shard: 1,
                    attempt: 1,
                },
                TestOutputFile {
                    label: "//z:t".to_owned(),
                    name: "test.xml".to_owned(),
                    exec_path: out_path("z/test.xml"),
                    run: 1,
                    shard: 1,
                    attempt: 1,
                },
            ]
        );
    }

    fn test_result_with_identity(
        label: &str,
        run: u32,
        shard: u32,
        attempt: u32,
        outputs: &[(&str, &str)],
    ) -> String {
        let entries = outputs
            .iter()
            .map(|(name, uri)| format!(r#"{{"name":{name:?},"uri":{uri:?}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            r#"{{"id":{{"testResult":{{"label":{label:?},"run":{run},"shard":{shard},"attempt":{attempt}}}}},"testResult":{{"status":"PASSED","testActionOutput":[{entries}]}}}}"#
        )
    }

    #[test]
    fn test_outputs_capture_run_shard_attempt() {
        let stream = [
            test_result_with_identity("//a:t", 2, 3, 4, &[("test.xml", out_uri("a.xml").as_str())]),
            test_result_with_identity("//a:t", 1, 1, 2, &[("test.xml", out_uri("b.xml").as_str())]),
            test_result_with_identity("//a:t", 1, 1, 1, &[("test.xml", out_uri("c.xml").as_str())]),
        ]
        .join("\n");
        let got = collect_test_outputs(Cursor::new(stream), None).expect("collect");
        assert_eq!(
            got.iter()
                .map(|output| (output.run, output.shard, output.attempt))
                .collect::<Vec<_>>(),
            vec![(1, 1, 1), (1, 1, 2), (2, 3, 4)],
            "records sort by run, shard, then attempt"
        );
        assert_eq!(got[0].exec_path, out_path("c.xml"));
        assert_eq!(got[2].exec_path, out_path("a.xml"));
    }

    #[test]
    fn test_outputs_default_missing_identity_to_one() {
        let stream = format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
            out_uri("a.xml")
        );
        let got = collect_test_outputs(Cursor::new(stream), None).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].run, got[0].shard, got[0].attempt), (1, 1, 1));
    }

    #[test]
    fn test_outputs_reject_non_positive_identity() {
        for field in ["run", "shard", "attempt"] {
            let stream = format!(
                r#"{{"id":{{"testResult":{{"label":"//a:t","{field}":0}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
                out_uri("a.xml")
            );
            assert!(
                collect_test_outputs(Cursor::new(stream), None).is_err(),
                "{field}=0 must fail"
            );
            let stream = format!(
                r#"{{"id":{{"testResult":{{"label":"//a:t","{field}":"one"}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"{}"}}]}}}}"#,
                out_uri("a.xml")
            );
            assert!(
                collect_test_outputs(Cursor::new(stream), None).is_err(),
                "{field} string must fail"
            );
        }
    }

    #[test]
    fn test_outputs_without_entries_or_results_are_ignored() {
        let stream = [
            r#"{"id":{"progress":{}}}"#.to_owned(),
            test_result("//a:t", &[]),
        ]
        .join("\n");
        let got = collect_test_outputs(Cursor::new(stream), None).expect("collect");
        assert!(got.is_empty());
    }

    #[test]
    fn test_output_shape_failures_fail_collection() {
        for stream in [
            "not json".to_owned(),
            r#"{"no-id":true}"#.to_owned(),
            r#"{"id":{"testResult":{}},"testResult":{"testActionOutput":[]}}"#.to_owned(),
            test_result("//a:t", &[("test.xml", "bytestream://remote/1")]),
            "[]".to_owned(),
            r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":{}}}"#
                .to_owned(),
            r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":["x"]}}"#
                .to_owned(),
            format!(
                r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"testActionOutput":[{{"uri":"{}"}}]}}}}"#,
                out_uri("a.xml")
            )
                .to_owned(),
            r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":[{"name":"test.xml"}]}}"#
                .to_owned(),
        ] {
            assert!(
                collect_test_outputs(Cursor::new(stream), None).is_err(),
                "must fail closed"
            );
        }
    }

    #[test]
    fn test_output_malformed_reasons_carry_json_paths() {
        let cases = [
            (
                r#"{"id":{"testResult":{}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.label",
                "test result without label",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t","run":0}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.run",
                "must be a positive integer",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t","shard":"one"}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.shard",
                "must be a positive integer",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t","attempt":4294967296}},"testResult":{"testActionOutput":[]}}"#,
                "id.testResult.attempt",
                "must be a positive integer",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":{}}}"#,
                "testResult.testActionOutput",
                "must be an array",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":["x"]}}"#,
                "testResult.testActionOutput[0]",
                "must be an object",
            ),
            (
                &format!(
                    r#"{{"id":{{"testResult":{{"label":"//a:t"}}}},"testResult":{{"testActionOutput":[{{"uri":"{}"}}]}}}}"#,
                    out_uri("a.xml")
                ),
                "testResult.testActionOutput[0].name",
                "without name",
            ),
            (
                r#"{"id":{"testResult":{"label":"//a:t"}},"testResult":{"testActionOutput":[{"name":"test.xml"}]}}"#,
                "testResult.testActionOutput[0].uri",
                "without uri",
            ),
        ];
        for (stream, path, wording) in cases {
            let err =
                collect_test_outputs(Cursor::new(stream), None).expect_err("must fail closed");
            let BepError::MalformedEvent { line, reason } = err else {
                panic!("want MalformedEvent, got {err:?}");
            };
            assert_eq!(line, 1);
            assert!(
                reason.contains(path),
                "reason {reason:?} must name JSON path {path}"
            );
            assert!(
                reason.contains(wording),
                "reason {reason:?} must keep wording {wording:?}"
            );
        }
    }

    #[test]
    fn test_outputs_unreadable_stream_fails() {
        struct FailRead;
        impl std::io::Read for FailRead {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "boom"))
            }
        }
        assert!(collect_test_outputs(std::io::BufReader::new(FailRead), None).is_err());
    }

    fn test_summary(label: &str, shard_count: Option<u32>) -> String {
        let count = shard_count
            .map(|count| format!(r#","shardCount":{count}"#))
            .unwrap_or_default();
        format!(
            r#"{{"id":{{"testSummary":{{"label":{label:?}}}}},"testSummary":{{"status":"PASSED"{count}}}}}"#
        )
    }

    #[test]
    fn bytestream_test_outputs_resolve_through_the_workspace_fallback() {
        use std::path::Path;
        let ws = Path::new("/ws");
        let stream = [test_result(
            "//cli/bep:dx_bep_test",
            &[
                ("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10"),
                (
                    "test.lcov",
                    "bytestream://remote.buildbuddy.io/blobs/def/20",
                ),
            ],
        )]
        .join("\n");
        let locations = OutputLocations::new(ws);
        let got =
            collect_test_outputs(Cursor::new(stream), Some(&locations)).expect("workspace root");
        assert_eq!(got.len(), 2);
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/ws",
                &["bazel-testlogs", "cli/bep", "dx_bep_test", "coverage.dat"]
            )
        );
        assert_eq!(got[0].name, "test.lcov");
        assert_eq!(
            got[1].exec_path,
            want_path(
                "/ws",
                &["bazel-testlogs", "cli/bep", "dx_bep_test", "test.xml"]
            )
        );
    }

    #[test]
    fn bytestream_test_outputs_resolve_from_reported_roots() {
        use std::path::Path;
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(
            b"bazel-testlogs: /out/k8-fastbuild/testlogs\nexecution_root: /out/execroot\n",
        );
        let stream = [
            test_summary("//a:t", Some(3)),
            test_result_with_identity(
                "//a:t",
                2,
                3,
                2,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
            ),
            test_result_with_identity(
                "//a:t",
                2,
                3,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/def/20")],
            ),
            test_result_with_identity(
                "@@dep+//pkg:t",
                1,
                1,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/ghi/30")],
            ),
        ]
        .join("\n");
        let got =
            collect_test_outputs(Cursor::new(stream), Some(&locations)).expect("reported roots");
        assert_eq!(got.len(), 3);
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/k8-fastbuild/testlogs",
                &[
                    "a",
                    "t",
                    "shard_3_of_3_run_2_of_2",
                    "test_attempts",
                    "attempt_1.xml"
                ]
            ),
            "a superseded attempt reads its own file"
        );
        assert_eq!(
            got[1].exec_path,
            want_path(
                "/out/k8-fastbuild/testlogs",
                &["a", "t", "shard_3_of_3_run_2_of_2", "test.xml"]
            ),
            "the final attempt reads the canonical file"
        );
        assert_eq!(
            got[2].exec_path,
            want_path(
                "/out/k8-fastbuild/testlogs",
                &["external", "dep+", "pkg", "t", "test.xml"]
            )
        );
        assert_ne!(got[0].exec_path, got[1].exec_path);
    }

    #[test]
    fn bytestream_test_outputs_prefer_path_prefix_under_the_execution_root() {
        use std::path::Path;
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(
            b"bazel-testlogs: /out/k8-fastbuild/testlogs\nexecution_root: /out/execroot\n",
        );
        let stream = [format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t","run":1,"shard":1,"attempt":1}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"bytestream://remote.buildbuddy.io/blobs/abc/10","pathPrefix":["bazel-out","k8-fastbuild","testlogs","a","t"]}}]}}}}"#
        )]
        .join("\n");
        let got = collect_test_outputs(Cursor::new(stream), Some(&locations)).expect("path prefix");
        assert_eq!(
            got[0].exec_path,
            want_path(
                "/out/execroot",
                &[
                    "bazel-out",
                    "k8-fastbuild",
                    "testlogs",
                    "a",
                    "t",
                    "test.xml"
                ]
            ),
            "path prefixes are execution-root relative"
        );
        let mut workspace_only = OutputLocations::new(Path::new("/ws"));
        workspace_only.apply_bazel_info(b"bazel-testlogs: /out/k8-fastbuild/testlogs\n");
        let stream = [format!(
            r#"{{"id":{{"testResult":{{"label":"//a:t","run":1,"shard":1,"attempt":1}}}},"testResult":{{"status":"PASSED","testActionOutput":[{{"name":"test.xml","uri":"bytestream://remote.buildbuddy.io/blobs/abc/10","pathPrefix":["bazel-out","k8-fastbuild","testlogs","a","t"]}}]}}}}"#
        )]
        .join("\n");
        let got =
            collect_test_outputs(Cursor::new(stream), Some(&workspace_only)).expect("no execroot");
        assert_eq!(
            got[0].exec_path,
            want_path("/out/k8-fastbuild/testlogs", &["a", "t", "test.xml"]),
            "without an execution root the testlogs root still resolves"
        );
    }

    #[test]
    fn bytestream_test_outputs_without_roots_name_label_and_identity() {
        let stream = [test_result_with_identity(
            "//a:t",
            2,
            3,
            1,
            &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
        )]
        .join("\n");
        let err = collect_test_outputs(Cursor::new(stream), None).expect_err("no roots must fail");
        let message = err.to_string();
        assert!(message.contains("//a:t"), "{message}");
        assert!(message.contains("run 2"), "{message}");
        assert!(message.contains("shard 3"), "{message}");
        assert!(message.contains("attempt 1"), "{message}");
        assert!(message.contains("test.xml"), "{message}");
    }

    #[test]
    fn shard_counts_shape_paths_even_when_one_shard_reports() {
        use std::path::Path;
        let mut locations = OutputLocations::new(Path::new("/ws"));
        locations.apply_bazel_info(b"bazel-testlogs: /out/testlogs\n");
        let stream = [
            test_summary("//a:t", Some(3)),
            test_result_with_identity(
                "//a:t",
                1,
                2,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
            ),
        ]
        .join("\n");
        let got = collect_test_outputs(Cursor::new(stream), Some(&locations)).expect("summary");
        assert_eq!(
            got[0].exec_path,
            want_path("/out/testlogs", &["a", "t", "shard_2_of_3", "test.xml"]),
            "testSummary.shardCount supplies the shard total"
        );
        let stream = [
            test_result_with_identity(
                "//a:t",
                1,
                1,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/abc/10")],
            ),
            test_result_with_identity(
                "//a:t",
                3,
                1,
                1,
                &[("test.xml", "bytestream://remote.buildbuddy.io/blobs/def/20")],
            ),
        ]
        .join("\n");
        let got = collect_test_outputs(Cursor::new(stream), Some(&locations)).expect("runs");
        assert_eq!(
            got[1].exec_path,
            want_path("/out/testlogs", &["a", "t", "run_3_of_3", "test.xml"]),
            "the highest reported run is the run total"
        );
    }
}
