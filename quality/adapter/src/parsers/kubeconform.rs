use super::{as_text, code_name, FileFinding, ParseError};
use crate::kubernetes_preflight::{escape, Document};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "kubeconform";
const FILE_URL: &str = "file://";
const ADDITIONAL: &str = "additional properties '";

/// One path kubeconform was handed, the owned file it stands for and that file's documents.
#[derive(Debug, Clone, Copy)]
pub struct KubeconformTarget<'a> {
    pub reported: &'a str,
    pub file: &'a str,
    pub documents: &'a [Document],
}

fn text<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a str, ParseError> {
    match value.get(key) {
        None | Some(serde_json::Value::Null) => Ok(""),
        Some(raw) => raw
            .as_str()
            .ok_or_else(|| ParseError::shape(TOOL, format!("{key} is not a string"))),
    }
}

/// The message with scratch `file://` URLs cut to their file name and staged paths renamed.
fn clean(message: &str, targets: &[KubeconformTarget<'_>]) -> String {
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(at) = rest.find(FILE_URL) {
        out.push_str(&rest[..at]);
        let tail = &rest[at + FILE_URL.len()..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | '#' | ','))
            .unwrap_or(tail.len());
        let url = &tail[..end];
        out.push_str(url.rsplit(['/', '\\']).next().unwrap_or(url));
        rest = &tail[end..];
    }
    out.push_str(rest);
    for target in targets {
        if !target.reported.is_empty() && target.reported != target.file {
            out = out.replace(target.reported, target.file);
        }
    }
    out.trim().to_owned()
}

/// The document a result names: the only one, or the one matching kind, name and version.
fn document<'a>(
    target: &KubeconformTarget<'a>,
    resource: &serde_json::Value,
) -> Result<Option<&'a Document>, ParseError> {
    if let [only] = target.documents {
        return Ok(Some(only));
    }
    let kind = text(resource, "kind")?;
    let name = text(resource, "name")?;
    let version = text(resource, "version")?;
    let same = |field: &Option<String>, wanted: &str| field.as_deref().unwrap_or("") == wanted;
    Ok(target
        .documents
        .iter()
        .find(|doc| {
            same(&doc.kind, kind) && same(&doc.name, name) && same(&doc.api_version, version)
        })
        .or_else(|| target.documents.iter().find(|doc| same(&doc.kind, kind))))
}

/// The position of one validation error, on the named key of an additional property.
fn position(document: Option<&Document>, path: &str, message: &str) -> TextPosition {
    let Some(document) = document else {
        return TextPosition { line: 1, column: 1 };
    };
    let named = message
        .strip_prefix(ADDITIONAL)
        .and_then(|rest| rest.split_once('\''))
        .map(|(key, _)| format!("{path}/{}", escape(key)));
    let (line, column) = match named.and_then(|pointer| document.pointers.get(&pointer).copied()) {
        Some(found) => found,
        None => document.locate(path),
    };
    TextPosition { line, column }
}

