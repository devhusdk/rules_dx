//! Offline checks dx runs on CloudFormation templates before cfn-lint sees them.
//!
//! A `cloudformation_template` target owns plain YAML and JSON files, so dx decides whether
//! each file is a template and refuses a cfn-lint config that loads code or reads other files.

use saphyr_parser::{Event, Parser};

/// A file that does not parse as YAML or JSON, or is too large or not UTF-8.
pub const RULE_SYNTAX: &str = "dx/cloudformation-syntax";
/// A file that is not a CloudFormation template.
pub const RULE_ROOT: &str = "dx/cloudformation-root";

/// The largest template dx hands to cfn-lint.
pub const MAX_TEMPLATE_BYTES: usize = 2 * 1024 * 1024;

/// The only cfn-lint config keys dx accepts; everything else could load code, read or write unstaged paths, or hide failures.
const ALLOWED_CONFIG_KEYS: &[&str] = &[
    "regions",
    "ignore_checks",
    "include_checks",
    "mandatory_checks",
    "configure_rules",
    "include_experimental",
];

/// One problem the preflight found, placed on the file it was checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub rule: &'static str,
    pub line: u64,
    pub column: u64,
    pub message: String,
}

#[derive(Debug, Default)]
struct Scanned {
    root_is_map: bool,
    root_keys: Vec<String>,
    aws_resource: bool,
}

enum Frame {
    Map {
        expecting_key: bool,
        key: Option<String>,
    },
    Seq,
}

fn syntax(line: usize, col: usize, detail: &str) -> Problem {
    Problem {
        rule: RULE_SYNTAX,
        line: line.max(1) as u64,
        column: col as u64 + 1,
        message: format!("not valid YAML or JSON: {detail}"),
    }
}

fn root_problem(message: &str) -> Problem {
    Problem {
        rule: RULE_ROOT,
        line: 1,
        column: 1,
        message: message.to_owned(),
    }
}

/// Marks the value of the enclosing mapping as consumed.
fn value_done(stack: &mut [Frame]) {
    if let Some(Frame::Map { expecting_key, .. }) = stack.last_mut() {
        *expecting_key = true;
    }
}

