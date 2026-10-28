use std::path::Path;

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers, sqlfluff_preflight};

use super::*;

impl RealBackend {
    /// Preflights the bound config, then lints every owned file in one sqlfluff run.
    pub(super) fn check_sqlfluff(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: &Path,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "sqlfluff has no delegated mode".to_owned(),
            ));
        }
        let text = std::fs::read_to_string(config)
            .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
        sqlfluff_preflight::check_config(&text).map_err(|err| execution(tool_id, err))?;
        let files: Vec<&Path> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        if files.is_empty() {
            return Ok(Vec::new());
        }
        for file in &files {
            // Unreadable or non-UTF-8 files are left to sqlfluff to report.
            if let Ok(text) = std::fs::read_to_string(file) {
                sqlfluff_preflight::check_inline_config(&text)
                    .map_err(|err| execution(tool_id, format!("{err} (in {})", file.display())))?;
            }
        }
        let invocation = commands::sqlfluff_check(&tool.binary, &files, config);
        let out = self.run(tool_id, tool, &invocation, &staged.scratch)?;
        let names: Vec<String> = files
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        parsed(
            tool_id,
            parsers::parse_sqlfluff(&out.stdout, &out.stderr, out.code, &strs),
        )
    }
}
