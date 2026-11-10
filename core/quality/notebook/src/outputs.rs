//! Output-stripping check: finds and removes cell outputs and execution counts.
//!
//! Only the byte spans of `outputs` and `execution_count` are rewritten, so
//! every other byte of the notebook stays as it was.

use crate::{Cell, Notebook, NotebookError, Span};

/// Rule id for a code cell whose `outputs` array is not empty.
pub const CELL_OUTPUTS: &str = "cell-outputs";
/// Rule id for a code cell whose `execution_count` is not null.
pub const EXECUTION_COUNT: &str = "execution-count";

/// One output-check finding, located in the notebook file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFinding {
    /// 1-based cell number.
    pub cell: usize,
    /// 1-based file line of the offending field.
    pub line: u64,
    /// 1-based scalar column of the offending field.
    pub column: u64,
    /// Stable rule id.
    pub rule: &'static str,
    /// Human-readable message, starting with `cell N:`.
    pub message: String,
}

fn dirty_outputs(text: &str, cell: &Cell) -> Option<Span> {
    let span = cell.outputs?;
    let raw = &text[span.0..span.1];
    let inner = raw.strip_prefix('[')?.strip_suffix(']')?;
    (!inner.trim().is_empty()).then_some(span)
}

fn dirty_count(text: &str, cell: &Cell) -> Option<Span> {
    let span = cell.execution_count?;
    (&text[span.0..span.1] != "null").then_some(span)
}

fn position(text: &str, offset: usize) -> (u64, u64) {
    let before = &text[..offset];
    let line = before.matches('\n').count() as u64 + 1;
    let start = before.rfind('\n').map_or(0, |at| at + 1);
    (line, before[start..].chars().count() as u64 + 1)
}

fn dirty_fields(notebook: &Notebook<'_>) -> Vec<(usize, &'static str, Span)> {
    let mut found = Vec::new();
    for (at, cell) in notebook.cells.iter().enumerate() {
        if cell.kind != "code" {
            continue;
        }
        if let Some(span) = dirty_outputs(notebook.text, cell) {
            found.push((at + 1, CELL_OUTPUTS, span));
        }
        if let Some(span) = dirty_count(notebook.text, cell) {
            found.push((at + 1, EXECUTION_COUNT, span));
        }
    }
    found.sort_by_key(|(_, _, span)| span.0);
    found
}

/// Lists the code cells that keep outputs or an execution count.
pub fn output_findings(text: &str) -> Result<Vec<OutputFinding>, NotebookError> {
    let notebook = Notebook::parse(text)?;
    crate::validate(text)?;
    Ok(dirty_fields(&notebook)
        .into_iter()
        .map(|(cell, rule, span)| {
            let (line, column) = position(text, span.0);
            let message = if rule == CELL_OUTPUTS {
                format!("cell {cell}: code cell has saved outputs")
            } else {
                format!("cell {cell}: code cell has a non-null execution_count")
            };
            OutputFinding {
                cell,
                line,
                column,
                rule,
                message,
            }
        })
        .collect())
}

