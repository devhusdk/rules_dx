use super::{as_text, code_name, FileFinding, ParseError};
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

const TOOL: &str = "helm";
const SOURCE: &str = "# Source: ";

/// One rendered template: the chart-relative path `helm template` named and its YAML text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedFile {
    pub path: String,
    pub text: String,
}

fn join(chart_dir: &str, rel: &str) -> String {
    if chart_dir.is_empty() {
        rel.to_owned()
    } else {
        format!("{chart_dir}/{rel}")
    }
}

/// The `(path, line, column)` of the first `<chart>/<path>:<line>[:<column>]` token in a message.
///
/// The leading segment is the chart name from Chart.yaml, so it is dropped, not matched.
fn location(message: &str) -> Option<(String, u64, u64)> {
    let delimiters = |c: char| c.is_whitespace() || matches!(c, '(' | ')' | '"' | ',');
    for token in message.split(delimiters) {
        let token = token.trim_end_matches(':');
        let Some((head, last)) = token.rsplit_once(':') else {
            continue;
        };
        let Ok(last) = last.parse::<u64>() else {
            continue;
        };
        let (path, line, column) = match head.rsplit_once(':') {
            Some((path, line)) if line.parse::<u64>().is_ok() => {
                (path, line.parse::<u64>().unwrap_or(1), last)
            }
            _ => (head, last, 1),
        };
        if let Some((_, rel)) = path.split_once('/') {
            if !rel.is_empty() {
                return Some((rel.to_owned(), line.max(1), column.max(1)));
            }
        }
    }
    None
}

fn pick_file(chart_dir: &str, rel: Option<&str>, files: &[&str]) -> Result<String, ParseError> {
    if let Some(rel) = rel {
        let candidate = join(chart_dir, rel);
        if files.contains(&candidate.as_str()) {
            return Ok(candidate);
        }
    }
    let chart = join(chart_dir, "Chart.yaml");
    if files.contains(&chart.as_str()) {
        Ok(chart)
    } else {
        Err(ParseError::UnknownFile {
            tool: TOOL,
            path: chart,
        })
    }
}

fn finding(
    file: String,
    rule: &str,
    message: String,
    severity: ToolSeverity,
    at: (u64, u64),
) -> FileFinding {
    FileFinding {
        file,
        finding: Finding {
            tool_id: TOOL.to_owned(),
            rule_id: rule.to_owned(),
            message,
            severity,
            start: TextPosition {
                line: at.0,
                column: at.1,
            },
            end: None,
            suggestions: Vec::new(),
            encoding: ColumnEncoding::Scalars,
            related: Vec::new(),
        },
    }
}

fn one(
    rule: &str,
    message: &str,
    severity: ToolSeverity,
    named: Option<&str>,
    chart_dir: &str,
    files: &[&str],
) -> Result<FileFinding, ParseError> {
    let message = message.trim();
    if message.is_empty() {
        return Err(ParseError::shape(TOOL, "a diagnostic without a message"));
    }
    let (rel, at) = match location(message) {
        Some((rel, line, column)) => (Some(rel), (line, column)),
        None => (named.map(str::to_owned), (1, 1)),
    };
    let file = pick_file(chart_dir, rel.as_deref(), files)?;
    Ok(finding(file, rule, message.to_owned(), severity, at))
}