/// The root keys of one YAML document and whether `Resources` holds an `AWS::` typed entry.
fn scan_yaml(text: &str) -> Result<Scanned, Problem> {
    let mut out = Scanned::default();
    let mut stack: Vec<Frame> = Vec::new();
    let mut documents = 0usize;
    for item in Parser::new_from_str(text) {
        let (event, span) =
            item.map_err(|err| syntax(err.marker().line(), err.marker().col(), err.info()))?;
        match event {
            Event::DocumentStart(_) => {
                documents += 1;
                if documents > 1 {
                    return Err(syntax(
                        span.start.line(),
                        span.start.col(),
                        "more than one document in the stream",
                    ));
                }
            }
            Event::Scalar(value, ..) => {
                let expecting = matches!(
                    stack.last(),
                    Some(Frame::Map {
                        expecting_key: true,
                        ..
                    })
                );
                if expecting {
                    if stack.len() == 1 {
                        out.root_keys.push(value.to_string());
                    }
                    if let Some(Frame::Map { expecting_key, key }) = stack.last_mut() {
                        *expecting_key = false;
                        *key = Some(value.to_string());
                    }
                } else {
                    if stack.len() == 3 && value.starts_with("AWS::") && is_resource_type(&stack) {
                        out.aws_resource = true;
                    }
                    value_done(&mut stack);
                }
            }
            Event::Alias(_) => value_done(&mut stack),
            Event::MappingStart(..) | Event::SequenceStart(..) => {
                if matches!(
                    stack.last(),
                    Some(Frame::Map {
                        expecting_key: true,
                        ..
                    })
                ) {
                    return Err(syntax(
                        span.start.line(),
                        span.start.col(),
                        "a mapping or sequence used as a key",
                    ));
                }
                let is_map = matches!(event, Event::MappingStart(..));
                if stack.is_empty() {
                    out.root_is_map = is_map;
                }
                stack.push(if is_map {
                    Frame::Map {
                        expecting_key: true,
                        key: None,
                    }
                } else {
                    Frame::Seq
                });
            }
            Event::MappingEnd | Event::SequenceEnd => {
                stack.pop();
                value_done(&mut stack);
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Whether the scalar being read is the `Type` of an entry directly under root `Resources`.
fn is_resource_type(stack: &[Frame]) -> bool {
    let key_of = |frame: &Frame| match frame {
        Frame::Map { key, .. } => key.clone(),
        Frame::Seq => None,
    };
    stack.len() == 3
        && matches!(stack[1], Frame::Map { .. })
        && matches!(stack[2], Frame::Map { .. })
        && key_of(&stack[0]).as_deref() == Some("Resources")
        && key_of(&stack[2]).as_deref() == Some("Type")
}

fn scan_json(text: &str) -> Result<Scanned, Problem> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|err| syntax(err.line(), err.column().saturating_sub(1), &err.to_string()))?;
    let mut out = Scanned::default();
    if let serde_json::Value::Object(root) = &value {
        out.root_is_map = true;
        out.root_keys = root.keys().cloned().collect();
        if let Some(serde_json::Value::Object(resources)) = root.get("Resources") {
            out.aws_resource = resources.values().any(|resource| {
                resource
                    .get("Type")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|kind| kind.starts_with("AWS::"))
            });
        }
    }
    Ok(out)
}

/// Whether the file is read as JSON: a `.json` file, or a `.template` that opens with `{`.
fn is_json(path: &str, text: &str) -> bool {
    path.ends_with(".json") || (path.ends_with(".template") && text.trim_start().starts_with('{'))
}

fn decode(bytes: &[u8]) -> Result<&str, Problem> {
    if bytes.len() > MAX_TEMPLATE_BYTES {
        return Err(syntax(
            1,
            0,
            &format!("the file is larger than {MAX_TEMPLATE_BYTES} bytes"),
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| syntax(1, 0, "the file is not UTF-8"))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(text))
}

/// The problem that keeps one owned file from going to cfn-lint, if any.
///
/// A file passes when it has a root `AWSTemplateFormatVersion`, or a root `Resources` mapping
/// with an entry whose `Type` starts with `AWS::`.
pub fn check_template(path: &str, bytes: &[u8]) -> Option<Problem> {
    let text = match decode(bytes) {
        Ok(text) => text,
        Err(problem) => return Some(problem),
    };
    let scanned = if is_json(path, text) {
        scan_json(text)
    } else {
        scan_yaml(text)
    };
    let scanned = match scanned {
        Ok(scanned) => scanned,
        Err(problem) => return Some(problem),
    };
    if !scanned.root_is_map {
        return Some(root_problem(
            "a CloudFormation template needs a mapping at the root",
        ));
    }
    if scanned
        .root_keys
        .iter()
        .any(|key| key == "AWSTemplateFormatVersion")
        || scanned.aws_resource
    {
        return None;
    }
    Some(root_problem(
        "not a CloudFormation template: it needs a root AWSTemplateFormatVersion or a Resources entry with an AWS:: Type",
    ))
}

/// Refuses a cfn-lint config with any key outside the allowlist.
pub fn check_config(text: &str) -> Result<(), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let scanned = scan_yaml(text)
        .map_err(|problem| format!("cfn_lint config line {}: {}", problem.line, problem.message))?;
    if !text.trim().is_empty() && !scanned.root_is_map {
        return Err("cfn_lint config must be a YAML mapping".to_owned());
    }
    for key in &scanned.root_keys {
        if !ALLOWED_CONFIG_KEYS.contains(&key.as_str()) {
            return Err(format!(
                "cfn_lint config key `{key}` is not allowed: dx accepts only {}",
                ALLOWED_CONFIG_KEYS.join(", ")
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(path: &str, text: &str) -> Option<&'static str> {
        check_template(path, text.as_bytes()).map(|p| p.rule)
    }

    #[test]
    fn yaml_with_short_tags_passes() {
        let text = "AWSTemplateFormatVersion: '2010-09-09'\nResources:\n  B:\n    Type: AWS::S3::Bucket\n    Properties:\n      BucketName: !Sub \"${AWS::StackName}-b\"\n      Tags: [!Ref X]\n";
        assert_eq!(rule("t.yaml", text), None);
    }

    #[test]
    fn resources_only_json_passes() {
        let text = r#"{"Resources":{"B":{"Type":"AWS::S3::Bucket"}}}"#;
        assert_eq!(rule("t.json", text), None);
    }

    #[test]
    fn json_with_bom_passes() {
        let text = "\u{feff}{\"AWSTemplateFormatVersion\":\"2010-09-09\"}";
        assert_eq!(rule("t.json", text), None);
    }

    #[test]
    fn sam_template_passes() {
        let text = "Transform: AWS::Serverless-2016-10-31\nResources:\n  F:\n    Type: AWS::Serverless::Function\n";
        assert_eq!(rule("t.yml", text), None);
    }

    #[test]
    fn template_extension_reads_json_or_yaml() {
        let json = r#"{"Resources":{"B":{"Type":"AWS::S3::Bucket"}}}"#;
        assert_eq!(rule("t.template", json), None);
        assert_eq!(
            rule(
                "t.template",
                "Resources:\n  B:\n    Type: AWS::S3::Bucket\n"
            ),
            None
        );
    }

    #[test]
    fn version_with_custom_types_passes() {
        let text =
            "AWSTemplateFormatVersion: '2010-09-09'\nResources:\n  C:\n    Type: Custom::Thing\n";
        assert_eq!(rule("t.yaml", text), None);
    }

    #[test]
    fn kubernetes_manifest_is_not_a_template() {
        let text = "apiVersion: v1\nkind: Pod\nmetadata:\n  name: x\n";
        assert_eq!(rule("k.yaml", text), Some(RULE_ROOT));
    }

    #[test]
    fn plain_json_is_not_a_template() {
        assert_eq!(rule("p.json", r#"{"name":"x"}"#), Some(RULE_ROOT));
    }

    #[test]
    fn custom_only_resources_without_version_fail() {
        let text = "Resources:\n  C:\n    Type: Custom::Thing\n";
        assert_eq!(rule("t.yaml", text), Some(RULE_ROOT));
    }

    #[test]
    fn deeper_aws_type_does_not_count() {
        let text = "Resources:\n  C:\n    Properties:\n      Type: AWS::S3::Bucket\n";
        assert_eq!(rule("t.yaml", text), Some(RULE_ROOT));
        let nested = "Other:\n  Resources:\n    B:\n      Type: AWS::S3::Bucket\n";
        assert_eq!(rule("t.yaml", nested), Some(RULE_ROOT));
    }

    #[test]
    fn non_mapping_root_fails() {
        assert_eq!(rule("t.yaml", "- a\n- b\n"), Some(RULE_ROOT));
        assert_eq!(rule("t.yaml", ""), Some(RULE_ROOT));
        assert_eq!(rule("t.json", "[1]"), Some(RULE_ROOT));
    }

    #[test]
    fn syntax_errors_are_located() {
        let problem = check_template("t.yaml", b"Resources: [\n").expect("problem");
        assert_eq!(problem.rule, RULE_SYNTAX);
        let problem = check_template("t.json", b"{\n  \"a\": }").expect("problem");
        assert_eq!(problem.rule, RULE_SYNTAX);
        assert_eq!(problem.line, 2);
    }

    #[test]
    fn multiple_documents_fail() {
        assert_eq!(rule("t.yaml", "A: 1\n---\nB: 2\n"), Some(RULE_SYNTAX));
    }

    #[test]
    fn invalid_utf8_and_oversize_fail() {
        let problem = check_template("t.yaml", &[0xff, 0xfe, 0x00]).expect("problem");
        assert_eq!(problem.rule, RULE_SYNTAX);
        let big = vec![b'a'; MAX_TEMPLATE_BYTES + 1];
        let problem = check_template("t.yaml", &big).expect("problem");
        assert_eq!(problem.rule, RULE_SYNTAX);
    }

    #[test]
    fn config_accepts_every_allowed_key() {
        for key in ALLOWED_CONFIG_KEYS {
            assert!(check_config(&format!("{key}: [x]\n")).is_ok(), "{key}");
        }
    }

    #[test]
    fn config_accepts_regions_and_ignore_checks() {
        assert!(check_config("regions: [us-east-1]\nignore_checks: [W2001]\n").is_ok());
        assert!(check_config("").is_ok());
    }

    #[test]
    fn config_rejects_each_unlisted_key() {
        for key in [
            "templates",
            "ignore_templates",
            "append_rules",
            "custom_rules",
            "override_spec",
            "update_specs",
            "output_file",
            "non_zero_exit_code",
            "ignore_bad_template",
            "format",
            "merge_configs",
            "deployment_files",
            "parameter_files",
            "registry_schemas",
            "debug",
        ] {
            let err = check_config(&format!("{key}: [x]\n")).expect_err(key);
            assert!(err.contains(key), "{err}");
        }
    }

    #[test]
    fn config_ignores_nested_forbidden_names() {
        assert!(check_config("configure_rules:\n  templates: x\n").is_ok());
    }

    #[test]
    fn config_rejects_non_mapping_and_bad_yaml() {
        assert!(check_config("- a\n").is_err());
        assert!(check_config("a: [\n").is_err());
    }
}