/// Clears `outputs` to `[]` and `execution_count` to `null` in code cells,
/// leaving every other byte untouched.
pub fn strip_outputs(text: &str) -> Result<String, NotebookError> {
    let notebook = Notebook::parse(text)?;
    crate::validate(text)?;
    let mut edits: Vec<(Span, &str)> = dirty_fields(&notebook)
        .into_iter()
        .map(|(_, rule, span)| (span, if rule == CELL_OUTPUTS { "[]" } else { "null" }))
        .collect();
    edits.sort_by_key(|(span, _)| std::cmp::Reverse(span.0));
    let mut out = text.to_owned();
    for ((start, end), replacement) in edits {
        out.replace_range(start..end, replacement);
    }
    Notebook::parse(&out)
        .map_err(|err| NotebookError::Splice(format!("result is not a notebook: {err}")))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nb(cells: &str, meta: &str) -> String {
        format!(
            "{{\r\n \"cells\": [{cells}],\r\n \"metadata\": {meta},\r\n \"nbformat\": 4,\r\n \"nbformat_minor\": 5\r\n}}\r\n"
        )
    }

    const MD: &str = "{\"cell_type\": \"markdown\", \"attachments\": {\"a\": 1}, \"metadata\": {}, \"source\": [\"# hi\"]}";
    const CLEAN: &str = "{\"cell_type\": \"code\", \"execution_count\": null, \"metadata\": {}, \"outputs\": [], \"source\": [\"x = 1\"]}";
    const DIRTY: &str = "{\"cell_type\": \"code\",\n    \"execution_count\": 3,\n \"metadata\": {},\n \"outputs\": [\n  {\"name\": \"stdout\", \"output_type\": \"stream\", \"text\": [\"1\\n\"]}\n ],\n \"source\": [\"print(1)\"]}";

    #[test]
    fn clean_notebook_has_no_findings() {
        let text = nb(&format!("{MD}, {CLEAN}"), "{}");
        assert_eq!(output_findings(&text), Ok(Vec::new()));
        assert_eq!(strip_outputs(&text).as_deref(), Ok(text.as_str()));
    }

    #[test]
    fn outputs_after_markdown_report_cell_and_line() {
        let text = nb(&format!("{MD}, {CLEAN}, {DIRTY}"), "{}");
        let found = output_findings(&text).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].rule, EXECUTION_COUNT);
        assert_eq!(found[1].rule, CELL_OUTPUTS);
        assert!(found[0].message.starts_with("cell 3:"));
        let line_of = |needle: &str| {
            text.lines()
                .enumerate()
                .filter(|(_, l)| l.contains(needle))
                .last()
                .unwrap()
                .0 as u64
                + 1
        };
        assert_eq!(found[0].line, line_of("\"execution_count\": 3"));
        assert_eq!(found[1].line, line_of("\"outputs\": ["));
    }

    #[test]
    fn count_without_outputs_reports_only_execution_count() {
        let cell = "{\"cell_type\": \"code\", \"execution_count\": 7, \"metadata\": {}, \"outputs\": [], \"source\": []}";
        let found = output_findings(&nb(cell, "{}")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, EXECUTION_COUNT);
    }

    #[test]
    fn missing_outputs_key_is_not_flagged_or_inserted() {
        let cell = "{\"cell_type\": \"markdown\", \"metadata\": {}, \"source\": []}";
        let raw = "{\"cell_type\": \"code\", \"metadata\": {}, \"source\": []}";
        let text = nb(&format!("{cell}, {raw}"), "{}");
        assert_eq!(output_findings(&text), Ok(Vec::new()));
        assert_eq!(strip_outputs(&text).as_deref(), Ok(text.as_str()));
    }

    #[test]
    fn strip_touches_only_outputs_and_counts() {
        let text = nb(&format!("{MD}, {DIRTY}"), "{\"x\": [1, 2]}");
        let stripped = strip_outputs(&text).unwrap();
        let want = text
            .replace("\"execution_count\": 3", "\"execution_count\": null")
            .replace(
                "[\n  {\"name\": \"stdout\", \"output_type\": \"stream\", \"text\": [\"1\\n\"]}\n ]",
                "[]",
            );
        assert_eq!(stripped, want);
        assert!(stripped.contains("\r\n"));
        assert_eq!(output_findings(&stripped), Ok(Vec::new()));
        assert_eq!(strip_outputs(&stripped), Ok(stripped));
    }

    #[test]
    fn invalid_input_is_an_error() {
        assert!(matches!(output_findings("{"), Err(NotebookError::Json(_))));
        let v3 = "{\"cells\": [], \"metadata\": {}, \"nbformat\": 3}";
        assert!(matches!(strip_outputs(v3), Err(NotebookError::Format(_))));
    }

    #[test]
    fn non_python_kernels_are_accepted() {
        let meta = "{\"language_info\": {\"name\": \"R\"}}";
        let text = nb(DIRTY, meta);
        assert_eq!(output_findings(&text).unwrap().len(), 2);
    }
}