/// Parses `helm lint --quiet`: `[ERROR] <path>: <message>` blocks with indented continuations.
///
/// `[INFO]` is dropped. Exit 1 needs an `[ERROR]` and exit 0 must not have one.
pub fn parse_helm_lint(
    stdout: &[u8],
    code: Option<i32>,
    chart_dir: &str,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    if !matches!(code, Some(0 | 1)) {
        return Err(ParseError::shape(TOOL, format!("exit {}", code_name(code))));
    }
    let text = as_text(TOOL, stdout)?;
    let mut blocks: Vec<(ToolSeverity, bool, String, String)> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty()
            || line.starts_with("==> ")
            || line.starts_with("Error: ")
            || line.starts_with("1 chart(s) linted")
        {
            continue;
        }
        let level = [
            ("[ERROR] ", Some(ToolSeverity::Error)),
            ("[WARNING] ", Some(ToolSeverity::Warning)),
            ("[INFO] ", None),
        ]
        .into_iter()
        .find_map(|(prefix, level)| line.strip_prefix(prefix).map(|rest| (rest, level)));
        match level {
            Some((rest, level)) => {
                let (path, message) = rest.split_once(": ").unwrap_or(("", rest));
                let severity = level.unwrap_or(ToolSeverity::Warning);
                blocks.push((
                    severity,
                    level.is_none(),
                    path.trim().to_owned(),
                    message.to_owned(),
                ));
            }
            None if line.starts_with([' ', '\t']) => match blocks.last_mut() {
                Some(block) => {
                    block.3.push('\n');
                    block.3.push_str(line.trim());
                }
                None => return Err(ParseError::shape(TOOL, format!("stray line {line:?}"))),
            },
            None => {
                return Err(ParseError::shape(
                    TOOL,
                    format!(
                        "unrecognised line {:?}",
                        line.chars().take(80).collect::<String>()
                    ),
                ))
            }
        }
    }
    let errors = blocks
        .iter()
        .filter(|block| block.0 == ToolSeverity::Error)
        .count();
    if (code == Some(1)) != (errors > 0) {
        return Err(ParseError::shape(
            TOOL,
            format!(
                "exit {} disagrees with {errors} error line(s)",
                code_name(code)
            ),
        ));
    }
    let mut findings = Vec::new();
    for (severity, info, path, message) in &blocks {
        if *info {
            continue;
        }
        let named = (!path.is_empty() && !path.ends_with('/')).then_some(path.as_str());
        let rule = if *severity == ToolSeverity::Error {
            "error"
        } else {
            "warning"
        };
        findings.push(one(rule, message, *severity, named, chart_dir, files)?);
    }
    Ok(findings)
}

/// Turns a `helm template` render failure on stderr into one finding on the owning template.
///
/// Stderr without any `Error:` line is outside the grammar and fails closed.
pub fn parse_helm_template_error(
    stderr: &[u8],
    chart_dir: &str,
    files: &[&str],
) -> Result<Vec<FileFinding>, ParseError> {
    let text = as_text(TOOL, stderr)?;
    let Some(start) = text.lines().position(|line| line.starts_with("Error: ")) else {
        return Err(ParseError::shape(
            TOOL,
            "helm template failed without an Error line",
        ));
    };
    let mut message = String::new();
    for (index, line) in text.lines().skip(start).enumerate() {
        if line.trim().is_empty() || line.starts_with("Use --debug") {
            break;
        }
        let line = if index == 0 {
            &line["Error: ".len()..]
        } else {
            line.trim()
        };
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(line);
    }
    if location(&message).is_none() {
        return Err(ParseError::shape(
            TOOL,
            format!("render failure names no template: {message}"),
        ));
    }
    Ok(vec![one(
        "template",
        &message,
        ToolSeverity::Error,
        None,
        chart_dir,
        files,
    )?])
}

