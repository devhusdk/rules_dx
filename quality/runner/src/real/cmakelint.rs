use std::path::Path;

use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers};

use super::*;

const DEFAULT_CONFIG_REL: &str = "dx-cmakelint-default/.cmakelintrc";

/// The `.cmakelintrc` keys dx accepts. cmakelint silently ignores any other line, so an
/// allowlist is the only way a typo can fail instead of passing as clean.
const CONFIG_KEYS: &[&str] = &["filter=", "spaces=", "linelength="];

/// Rejects a config cmakelint would half-read.
///
/// Only blank lines, `#` comments and the `CONFIG_KEYS` are allowed. `quiet` is rejected
/// because it drops the `Total Errors` summary on clean runs, which the parser requires.
fn check_config(text: &str) -> Result<(), String> {
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "quiet" {
            return Err(format!(
                "cmakelint config line {}: `quiet` is not supported because it drops the summary dx checks",
                index + 1
            ));
        }
        if !CONFIG_KEYS.iter().any(|key| line.starts_with(key)) {
            return Err(format!(
                "cmakelint config line {}: {line:?} is not one of {}; cmakelint would silently ignore it",
                index + 1,
                CONFIG_KEYS.join(", ")
            ));
        }
    }
    Ok(())
}

impl RealBackend {
    /// Lints every staged CMake file in one cmakelint run, with a preflighted config.
    pub(super) fn check_cmakelint(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: Option<&Path>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "cmakelint has no delegated mode".to_owned(),
            ));
        }
        if staged.pairs.is_empty() {
            return Ok(Vec::new());
        }
        let scratch = &staged.scratch;
        let config_path = match config {
            Some(path) => {
                let text = std::fs::read_to_string(path)
                    .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
                check_config(&text).map_err(|err| execution(tool_id, err))?;
                path.to_path_buf()
            }
            None => {
                let default = scratch
                    .resolve(Path::new(DEFAULT_CONFIG_REL))
                    .map_err(|err| execution(tool_id, format!("scratch config: {err}")))?;
                if let Some(parent) = default.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| execution(tool_id, format!("scratch config: {err}")))?;
                }
                std::fs::write(&default, b"")
                    .map_err(|err| execution(tool_id, format!("scratch config: {err}")))?;
                default
            }
        };
        let linted: Vec<&Path> = staged
            .pairs
            .iter()
            .map(|(_, absolute)| absolute.as_path())
            .collect();
        let invocation = commands::cmakelint_check(&tool.binary, &linted, &config_path);
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        let names: Vec<String> = linted
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let strs: Vec<&str> = names.iter().map(String::as_str).collect();
        parsed(
            tool_id,
            parsers::parse_cmakelint(&out.stdout, &out.stderr, out.code, &strs),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::check_config;

    #[test]
    fn config_allows_comments_and_the_three_keys() {
        assert!(
            check_config("# c\n\nfilter=-whitespace/indent\nspaces=4\n  linelength=100\n").is_ok()
        );
        assert!(check_config("").is_ok());
    }

    #[test]
    fn config_rejects_quiet_and_unknown_keys() {
        assert!(check_config("quiet\n").is_err());
        assert!(check_config("spaces=4\nquiet\n").is_err());
        assert!(check_config("spacs=4\n").is_err());
        assert!(check_config("filter -a\n").is_err());
    }
}
