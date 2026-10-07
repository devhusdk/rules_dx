use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

use serde_json::Value;

use super::{
    file_uri_to_path, is_bytestream_uri, local_path_for_bep_file, ArtifactReader, BepError,
    CollectedArtifact, CollectorConfig, TargetOutput,
};

struct RawFile {
    line: u64,
    uri: Option<String>,
    name: Option<String>,
    path_prefix: Vec<String>,
}

struct RawSet {
    line: u64,
    files: Vec<RawFile>,
    children: Vec<String>,
}

struct PendingTarget {
    line: u64,
    label: String,
    success: bool,
    set_ids: Vec<String>,
}

fn named_set_id(id: &serde_json::Map<String, Value>) -> Option<&str> {
    for key in ["namedSet", "namedSetOfFiles"] {
        if let Some(set_id) = id
            .get(key)
            .and_then(Value::as_object)
            .and_then(|named| named.get("id"))
            .and_then(Value::as_str)
        {
            return Some(set_id);
        }
    }
    None
}

pub fn collect(
    reader: impl BufRead,
    config: &CollectorConfig,
    artifacts: &dyn ArtifactReader,
) -> Result<Vec<TargetOutput>, BepError> {
    collect_with_workspace(reader, config, artifacts, None)
}

pub fn collect_with_workspace(
    reader: impl BufRead,
    config: &CollectorConfig,
    artifacts: &dyn ArtifactReader,
    workspace: Option<&Path>,
) -> Result<Vec<TargetOutput>, BepError> {
    let mut sets: BTreeMap<String, RawSet> = BTreeMap::new();
    let mut pending: Vec<PendingTarget> = Vec::new();
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
        if let Some(set_id) = named_set_id(id) {
            let named = object.get("namedSetOfFiles").and_then(Value::as_object);
            let mut raw = Vec::new();
            if let Some(files) = named.and_then(|named| named.get("files")) {
                let files = files.as_array().ok_or_else(|| BepError::MalformedEvent {
                    line: line_no,
                    reason: "named set files must be an array".to_owned(),
                })?;
                for file in files {
                    let obj = file.as_object();
                    let uri = obj
                        .and_then(|entry| entry.get("uri"))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let name = obj
                        .and_then(|entry| entry.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let mut path_prefix = Vec::new();
                    if let Some(prefix) = obj
                        .and_then(|entry| entry.get("pathPrefix"))
                        .and_then(Value::as_array)
                    {
                        for part in prefix {
                            if let Some(text) = part.as_str() {
                                path_prefix.push(text.to_owned());
                            }
                        }
                    }
                    raw.push(RawFile {
                        line: line_no,
                        uri,
                        name,
                        path_prefix,
                    });
                }
            }
            let mut children = Vec::new();
            if let Some(sets) = named.and_then(|named| named.get("fileSets")) {
                let sets = sets.as_array().ok_or_else(|| BepError::MalformedEvent {
                    line: line_no,
                    reason: "named set fileSets must be an array".to_owned(),
                })?;
                for child in sets {
                    let child_id = child
                        .as_object()
                        .and_then(|set| set.get("id"))
                        .and_then(Value::as_str)
                        .ok_or_else(|| BepError::MalformedEvent {
                            line: line_no,
                            reason: "named set child without id".to_owned(),
                        })?;
                    children.push(child_id.to_owned());
                }
            }
            sets.entry(set_id.to_owned()).or_insert(RawSet {
                line: line_no,
                files: raw,
                children,
            });
        }
        if let Some(completed) = object.get("completed") {
            if completed.is_null() {
                continue;
            }
            let completed = completed
                .as_object()
                .ok_or_else(|| BepError::MalformedEvent {
                    line: line_no,
                    reason: "completed must be an object".to_owned(),
                })?;
            let label = id
                .get("targetCompleted")
                .and_then(Value::as_object)
                .and_then(|target| target.get("label"))
                .and_then(Value::as_str)
                .ok_or_else(|| BepError::MalformedEvent {
                    line: line_no,
                    reason: "completed event without target label".to_owned(),
                })?;
            let success = completed
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let mut set_ids = Vec::new();
            if success {
                if let Some(groups) = completed.get("outputGroup") {
                    let groups = groups.as_array().ok_or_else(|| BepError::MalformedEvent {
                        line: line_no,
                        reason: "outputGroup must be an array".to_owned(),
                    })?;
                    for group in groups {
                        let group = group.as_object().ok_or_else(|| BepError::MalformedEvent {
                            line: line_no,
                            reason: "output group must be an object".to_owned(),
                        })?;
                        if group.get("name").and_then(Value::as_str) != Some(&config.output_group) {
                            continue;
                        }
                        if let Some(file_sets) = group.get("fileSets") {
                            let file_sets =
                                file_sets
                                    .as_array()
                                    .ok_or_else(|| BepError::MalformedEvent {
                                        line: line_no,
                                        reason: "fileSets must be an array".to_owned(),
                                    })?;
                            for set in file_sets {
                                let set_id = set
                                    .as_object()
                                    .and_then(|s| s.get("id"))
                                    .and_then(Value::as_str)
                                    .ok_or_else(|| BepError::MalformedEvent {
                                        line: line_no,
                                        reason: "file set without id".to_owned(),
                                    })?;
                                set_ids.push(set_id.to_owned());
                            }
                        }
                    }
                }
            }
            if success && set_ids.is_empty() && !mentions_group(completed, config.output_group()) {
                continue;
            }
            pending.push(PendingTarget {
                line: line_no,
                label: label.to_owned(),
                success,
                set_ids,
            });
        }
    }
    let mut outputs = Vec::with_capacity(pending.len());
    for target in &pending {
        let mut seen: BTreeMap<String, (u64, Option<String>, Vec<String>)> = BTreeMap::new();
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<(String, u64)> = target
            .set_ids
            .iter()
            .map(|set_id| (set_id.clone(), target.line))
            .collect();
        while let Some((set_id, line)) = stack.pop() {
            if !visited.insert(set_id.clone()) {
                continue;
            }
            let set = sets.get(&set_id).ok_or_else(|| BepError::MissingNamedSet {
                id: set_id.clone(),
                line,
            })?;
            for file in &set.files {
                let uri = file.uri.clone().ok_or_else(|| BepError::MalformedEvent {
                    line: file.line,
                    reason: "named set file without uri".to_owned(),
                })?;
                seen.entry(uri)
                    .or_insert((file.line, file.name.clone(), file.path_prefix.clone()));
            }
            for child in &set.children {
                stack.push((child.clone(), set.line));
            }
        }
        let mut collected = Vec::with_capacity(seen.len());
        for (uri, (_, name, path_prefix)) in &seen {
            let path = if is_bytestream_uri(uri) {
                let Some(ws) = workspace else {
                    return Err(BepError::UnsupportedUri { uri: uri.clone() });
                };
                let Some(file_name) = name else {
                    return Err(BepError::UnsupportedUri { uri: uri.clone() });
                };
                local_path_for_bep_file(ws, path_prefix, file_name)
            } else {
                file_uri_to_path(uri)?
            };
            let bytes =
                artifacts
                    .read_artifact(&path)
                    .map_err(|e| BepError::UnreadableArtifact {
                        path: path.display().to_string(),
                        message: e.to_string(),
                    })?;
            collected.push(CollectedArtifact {
                exec_path: path,
                bytes,
            });
        }
        collected.sort_by(|a, b| {
            a.exec_path
                .as_os_str()
                .as_encoded_bytes()
                .cmp(b.exec_path.as_os_str().as_encoded_bytes())
        });
        outputs.push(TargetOutput {
            label: target.label.clone(),
            success: target.success,
            artifacts: collected,
        });
    }
    outputs.sort_by(|a, b| a.label.as_bytes().cmp(b.label.as_bytes()));
    Ok(outputs)
}

