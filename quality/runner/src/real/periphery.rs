use std::path::Path;

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::check::ToolDef;
use super::*;

/// Where the Swift family stages the target's index store.
const INDEX_STORE_REL: &str = "periphery.indexstore";

pub(super) const TOOL: ToolDef = ToolDef {
    id: "periphery",
    run: |backend, call| backend.check_periphery(call.tool_id, call.tool, call.staged),
};

impl RealBackend {
    /// Scans the staged index store and keeps the findings in the target's own Swift files.
    pub(super) fn check_periphery(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "periphery has no delegated mode".to_owned(),
            ));
        }
        let scratch = &staged.scratch;
        let store = scratch
            .resolve(Path::new(INDEX_STORE_REL))
            .map_err(|err| execution(tool_id, format!("index store: {err}")))?;
        if !store.is_dir() {
            return Err(execution(
                tool_id,
                "the target wrote no index store; build it with the swift.index_while_building feature"
                    .to_owned(),
            ));
        }
        let invocation = commands::periphery_check(&tool.binary, &store);
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        let names: Vec<&str> = staged.pairs.iter().map(|(rel, _)| rel.as_str()).collect();
        let mut findings = parsed(
            tool_id,
            parsers::parse_periphery(&out.stdout, &out.stderr, out.code, &names),
        )?;
        for found in &mut findings {
            if let Some((_, absolute)) = staged.pairs.iter().find(|(rel, _)| *rel == found.file) {
                found.file = absolute.to_string_lossy().into_owned();
            }
        }
        Ok(findings)
    }
}