/// Splits `helm template` output at its `# Source: <chart>/<path>` markers.
///
/// The first path segment is the chart name from Chart.yaml, not the directory, so it is cut.
pub fn split_rendered(stdout: &[u8]) -> Result<Vec<RenderedFile>, ParseError> {
    let text = as_text(TOOL, stdout)?;
    let mut groups: Vec<RenderedFile> = Vec::new();
    for line in text.lines() {
        if let Some(source) = line.strip_prefix(SOURCE) {
            let path = source
                .trim()
                .split_once('/')
                .map(|(_, rel)| rel)
                .filter(|rel| !rel.is_empty())
                .ok_or_else(|| ParseError::shape(TOOL, format!("bad source marker {line:?}")))?;
            groups.push(RenderedFile {
                path: path.to_owned(),
                text: String::new(),
            });
        } else if line == "---" {
            continue;
        } else if let Some(group) = groups.last_mut() {
            group.text.push_str(line);
            group.text.push('\n');
        } else if !line.trim().is_empty() {
            return Err(ParseError::shape(
                TOOL,
                "rendered text before the first source marker",
            ));
        }
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILES: &[&str] = &["c/Chart.yaml", "c/templates/a.yaml", "c/values.yaml"];

    #[test]
    fn helm_lint_reads_the_v4_template_error_position() {
        let out = b"==> Linting c\n[ERROR] templates/: demo/templates/a.yaml:4:18\n  executing \"demo/templates/a.yaml\" at <.Values.x>:\n    nil pointer\n\nError: 1 chart(s) linted, 1 chart(s) failed\n";
        let found = parse_helm_lint(out, Some(1), "c", FILES).expect("parses");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].file, "c/templates/a.yaml");
        assert_eq!(
            found[0].finding.start,
            TextPosition {
                line: 4,
                column: 18
            }
        );
        assert_eq!(found[0].finding.severity, ToolSeverity::Error);
    }

    #[test]
    fn helm_lint_maps_chart_yaml_and_levels() {
        let out = b"[ERROR] Chart.yaml: version 'bad' is not a valid SemVer\n[WARNING] Chart.yaml: odd\n[INFO] Chart.yaml: icon is recommended\n[ERROR] : unable to load chart\n\tvalidation: bad\n";
        let found = parse_helm_lint(out, Some(1), "c", FILES).expect("parses");
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|f| f.file == "c/Chart.yaml"));
        assert_eq!(found[1].finding.severity, ToolSeverity::Warning);
        assert!(found[2].finding.message.contains("validation: bad"));
    }

    #[test]
    fn helm_lint_fails_closed_on_disagreeing_exit() {
        assert!(parse_helm_lint(b"[ERROR] Chart.yaml: x\n", Some(0), "c", FILES).is_err());
        assert!(parse_helm_lint(b"", Some(1), "c", FILES).is_err());
        assert!(parse_helm_lint(b"", Some(2), "c", FILES).is_err());
        assert!(parse_helm_lint(b"", None, "c", FILES).is_err());
        assert!(parse_helm_lint(b"weird\n", Some(0), "c", FILES).is_err());
        assert!(parse_helm_lint(b"", Some(0), "c", FILES)
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn helm_lint_drops_info_only_output() {
        let out = b"[INFO] Chart.yaml: icon is recommended\n";
        assert!(parse_helm_lint(out, Some(0), "c", FILES)
            .expect("clean")
            .is_empty());
    }

    #[test]
    fn helm_template_error_names_the_template() {
        let err = b"Error: execution error at (demo/templates/a.yaml:1:6): boom\n\nUse --debug flag to render out invalid YAML\n";
        let found = parse_helm_template_error(err, "c", FILES).expect("parses");
        assert_eq!(found[0].file, "c/templates/a.yaml");
        assert_eq!(found[0].finding.start, TextPosition { line: 1, column: 6 });
        let multi = b"Error: demo/templates/a.yaml:1:13\n  executing \"x\"\n\nUse --debug flag\n";
        let found = parse_helm_template_error(multi, "c", FILES).expect("parses");
        assert_eq!(
            found[0].finding.start,
            TextPosition {
                line: 1,
                column: 13
            }
        );
        let bare = b"Error: parse error at (demo/templates/a.yaml:1): missing value for if\n";
        let found = parse_helm_template_error(bare, "c", FILES).expect("parses");
        assert_eq!(found[0].finding.start, TextPosition { line: 1, column: 1 });
    }

    #[test]
    fn helm_template_error_fails_closed() {
        assert!(parse_helm_template_error(b"", "c", FILES).is_err());
        assert!(parse_helm_template_error(b"Error: no such thing\n", "c", FILES).is_err());
        let other = b"Error: execution error at (demo/templates/gone.yaml:1:6): boom\n";
        assert!(parse_helm_template_error(other, "c", &["c/templates/a.yaml"]).is_err());
    }

    #[test]
    fn helm_split_rendered_strips_the_chart_name() {
        let out = b"---\n# Source: demo/templates/a.yaml\na: 1\n---\n# Source: demo/charts/sub/templates/b.yaml\nb: 2\n";
        let groups = split_rendered(out).expect("splits");
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].path, "templates/a.yaml");
        assert_eq!(groups[0].text, "a: 1\n");
        assert_eq!(groups[1].path, "charts/sub/templates/b.yaml");
        assert!(split_rendered(b"stray: 1\n").is_err());
        assert!(split_rendered(b"# Source: demo\n").is_err());
        assert!(split_rendered(b"").expect("empty").is_empty());
    }
}
