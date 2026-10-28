use serde::Deserialize;

use super::{check_output_size, code_name, known, FileFinding, ParseError};
use crate::notebook::Notebook;
use crate::{ColumnEncoding, Finding, TextPosition, ToolSeverity};

#[derive(Debug, Deserialize)]
struct RuffPosition {
    column: u64,
    row: u64,
}

#[derive(Debug, Deserialize)]
struct RuffDiagnostic {
    #[serde(default)]
    cell: Option<u64>,
    code: String,
    filename: String,
    location: RuffPosition,
    end_location: RuffPosition,
    message: String,
    severity: String,
}

fn ruff_severity(tool: &'static str, severity: &str) -> Result<ToolSeverity, ParseError> {
    match severity {
        "error" => Ok(ToolSeverity::Error),
        "warning" => Ok(ToolSeverity::Warning),
        _ => Err(ParseError::Shape {
            tool,
            detail: format!("unknown severity: {severity}"),
        }),
    }
}

fn ruff_position(
    tool: &'static str,
    what: &str,
    position: &RuffPosition,
) -> Result<TextPosition, ParseError> {
    if position.row == 0 || position.column == 0 {
        return Err(ParseError::Shape {
            tool,
            detail: format!("zero {what} position"),
        });
    }
    Ok(TextPosition {
        line: position.row,
        column: position.column,
    })
}

/// Remaps Ruff's per-cell positions into notebook file positions.
struct Mapper<'a> {
    notebooks: &'a [(&'a str, &'a str)],
    parsed: Vec<(&'a str, Notebook<'a>)>,
}

impl<'a> Mapper<'a> {
    fn new(notebooks: &'a [(&'a str, &'a str)]) -> Self {
        Self {
            notebooks,
            parsed: Vec::new(),
        }
    }

    fn notebook(&mut self, tool: &'static str, path: &str) -> Result<&Notebook<'a>, ParseError> {
        if let Some(at) = self.parsed.iter().position(|(known, _)| *known == path) {
            return Ok(&self.parsed[at].1);
        }
        let Some((name, text)) = self.notebooks.iter().find(|(name, _)| *name == path) else {
            return Err(ParseError::Shape {
                tool,
                detail: format!("{path}: cell diagnostic for a notebook that was not read"),
            });
        };
        let notebook = Notebook::parse(text).map_err(|err| ParseError::Shape {
            tool,
            detail: format!("{path}: {err}"),
        })?;
        self.parsed.push((name, notebook));
        Ok(&self.parsed[self.parsed.len() - 1].1)
    }

    /// Start, end and message of one diagnostic, in file coordinates.
    fn place(
        &mut self,
        tool: &'static str,
        checked: &str,
        diagnostic: &RuffDiagnostic,
    ) -> Result<(TextPosition, TextPosition, String), ParseError> {
        let start = ruff_position(tool, "start", &diagnostic.location)?;
        let end = ruff_position(tool, "end", &diagnostic.end_location)?;
        let Some(cell) = diagnostic.cell else {
            if checked.ends_with(".ipynb") && (start.line, start.column) != (1, 1) {
                return Err(ParseError::Shape {
                    tool,
                    detail: format!("{checked}: notebook diagnostic without a cell"),
                });
            }
            return Ok((start, end, diagnostic.message.clone()));
        };
        let notebook = self.notebook(tool, checked)?;
        let place = |position: &TextPosition| {
            notebook
                .locate(cell, position.line, position.column)
                .map(|(line, column)| TextPosition { line, column })
                .ok_or_else(|| ParseError::Shape {
                    tool,
                    detail: format!(
                        "{checked}: cell {cell}, line {} is outside the notebook",
                        position.line
                    ),
                })
        };
        let mapped = (place(&start)?, place(&end)?);
        Ok((
            mapped.0,
            mapped.1,
            format!("cell {cell}, line {}: {}", start.line, diagnostic.message),
        ))
    }
}

pub fn parse_ruff(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    notebooks: &[(&str, &str)],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "ruff";
    check_output_size(TOOL, stdout)?;
    let diagnostics: Vec<RuffDiagnostic> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::with_capacity(diagnostics.len());
    let mut mapper = Mapper::new(notebooks);
    for diagnostic in &diagnostics {
        let checked = known(TOOL, files, &diagnostic.filename)?;
        let (start, end, message) = mapper.place(TOOL, checked, diagnostic)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: TOOL.to_owned(),
                rule_id: diagnostic.code.clone(),
                message,
                severity: ruff_severity(TOOL, &diagnostic.severity)?,
                start,
                end: Some(end),
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with an empty findings array", code_name(code)),
        });
    }
    Ok(findings)
}