fn finding(file: &str, rule: &str, message: String, start: TextPosition) -> FileFinding {
    FileFinding {
        file: file.to_owned(),
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message,
            severity: ToolSeverity::Error,
            start,
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

fn resource_findings(
    resource: &serde_json::Value,
    targets: &[KubeconformTarget<'_>],
    findings: &mut Vec<FileFinding>,
) -> Result<bool, ParseError> {
    if !resource.is_object() {
        return Err(ParseError::shape(TOOL, "a resource is not an object"));
    }
    let reported = text(resource, "filename")?;
    let wanted = dx_path::posix(std::path::Path::new(reported));
    let target = targets
        .iter()
        .find(|target| dx_path::posix(std::path::Path::new(target.reported)) == wanted)
        .ok_or_else(|| ParseError::UnknownFile {
            tool: TOOL,
            path: reported.to_owned(),
        })?;
    let document = document(target, resource)?;
    let start = document.map_or(TextPosition { line: 1, column: 1 }, |doc| TextPosition {
        line: doc.line,
        column: doc.column,
    });
    let message = clean(text(resource, "msg")?, targets);
    match text(resource, "status")? {
        "statusValid" | "statusSkipped" | "statusEmpty" => Ok(false),
        "statusError" => {
            let message = if message.is_empty() {
                "kubeconform could not validate the resource".to_owned()
            } else {
                message
            };
            findings.push(finding(target.file, "error", message, start));
            Ok(true)
        }
        "statusInvalid" => {
            let errors = match resource.get("validationErrors") {
                None | Some(serde_json::Value::Null) => Vec::new(),
                Some(serde_json::Value::Array(errors)) => errors.clone(),
                Some(_) => return Err(ParseError::shape(TOOL, "validationErrors is not an array")),
            };
            if errors.is_empty() {
                let message = if message.is_empty() {
                    "the resource does not match its schema".to_owned()
                } else {
                    message
                };
                findings.push(finding(target.file, "invalid", message, start));
            }
            for error in &errors {
                let path = text(error, "path")?;
                let detail = clean(text(error, "msg")?, targets);
                if detail.is_empty() {
                    return Err(ParseError::shape(TOOL, "validation error without msg"));
                }
                let at = position(document, path, &detail);
                let message = if path.is_empty() {
                    detail
                } else {
                    format!("{path}: {detail}")
                };
                findings.push(finding(target.file, "invalid", message, at));
            }
            Ok(true)
        }
        other => Err(ParseError::shape(TOOL, format!("unknown status {other:?}"))),
    }
}

/// Parses `kubeconform -output json -summary=false`: a `resources` list, or one object each.
///
/// Exit 1 needs a failing result and exit 0 must not have one; anything else fails closed.
pub fn parse_kubeconform(
    stdout: &[u8],
    stderr: &[u8],
    code: Option<i32>,
    targets: &[KubeconformTarget<'_>],
) -> Result<Vec<FileFinding>, ParseError> {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(
            TOOL,
            format!("exit {}: {stderr}", code_name(code)),
        ));
    }
    let report = as_text(TOOL, stdout)?.trim();
    if !report.is_empty() && !report.starts_with('{') {
        return Err(ParseError::Json {
            tool: TOOL,
            detail: format!(
                "stdout is not JSON: {:?}",
                report.chars().take(80).collect::<String>()
            ),
        });
    }
    let mut findings = Vec::new();
    let mut failing = false;
    for value in serde_json::Deserializer::from_str(report).into_iter::<serde_json::Value>() {
        let value = value.map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
        match value.get("resources") {
            Some(serde_json::Value::Array(resources)) => {
                for resource in resources {
                    failing |= resource_findings(resource, targets, &mut findings)?;
                }
            }
            Some(serde_json::Value::Null) => {}
            Some(_) => return Err(ParseError::shape(TOOL, "resources is not an array")),
            None => failing |= resource_findings(&value, targets, &mut findings)?,
        }
    }
    match (code, failing) {
        (Some(1), false) => Err(ParseError::shape(
            TOOL,
            format!("exit 1 without a failing resource: {stderr}"),
        )),
        (Some(0), true) => Err(ParseError::shape(TOOL, "exit 0 with failing resources")),
        _ => Ok(findings),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kubernetes_preflight::check_file;

    const MANIFEST: &str = "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: web\nspec:\n  replicas: \"2\"\n  template:\n    spec:\n      extra: 1\n";

    fn docs() -> Vec<Document> {
        check_file("m.yaml", MANIFEST.as_bytes()).documents
    }

    fn target<'a>(documents: &'a [Document]) -> Vec<KubeconformTarget<'a>> {
        vec![KubeconformTarget {
            reported: "/s/dx-kubeconform/docs/0.yaml",
            file: "/s/k8s/m.yaml",
            documents,
        }]
    }

    const INVALID: &str = r#"{
  "resources": [
    {
      "filename": "/s/dx-kubeconform/docs/0.yaml",
      "kind": "Deployment",
      "name": "web",
      "version": "apps/v1",
      "status": "statusInvalid",
      "msg": "problem validating schema. Check JSON formatting: jsonschema: '/spec/replicas' does not validate with file:///s/k.kubeconform/core/deployment-apps-v1.json#/properties/spec/properties/replicas/type: expected integer, but got string",
      "validationErrors": [
        {"path": "/spec/replicas", "msg": "expected integer, but got string"},
        {"path": "/spec/template/spec", "msg": "additional properties 'extra' not allowed"},
        {"path": "/spec/nothing/here", "msg": "missing properties 'x'"},
        {"path": "", "msg": "missing properties 'status'"}
      ]
    }
  ]
}"#;

    #[test]
    fn kubeconform_clean_run_is_clean() {
        let documents = docs();
        for stdout in ["", "{\n  \"resources\": []\n}", "{\"resources\":null}"] {
            assert!(
                parse_kubeconform(stdout.as_bytes(), b"", Some(0), &target(&documents))
                    .expect("parsed")
                    .is_empty()
            );
        }
        let valid = r#"{"filename":"/s/dx-kubeconform/docs/0.yaml","kind":"Deployment","name":"web","version":"apps/v1","status":"statusValid","msg":""}"#;
        assert!(
            parse_kubeconform(valid.as_bytes(), b"", Some(0), &target(&documents))
                .expect("parsed")
                .is_empty()
        );
    }

    #[test]
    fn kubeconform_invalid_resources_land_on_their_lines() {
        let documents = docs();
        let findings = parse_kubeconform(INVALID.as_bytes(), b"", Some(1), &target(&documents))
            .expect("parsed");
        assert_eq!(findings.len(), 4);
        assert!(findings.iter().all(|f| f.file == "/s/k8s/m.yaml"));
        assert!(findings.iter().all(|f| f.finding.rule_id == "invalid"));
        let lines: Vec<u64> = findings.iter().map(|f| f.finding.start.line).collect();
        assert_eq!(lines, vec![6, 9, 5, 1]);
        assert_eq!(
            findings[0].finding.message,
            "/spec/replicas: expected integer, but got string"
        );
        assert_eq!(findings[1].finding.start.column, 7);
    }

    #[test]
    fn kubeconform_errors_sit_at_the_document_start_without_scratch_paths() {
        let documents = docs();
        let error = r#"{"filename":"/s/dx-kubeconform/docs/0.yaml","kind":"Deployment","name":"web","version":"apps/v1","status":"statusError","msg":"could not find schema for Deployment at file:///s/k.kubeconform/core/deployment-apps-v1.json in /s/dx-kubeconform/docs/0.yaml"}"#;
        let findings =
            parse_kubeconform(error.as_bytes(), b"", Some(1), &target(&documents)).expect("parsed");
        assert_eq!(findings.len(), 1);
        let finding = &findings[0].finding;
        assert_eq!(finding.rule_id, "error");
        assert_eq!(finding.start, TextPosition { line: 1, column: 1 });
        assert_eq!(
            finding.message,
            "could not find schema for Deployment at deployment-apps-v1.json in /s/k8s/m.yaml"
        );
        assert!(!finding.message.contains("file://"));
    }

    #[test]
    fn kubeconform_picks_the_document_by_identity() {
        let text = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: a\n---\napiVersion: v1\nkind: Service\nmetadata:\n  name: b\nspec:\n  ports: x\n";
        let documents = check_file("m.yaml", text.as_bytes()).documents;
        let targets = [KubeconformTarget {
            reported: "k8s/m.yaml",
            file: "k8s/m.yaml",
            documents: &documents,
        }];
        let out = r#"{"filename":"k8s/m.yaml","kind":"Service","name":"b","version":"v1","status":"statusInvalid","msg":"x","validationErrors":[{"path":"/spec/ports","msg":"expected array, but got string"}]}"#;
        let findings = parse_kubeconform(out.as_bytes(), b"", Some(1), &targets).expect("parsed");
        assert_eq!(findings[0].finding.start.line, 11);
    }

    #[test]
    fn kubeconform_execution_errors_are_never_clean() {
        let documents = docs();
        let targets = target(&documents);
        assert!(parse_kubeconform(b"", b"failed to read schema", Some(1), &targets).is_err());
        assert!(parse_kubeconform(b"{\"resources\":[", b"", Some(1), &targets).is_err());
        assert!(parse_kubeconform(b"oops", b"", Some(0), &targets).is_err());
        assert!(parse_kubeconform(INVALID.as_bytes(), b"", Some(0), &targets).is_err());
        assert!(parse_kubeconform(INVALID.as_bytes(), b"", Some(2), &targets).is_err());
        assert!(parse_kubeconform(INVALID.as_bytes(), b"", None, &targets).is_err());
        let unknown = INVALID.replace("docs/0.yaml", "docs/9.yaml");
        let err =
            parse_kubeconform(unknown.as_bytes(), b"", Some(1), &targets).expect_err("unknown");
        assert!(matches!(err, ParseError::UnknownFile { .. }), "{err}");
        let status = INVALID.replace("statusInvalid", "statusWeird");
        assert!(parse_kubeconform(status.as_bytes(), b"", Some(1), &targets).is_err());
    }
}