fn mentions_group(completed: &serde_json::Map<String, Value>, group: &str) -> bool {
    completed
        .get("outputGroup")
        .and_then(Value::as_array)
        .map(|groups| {
            groups.iter().any(|g| {
                g.as_object()
                    .and_then(|g| g.get("name"))
                    .and_then(Value::as_str)
                    == Some(group)
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactReader, CollectorConfig};
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    struct FakeArtifacts {
        files: HashMap<PathBuf, Vec<u8>>,
    }

    impl ArtifactReader for FakeArtifacts {
        fn read_artifact(&self, path: &Path) -> std::io::Result<Vec<u8>> {
            self.files.get(path).cloned().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "missing artifact")
            })
        }
    }

    struct FsArtifacts;

    impl ArtifactReader for FsArtifacts {
        fn read_artifact(&self, path: &Path) -> std::io::Result<Vec<u8>> {
            std::fs::read(path)
        }
    }

    fn config() -> CollectorConfig {
        CollectorConfig::new("dx_results").expect("group")
    }

    fn named_set(id: &str, uris: &[String]) -> String {
        let files: Vec<String> = uris
            .iter()
            .map(|u| format!(r#"{{"name": "result.pb", "uri": "{u}"}}"#))
            .collect();
        format!(
            r#"{{"id": {{"namedSetOfFiles": {{"id": "{id}"}}}}, "namedSetOfFiles": {{"files": [{}]}}}}"#,
            files.join(",")
        )
    }

    fn completed(label: &str, success: bool, groups: &str) -> String {
        format!(
            r#"{{"id": {{"targetCompleted": {{"label": "{label}"}}}}, "completed": {{"success": {success}, "outputGroup": [{groups}]}}}}"#
        )
    }

    fn group_ref(name: &str, sets: &[&str]) -> String {
        let refs: Vec<String> = sets.iter().map(|s| format!(r#"{{"id": "{s}"}}"#)).collect();
        format!(r#"{{"name": "{name}", "fileSets": [{}]}}"#, refs.join(","))
    }

    /// A hand-written file URI naming one path under a root this host can resolve.
    fn out_uri(rel: &str) -> String {
        if cfg!(windows) {
            format!("file:///C:/out/{rel}")
        } else {
            format!("file:///out/{rel}")
        }
    }

    /// The host path one slash-separated relative name under the fixture root names.
    fn out_path(rel: &str) -> PathBuf {
        let root = if cfg!(windows) { r"C:\out" } else { "/out" };
        let mut path = PathBuf::from(root);
        for part in rel.split('/').filter(|part| !part.is_empty()) {
            path.push(part);
        }
        path
    }

    #[test]
    fn empty_group_rejected() {
        assert_eq!(
            CollectorConfig::new("").expect_err("empty"),
            BepError::EmptyOutputGroup
        );
    }

    #[test]
    fn display_is_human_readable() {
        assert_eq!(
            BepError::EmptyOutputGroup.to_string(),
            "empty output group: want a non-empty output group name"
        );
        assert_eq!(
            BepError::MalformedEvent {
                line: 3,
                reason: "invalid JSON: boom".to_owned(),
            }
            .to_string(),
            "malformed event at line 3: invalid JSON: boom"
        );
        assert_eq!(
            BepError::UnsupportedUri {
                uri: "bytestream://x".to_owned(),
            }
            .to_string(),
            "unsupported artifact URI \"bytestream://x\": want a local file:// URI"
        );
        assert_eq!(
            BepError::MissingNamedSet {
                id: "1".to_owned(),
                line: 2,
            }
            .to_string(),
            "missing named set \"1\" referenced at line 2"
        );
        assert_eq!(
            BepError::UnreadableArtifact {
                path: "/tmp/a".to_owned(),
                message: "boom".to_owned(),
            }
            .to_string(),
            "unreadable artifact \"/tmp/a\": boom"
        );
        for err in [
            BepError::EmptyOutputGroup,
            BepError::MalformedEvent {
                line: 1,
                reason: "x".to_owned(),
            },
        ] {
            assert!(!err.to_string().contains("BepError"));
            assert!(!err.to_string().contains("EmptyOutputGroup {"));
            assert!(!err.to_string().contains("MalformedEvent {"));
        }
    }

    #[test]
    fn unreadable_stream_lines_fail() {
        struct FailRead;
        impl std::io::Read for FailRead {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "boom"))
            }
        }
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(std::io::BufReader::new(FailRead), &config(), &artifacts)
            .expect_err("unreadable line");
        assert!(matches!(err, BepError::MalformedEvent { .. }));
    }

    #[test]
    fn named_set_files_must_be_array() {
        let stream =
            r#"{"id": {"namedSetOfFiles": {"id": "1"}}, "namedSetOfFiles": {"files": {}}}"#;
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("object files");
        assert!(matches!(err, BepError::MalformedEvent { .. }));
    }

    #[test]
    fn named_set_without_files_resolves_empty() {
        let stream = [
            r#"{"id": {"namedSetOfFiles": {"id": "1"}}}"#.to_owned(),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert!(got[0].artifacts.is_empty());
    }

    fn modern_set(id: &str, body: &str) -> String {
        format!(r#"{{"id": {{"namedSet": {{"id": "{id}"}}}}, "namedSetOfFiles": {body}}}"#)
    }

    #[test]
    fn modern_named_set_id_collects() {
        let body = format!(
            r#"{{"files": [{{"name": "result.pb", "uri": "{}"}}]}}"#,
            out_uri("a.pb")
        );
        let stream = [
            modern_set("1", &body),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![9])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].artifacts.len(), 1);
        assert_eq!(got[0].artifacts[0].bytes, vec![9]);
    }

    #[test]
    fn nested_child_sets_resolve_transitively() {
        let child = format!(
            r#"{{"files": [{{"name": "b.pb", "uri": "{}"}}]}}"#,
            out_uri("b.pb")
        );
        let parent = format!(
            r#"{{"files": [{{"name": "a.pb", "uri": "{}"}}], "fileSets": [{{"id": "2"}}]}}"#,
            out_uri("a.pb")
        );
        let stream = [
            modern_set("2", &child),
            modern_set("1", &parent),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![1]), (out_path("b.pb"), vec![2])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].artifacts.len(), 2);
        assert_eq!(got[0].artifacts[0].exec_path, out_path("a.pb"));
        assert_eq!(got[0].artifacts[1].exec_path, out_path("b.pb"));
    }

    #[test]
    fn missing_child_set_fails() {
        let parent = r#"{"files": [], "fileSets": [{"id": "9"}]}"#;
        let stream = [
            modern_set("1", &parent),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("missing child");
        assert_eq!(
            err,
            BepError::MissingNamedSet {
                id: "9".to_owned(),
                line: 1,
            }
        );
    }

    #[test]
    fn child_set_without_id_fails() {
        let parent = r#"{"files": [], "fileSets": [{}]}"#;
        let stream = modern_set("1", &parent);
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("child id");
        assert!(matches!(err, BepError::MalformedEvent { .. }));
    }

    #[test]
    fn non_array_file_sets_fails() {
        let parent = r#"{"files": [], "fileSets": {}}"#;
        let stream = modern_set("1", &parent);
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("fileSets");
        assert!(matches!(
            err,
            BepError::MalformedEvent {
                reason,
                ..
            } if reason.contains("fileSets must be an array")
        ));
    }

    #[test]
    fn repeated_set_reference_resolves_once() {
        let body = format!(
            r#"{{"files": [{{"name": "a.pb", "uri": "{}"}}], "fileSets": [{{"id": "1"}}]}}"#,
            out_uri("a.pb")
        );
        let stream = [
            modern_set("1", &body),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![1])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].artifacts.len(), 1);
    }

    #[test]
    fn null_completed_events_are_ignored() {
        let stream = [
            r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": null}"#.to_owned(),
            named_set("1", &[out_uri("a.pb")]),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![9])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn malformed_completed_events_fail() {
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let stream = r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": []}"#;
        assert!(matches!(
            collect(Cursor::new(stream), &config(), &artifacts),
            Err(BepError::MalformedEvent { .. })
        ));
        let stream = r#"{"id": {}, "completed": {"success": true}}"#;
        assert!(matches!(
            collect(Cursor::new(stream), &config(), &artifacts),
            Err(BepError::MalformedEvent { .. })
        ));
    }

    #[test]
    fn aborted_completed_events_collect_as_failed() {
        let stream = [
            r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": {}}"#.to_owned(),
            r#"{"id": {"targetCompleted": {"label": "//q:b"}}, "completed": {"success": false}}"#
                .to_owned(),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 2);
        for output in &got {
            assert!(!output.success);
            assert!(output.artifacts.is_empty());
        }
    }

    #[test]
    fn malformed_output_groups_fail() {
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let stream = r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": {"success": true, "outputGroup": {}}}"#;
        assert!(matches!(
            collect(Cursor::new(stream), &config(), &artifacts),
            Err(BepError::MalformedEvent { .. })
        ));
        let stream = r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": {"success": true, "outputGroup": ["dx_results"]}}"#;
        assert!(matches!(
            collect(Cursor::new(stream), &config(), &artifacts),
            Err(BepError::MalformedEvent { .. })
        ));
        let stream = r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": {"success": true, "outputGroup": [{"name": "dx_results", "fileSets": {}}]}}"#;
        assert!(matches!(
            collect(Cursor::new(stream), &config(), &artifacts),
            Err(BepError::MalformedEvent { .. })
        ));
        let stream = r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": {"success": true, "outputGroup": [{"name": "dx_results", "fileSets": [{}]}]}}"#;
        assert!(matches!(
            collect(Cursor::new(stream), &config(), &artifacts),
            Err(BepError::MalformedEvent { .. })
        ));
    }

    #[test]
    fn successful_target_without_group_is_skipped() {
        let stream = [
            r#"{"id": {"targetCompleted": {"label": "//q:a"}}, "completed": {"success": true}}"#
                .to_owned(),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert!(got.is_empty());
    }

    #[test]
    fn failed_targets_report_without_artifacts() {
        let stream = [completed("//q:a", false, &group_ref("dx_results", &["1"]))].join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert!(!got[0].success);
        assert!(got[0].artifacts.is_empty());
    }

    #[test]
    fn group_without_filesets_reports_no_artifacts() {
        let stream = [completed("//q:a", true, r#"{"name": "dx_results"}"#)].join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert!(got[0].success);
        assert!(got[0].artifacts.is_empty());
    }

    #[test]
    fn named_set_file_without_uri_fails() {
        let stream = [
            r#"{"id": {"namedSetOfFiles": {"id": "1"}}, "namedSetOfFiles": {"files": [{}]}}"#
                .to_owned(),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("uri missing");
        assert!(matches!(err, BepError::MalformedEvent { .. }));
    }

    #[test]
    fn artifacts_sort_by_path_bytes() {
        let stream = [
            named_set("1", &[out_uri("b.pb"), out_uri("a.pb")]),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![1]), (out_path("b.pb"), vec![2])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got[0].artifacts.len(), 2);
        assert_eq!(got[0].artifacts[0].exec_path, out_path("a.pb"));
        assert_eq!(got[0].artifacts[1].exec_path, out_path("b.pb"));
    }

    #[test]
    fn collects_reported_artifacts_only() {
        let stream = [
            named_set("1", &[out_uri("a.pb")]),
            named_set("2", &[out_uri("unrelated.pb")]),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
            completed("//q:b", true, &group_ref("other_group", &["2"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![1, 2, 3])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].label, "//q:a");
        assert!(got[0].success);
        assert_eq!(got[0].artifacts.len(), 1);
        assert_eq!(got[0].artifacts[0].exec_path, out_path("a.pb"));
        assert_eq!(got[0].artifacts[0].bytes, vec![1, 2, 3]);
    }

    #[test]
    fn records_sort_by_label_bytes() {
        let stream = [
            named_set("1", &[out_uri("b.pb")]),
            named_set("2", &[out_uri("a.pb")]),
            completed("//q:b", true, &group_ref("dx_results", &["1"])),
            completed("//q:a", true, &group_ref("dx_results", &["2"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![1]), (out_path("b.pb"), vec![2])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].label, "//q:a");
        assert_eq!(got[1].label, "//q:b");
    }

    #[test]
    fn failed_targets_keep_partial_records_without_reads() {
        let stream = completed("//q:a", false, "");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert!(!got[0].success);
        assert!(got[0].artifacts.is_empty());
    }

    #[test]
    fn successful_targets_without_group_are_skipped() {
        let stream = completed("//q:a", true, &group_ref("other_group", &["9"]));
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert!(got.is_empty());
    }

    #[test]
    fn malformed_lines_fail_collection() {
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new("not json\n"), &config(), &artifacts).expect_err("malformed");
        assert!(matches!(err, BepError::MalformedEvent { line: 1, .. }));
        let err = collect(Cursor::new("[1,2]\n"), &config(), &artifacts).expect_err("array");
        assert!(matches!(err, BepError::MalformedEvent { .. }));
        let err = collect(
            Cursor::new(r#"{"completed": {"success": true}}"#),
            &config(),
            &artifacts,
        )
        .expect_err("no id");
        assert!(matches!(err, BepError::MalformedEvent { .. }));
    }

    #[test]
    fn missing_named_set_fails() {
        let stream = completed("//q:a", true, &group_ref("dx_results", &["ghost"]));
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        assert_eq!(
            collect(Cursor::new(stream), &config(), &artifacts).expect_err("missing set"),
            BepError::MissingNamedSet {
                id: "ghost".to_owned(),
                line: 1,
            }
        );
    }

    #[test]
    fn remote_uris_fail_without_network_fetch() {
        let stream = [
            named_set("1", &["bytestream://remote/cache/a.pb".to_owned()]),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        struct NoReadReader {
            reads: std::cell::Cell<usize>,
        }
        impl ArtifactReader for NoReadReader {
            fn read_artifact(&self, _path: &Path) -> std::io::Result<Vec<u8>> {
                self.reads.set(self.reads.get() + 1);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "reader must not be reached",
                ))
            }
        }
        let reader = NoReadReader {
            reads: std::cell::Cell::new(0),
        };
        let err = collect(Cursor::new(stream), &config(), &reader).expect_err("remote");
        assert_eq!(
            err,
            BepError::UnsupportedUri {
                uri: "bytestream://remote/cache/a.pb".to_owned()
            }
        );
        assert_eq!(reader.reads.get(), 0);
        let probe = NoReadReader {
            reads: std::cell::Cell::new(0),
        };
        assert!(probe.read_artifact(Path::new("/out/a.pb")).is_err());
        assert_eq!(probe.reads.get(), 1);
    }

    #[test]
    fn reports_read_the_filenames_their_uris_name() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let root = dx_path::manifest(dir.path());
        let names: &[(&str, &str)] = if cfg!(windows) {
            &[("a b.pb", "a%20b.pb"), ("a%20b.pb", "a%2520b.pb")]
        } else {
            &[
                ("a b.pb", "a%20b.pb"),
                ("a%20b.pb", "a%2520b.pb"),
                ("a#b.pb", "a%23b.pb"),
                ("a?b.pb", "a%3Fb.pb"),
            ]
        };
        let mut uris = Vec::new();
        for (name, escaped) in names {
            let path = dir.path().join(name);
            std::fs::write(&path, name.as_bytes()).expect("write");
            uris.push(format!(
                "file:///{}/{escaped}",
                root.trim_start_matches('/')
            ));
        }
        let stream = [
            named_set("1", &uris),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let got = collect(Cursor::new(stream), &config(), &FsArtifacts).expect("collect");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].artifacts.len(), names.len());
        let mut read = got[0]
            .artifacts
            .iter()
            .map(|artifact| (artifact.exec_path.clone(), artifact.bytes.clone()))
            .collect::<Vec<_>>();
        read.sort();
        let mut want = names
            .iter()
            .map(|(name, _)| {
                let path = dir.path().join(name);
                let bytes = std::fs::read(&path).expect("read");
                (path, bytes)
            })
            .collect::<Vec<_>>();
        want.sort();
        assert_eq!(read, want);
    }

    #[test]
    fn bytestream_falls_back_to_workspace_local_copy() {
        let dir = tempfile::TempDir::new().expect("scratch");
        let ws = dir.path();
        let mut local = ws.to_path_buf();
        local.push("bazel-out");
        local.push("k8-fastbuild");
        local.push("bin");
        local.push("q/a.pb");
        std::fs::create_dir_all(local.parent().expect("parent")).expect("dirs");
        std::fs::write(&local, b"cached-bytes").expect("write");
        let file = r#"{"name": "q/a.pb", "uri": "bytestream://remote.buildbuddy.io/blobs/abc/12", "pathPrefix": ["bazel-out", "k8-fastbuild", "bin"]}"#;
        let stream = [
            format!(
                r#"{{"id": {{"namedSetOfFiles": {{"id": "1"}}}}, "namedSetOfFiles": {{"files": [{file}]}}}}"#
            ),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(local.clone(), b"cached-bytes".to_vec())]),
        };
        let got = collect_with_workspace(Cursor::new(stream), &config(), &artifacts, Some(ws))
            .expect("fallback");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].artifacts.len(), 1);
        assert_eq!(got[0].artifacts[0].bytes, b"cached-bytes".to_vec());
        assert_eq!(got[0].artifacts[0].exec_path, local);
    }

    #[test]
    fn bytestream_without_name_or_workspace_fails_closed() {
        let file = r#"{"uri": "bytestream://remote.buildbuddy.io/blobs/abc/12", "pathPrefix": ["bazel-out"]}"#;
        let stream = [
            format!(
                r#"{{"id": {{"namedSetOfFiles": {{"id": "1"}}}}, "namedSetOfFiles": {{"files": [{file}]}}}}"#
            ),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let dir = tempfile::TempDir::new().expect("scratch");
        let err = collect_with_workspace(
            Cursor::new(stream.clone()),
            &config(),
            &artifacts,
            Some(dir.path()),
        )
        .expect_err("nameless bytestream must fail");
        assert!(matches!(err, BepError::UnsupportedUri { .. }));
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("no workspace");
        assert!(matches!(err, BepError::UnsupportedUri { .. }));
    }

    #[test]
    fn unreadable_reported_files_fail() {
        let stream = [
            named_set("1", &[out_uri("missing.pb")]),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::new(),
        };
        let err = collect(Cursor::new(stream), &config(), &artifacts).expect_err("missing");
        assert!(matches!(err, BepError::UnreadableArtifact { .. }));
    }

    #[test]
    fn duplicate_artifacts_deduplicated() {
        let stream = [
            named_set("1", &[out_uri("a.pb")]),
            named_set("2", &[out_uri("a.pb")]),
            completed("//q:a", true, &group_ref("dx_results", &["1", "2"])),
        ]
        .join("\n");
        struct CountingReader {
            reads: std::cell::Cell<usize>,
        }
        impl ArtifactReader for CountingReader {
            fn read_artifact(&self, _path: &Path) -> std::io::Result<Vec<u8>> {
                self.reads.set(self.reads.get() + 1);
                Ok(vec![7])
            }
        }
        let reader = CountingReader {
            reads: std::cell::Cell::new(0),
        };
        let got = collect(Cursor::new(stream), &config(), &reader).expect("collect");
        assert_eq!(got[0].artifacts.len(), 1);
        assert_eq!(reader.reads.get(), 1);
    }

    #[test]
    fn unknown_events_are_ignored() {
        let stream = [
            r#"{"id": {"progress": {}}, "progress": {"stdout": "noise"}}"#.to_owned(),
            r#"{"id": {"buildFinished": {}}, "finished": {"success": true}}"#.to_owned(),
            named_set("1", &[out_uri("a.pb")]),
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![9])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got.len(), 1);
    }

    #[test]
    fn set_definition_order_does_not_matter() {
        let stream = [
            completed("//q:a", true, &group_ref("dx_results", &["1"])),
            named_set("1", &[out_uri("a.pb")]),
        ]
        .join("\n");
        let artifacts = FakeArtifacts {
            files: HashMap::from([(out_path("a.pb"), vec![5])]),
        };
        let got = collect(Cursor::new(stream), &config(), &artifacts).expect("collect");
        assert_eq!(got[0].artifacts[0].bytes, vec![5]);
    }
}
