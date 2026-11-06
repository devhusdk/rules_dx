use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::check::ToolDef;
use super::*;

/// Where the Haskell family stages the `.hie` files of the target.
const HIE_REL: &str = "weeder.hie";
const CONFIG_FILE: &str = "weeder.toml";
/// Only `Main.main` and instance methods count as used, so an exported function nobody imports is
/// reported.
const DEFAULT_CONFIG: &str =
    "roots = [\"^Main\\\\.main$\"]\ntype-class-roots = true\nunused-types = false\n";

pub(super) const TOOL: ToolDef = ToolDef {
    id: "weeder",
    run: |backend, call| backend.check_weeder(call.tool_id, call.tool, call.staged),
};

impl RealBackend {
    /// Runs weeder over the staged `.hie` files and keeps the findings in the target's own files.
    pub(super) fn check_weeder(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        let hie = scratch
            .resolve(Path::new(HIE_REL))
            .map_err(|err| execution(tool_id, format!("hie directory: {err}")))?;
        if !hie.is_dir() {
            return Err(execution(
                tool_id,
                "the target wrote no .hie files; weeder needs the Haskell compile replay"
                    .to_owned(),
            ));
        }
        let config = scratch.root().join(CONFIG_FILE);
        std::fs::write(&config, DEFAULT_CONFIG)
            .map_err(|err| execution(tool_id, format!("weeder config: {err}")))?;
        let invocation = commands::weeder_check(&tool.binary, &config, &hie);
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        let names: Vec<&str> = staged.pairs.iter().map(|(rel, _)| rel.as_str()).collect();
        let mut findings = parsed(
            tool_id,
            parsers::parse_weeder(&out.stdout, &out.stderr, out.code, &names),
        )?;
        for found in &mut findings {
            if let Some((_, absolute)) = staged.pairs.iter().find(|(rel, _)| *rel == found.file) {
                found.file = absolute.to_string_lossy().into_owned();
            }
        }
        Ok(findings)
    }
}