pub fn parse_ruff_format(
    stdout: &[u8],
    code: Option<i32>,
    files: &[&str],
    notebooks: &[(&str, &str)],
) -> Result<Vec<FileFinding>, ParseError> {
    const TOOL: &str = "ruff_format";
    check_output_size(TOOL, stdout)?;
    let diagnostics: Vec<RuffDiagnostic> =
        serde_json::from_slice(stdout).map_err(|err| ParseError::Json {
            tool: TOOL,
            detail: err.to_string(),
        })?;
    let mut findings = Vec::with_capacity(diagnostics.len());
    let mut mapper = Mapper::new(notebooks);
    for diagnostic in &diagnostics {
        if diagnostic.code != "unformatted" {
            return Err(ParseError::Shape {
                tool: TOOL,
                detail: format!("unexpected code {:?}", diagnostic.code),
            });
        }
        let checked = known(TOOL, files, &diagnostic.filename)?;
        let (start, end, message) = mapper.place(TOOL, checked, diagnostic)?;
        findings.push(FileFinding {
            file: checked.to_owned(),
            finding: Finding {
                tool_id: "ruff".to_owned(),
                rule_id: diagnostic.code.clone(),
                message,
                severity: ruff_severity(TOOL, &diagnostic.severity)?,
                start,
                end: Some(end),
                suggestions: Vec::new(),
                encoding: ColumnEncoding::Scalars,
                related: Vec::new(),
            },
        });
    }
    if findings.is_empty() && code != Some(0) {
        return Err(ParseError::Shape {
            tool: TOOL,
            detail: format!("exit {} with an empty findings array", code_name(code)),
        });
    }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUFF_LINT_DIRTY: &str = r#"[{"cell":null,"code":"F401","end_location":{"column":10,"row":1},"filename":"/s/dirty.py","fix":{"applicability":"safe","edits":[{"content":"","end_location":{"column":1,"row":2},"location":{"column":1,"row":1}}],"message":"Remove unused import: `os`"},"location":{"column":8,"row":1},"message":"`os` imported but unused","name":"unused-import","noqa_row":1,"severity":"error","url":"https://docs.astral.sh/ruff/rules/unused-import"}]"#;

    const RUFF_FORMAT_DIRTY: &str = r#"[{"cell":null,"code":"unformatted","end_location":{"column":7,"row":5},"filename":"/s/dirty.py","fix":{"applicability":"safe","edits":[{"content":" = ","end_location":{"column":7,"row":5},"location":{"column":6,"row":5}}],"message":null},"location":{"column":6,"row":5},"message":"File would be reformatted","name":"unformatted","noqa_row":null,"severity":"error","url":null}]"#;

    #[test]
    fn ruff_reports_lint_diagnostics() {
        let findings =
            parse_ruff(RUFF_LINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"], &[]).expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].file, "/s/dirty.py");
        assert_eq!(findings[0].finding.tool_id, "ruff");
        assert_eq!(findings[0].finding.rule_id, "F401");
        assert_eq!(findings[0].finding.message, "`os` imported but unused");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Error);
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 1, column: 8 },
                Some(TextPosition {
                    line: 1,
                    column: 10
                })
            )
        );
        assert!(findings[0].finding.suggestions.is_empty());
        assert!(parse_ruff(b"[]", Some(0), &["/s/dirty.py"], &[])
            .expect("parsed")
            .is_empty());
        assert!(parse_ruff(b"[]", Some(1), &["/s/dirty.py"], &[]).is_err());
        assert!(parse_ruff(RUFF_LINT_DIRTY.as_bytes(), Some(1), &["/s/other.py"], &[]).is_err());
        assert!(parse_ruff(b"not json", Some(1), &["/s/dirty.py"], &[]).is_err());
    }

    #[test]
    fn ruff_format_accepts_only_unformatted() {
        let findings =
            parse_ruff_format(RUFF_FORMAT_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"], &[])
                .expect("parsed");
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding.tool_id, "ruff");
        assert_eq!(findings[0].finding.rule_id, "unformatted");
        assert_eq!(findings[0].finding.message, "File would be reformatted");
        assert_eq!(
            (findings[0].finding.start, findings[0].finding.end),
            (
                TextPosition { line: 5, column: 6 },
                Some(TextPosition { line: 5, column: 7 })
            )
        );
        assert!(parse_ruff_format(b"[]", Some(0), &["/s/dirty.py"], &[])
            .expect("parsed")
            .is_empty());
        assert!(
            parse_ruff_format(RUFF_LINT_DIRTY.as_bytes(), Some(1), &["/s/dirty.py"], &[]).is_err()
        );
    }

    #[test]
    fn ruff_grammar_mismatches_are_fail_closed() {
        let warning = RUFF_LINT_DIRTY.replace("\"severity\":\"error\"", "\"severity\":\"warning\"");
        let findings =
            parse_ruff(warning.as_bytes(), Some(1), &["/s/dirty.py"], &[]).expect("parsed");
        assert_eq!(findings[0].finding.severity, ToolSeverity::Warning);
        let unknown = RUFF_LINT_DIRTY.replace("\"severity\":\"error\"", "\"severity\":\"info\"");
        assert!(parse_ruff(unknown.as_bytes(), Some(1), &["/s/dirty.py"], &[]).is_err());
        let zero_row = RUFF_LINT_DIRTY.replace(
            "\"location\":{\"column\":8,\"row\":1}",
            "\"location\":{\"column\":8,\"row\":0}",
        );
        assert!(parse_ruff(zero_row.as_bytes(), Some(1), &["/s/dirty.py"], &[]).is_err());
        let zero_col = RUFF_LINT_DIRTY.replace(
            "\"location\":{\"column\":8,\"row\":1}",
            "\"location\":{\"column\":0,\"row\":1}",
        );
        assert!(parse_ruff(zero_col.as_bytes(), Some(1), &["/s/dirty.py"], &[]).is_err());
        assert!(parse_ruff_format(b"not json", Some(1), &["/s/dirty.py"], &[]).is_err());
        assert!(parse_ruff_format(b"[]", Some(1), &["/s/dirty.py"], &[]).is_err());
    }

    const NB: &str = r##"{
 "cells": [
  {
   "cell_type": "markdown",
   "id": "m1",
   "metadata": {},
   "source": [
    "# Title"
   ]
  },
  {
   "cell_type": "code",
   "execution_count": null,
   "id": "c0",
   "metadata": {},
   "outputs": [],
   "source": [
    "x = 1"
   ]
  },
  {
   "cell_type": "code",
   "execution_count": null,
   "id": "c1",
   "metadata": {},
   "outputs": [],
   "source": [
    "y = 2"
   ]
  },
  {
   "cell_type": "code",
   "execution_count": null,
   "id": "c2",
   "metadata": {},
   "outputs": [],
   "source": [
    "import os\n",
    "print(x)"
   ]
  }
 ],
 "metadata": {
  "kernelspec": {
   "display_name": "Python 3",
   "language": "python",
   "name": "python3"
  },
  "language_info": {
   "name": "python"
  }
 },
 "nbformat": 4,
 "nbformat_minor": 5
}
"##;

    const NB_LINT: &str = r#"[{"cell":4,"code":"I001","end_location":{"column":10,"row":1},"filename":"/s/nb.ipynb","location":{"column":1,"row":1},"message":"Import block is un-sorted or un-formatted","name":"unsorted-imports","noqa_row":1,"severity":"error","url":"https://docs.astral.sh/ruff/rules/unsorted-imports"},{"cell":4,"code":"F401","end_location":{"column":10,"row":1},"filename":"/s/nb.ipynb","location":{"column":8,"row":1},"message":"`os` imported but unused","name":"unused-import","noqa_row":1,"severity":"error","url":"https://docs.astral.sh/ruff/rules/unused-import"}]"#;

    #[test]
    fn ruff_maps_notebook_cells_to_file_positions() {
        let found = parse_ruff(
            NB_LINT.as_bytes(),
            Some(1),
            &["/s/nb.ipynb"],
            &[("/s/nb.ipynb", NB)],
        )
        .expect("parsed");
        assert_eq!(found.len(), 2);
        let import_line = NB
            .lines()
            .position(|line| line.contains("import os"))
            .expect("line") as u64
            + 1;
        let unused = &found[1].finding;
        assert_eq!(unused.rule_id, "F401");
        assert_eq!(unused.message, "cell 4, line 1: `os` imported but unused");
        // The line holds `    "import os\n",` so `os` starts at scalar 13.
        assert_eq!(
            unused.start,
            TextPosition {
                line: import_line,
                column: 13
            }
        );
        assert_eq!(
            unused.end,
            Some(TextPosition {
                line: import_line,
                column: 15
            })
        );
    }

    #[test]
    fn ruff_notebook_diagnostics_fail_closed() {
        let names = ["/s/nb.ipynb"];
        assert!(parse_ruff(NB_LINT.as_bytes(), Some(1), &names, &[]).is_err());
        let wrong_cell = NB_LINT.replace("\"cell\":4", "\"cell\":9");
        assert!(parse_ruff(
            wrong_cell.as_bytes(),
            Some(1),
            &names,
            &[("/s/nb.ipynb", NB)]
        )
        .is_err());
        assert!(parse_ruff(NB_LINT.as_bytes(), Some(1), &names, &[("/s/nb.ipynb", "{")]).is_err());
        let no_cell = NB_LINT.replace("\"cell\":4", "\"cell\":null");
        assert!(parse_ruff(no_cell.as_bytes(), Some(1), &names, &[("/s/nb.ipynb", NB)]).is_err());
        let file_level = r#"[{"cell":null,"code":"E902","end_location":{"column":1,"row":1},"filename":"/s/nb.ipynb","location":{"column":1,"row":1},"message":"bad","severity":"error"}]"#;
        let found = parse_ruff(file_level.as_bytes(), Some(1), &names, &[]).expect("parsed");
        assert_eq!(found[0].finding.message, "bad");
    }

    #[test]
    fn ruff_format_maps_notebook_cells() {
        let format = r#"[{"cell":2,"code":"unformatted","end_location":{"column":10,"row":1},"filename":"/s/nb.ipynb","location":{"column":2,"row":1},"message":"File would be reformatted","severity":"error"}]"#;
        let found = parse_ruff_format(
            format.as_bytes(),
            Some(1),
            &["/s/nb.ipynb"],
            &[("/s/nb.ipynb", NB)],
        )
        .expect("parsed");
        assert_eq!(
            found[0].finding.message,
            "cell 2, line 1: File would be reformatted"
        );
        assert!(found[0].finding.start.line > 1);
    }
}
