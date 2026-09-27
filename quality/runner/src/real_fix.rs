use quality_adapter::commands;
use quality_adapter::exec::MirrorContents;

use super::*;

impl super::RealBackend {
    pub fn apply_fix(
        &self,
        tool_id: &str,
        path: &str,
        text: &str,
        capability: &str,
    ) -> Result<String, RunnerError> {
        if capability == "audit" || capability == "typecheck" {
            self.tool(tool_id)?;
            return Ok(text.to_owned());
        }
        let tool = self.tool(tool_id)?;
        match tool_id {
            "rustfmt" | "buildifier" | "taplo" | "google_java_format" | "ktfmt" => {
                self.run_fix(tool_id, tool, path, text)
            }
            "ruff" => self.run_ruff_fix(tool, path, text, capability == "format"),
            "vale" | "markdown_check" | "rustc" | "ty" | "pydoclint" | "flake8" | "pylint"
            | "clippy" | "scalafix" | "roslyn" | "fsharplint" | "checkstyle" | "pmd"
            | "spotbugs" | "qmllint" | "clang_tidy" | "cppcheck" | "staticcheck" | "govet"
            | "errcheck" | "stylelint" | "rubocop" | "psscriptanalyzer" | "yamllint"
            | "shellcheck" | "keep_sorted" => Ok(text.to_owned()),
            "buf" => {
                if capability == "format" {
                    self.run_buf_format_fix(tool, path, text)
                } else {
                    Ok(text.to_owned())
                }
            }
            "djlint" => {
                if capability == "format" {
                    self.run_djlint_format_fix(tool, path, text)
                } else {
                    Ok(text.to_owned())
                }
            }
            "biome" => {
                if capability == "format" {
                    self.run_biome_format_fix(tool, path, text)
                } else {
                    Ok(text.to_owned())
                }
            }
            "prettier" => {
                if capability == "format" {
                    self.run_prettier_fix(tool, path, text)
                } else {
                    Ok(text.to_owned())
                }
            }
            "scalafmt" => self.run_scalafmt_fix(tool, path, text),
            "csharpier" => self.run_csharpier_fix(tool, path, text),
            "fantomas" => self.run_fantomas_fix(tool, path, text),
            "clang_format" => self.run_clang_format_fix(tool, path, text),
            "gofumpt" => self.run_gofumpt_fix(tool, path, text),
            "qmlformat" => self.run_qmlformat_fix(tool, path, text),
            "eslint" => self.run_eslint_fix(tool, path, text),
            "ktlint" => self.run_ktlint_fix(tool, path, text),
            "cue" => self.run_cue_fix(tool, path, text),
            "jsonnetfmt" => self.run_jsonnetfmt_fix(tool, path, text),
            "pkl" => self.run_pkl_fix(tool, path, text),
            "modfmt" => self.run_modfmt_fix(tool, path, text),
            "terraform" => self.run_terraform_fix(tool, path, text),
            "yamlfmt" => self.run_yamlfmt_fix(tool, path, text),
            "shfmt" => self.run_shfmt_fix(tool, path, text),
            "standardrb" => self.run_standardrb_fix(tool, path, text),
            _ => Err(execution(
                tool_id,
                format!("unsupported real tool: {tool_id}"),
            )),
        }
    }

    fn fix_scratch(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<(Scratch, PathBuf), RunnerError> {
        let scratch = fresh_scratch(&self.scratch_parent, tool_id)?;
        let mut mirrors = vec![MirrorFile {
            mirror_rel: PathBuf::from(path),
            contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
        }];
        mirrors.extend(Self::mirror_tool_files(tool_id, tool));
        write_all(&scratch, tool_id, &mirrors)?;
        let absolute = scratch.root().join(path);
        Ok((scratch, absolute))
    }

    fn reread_fixed(tool_id: &str, absolute: &Path) -> Result<String, RunnerError> {
        let fixed = std::fs::read(absolute)
            .map_err(|err| execution(tool_id, format!("re-read fixed file: {err}")))?;
        String::from_utf8(fixed).map_err(|err| RunnerError::ToolOutput {
            tool_id: tool_id.to_owned(),
            detail: format!("fixed file is not UTF-8: {err}"),
        })
    }

    fn run_fix(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        let (scratch, absolute) = self.fix_scratch(tool_id, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config = self.config_abs(tool_id, tool, &scratch)?;
        let cwd_rel = Self::cwd_rel(tool_id, tool.config_rel.as_deref());
        let invocation = match tool_id {
            "rustfmt" => {
                let Some(cfg) = config.as_ref() else {
                    return Err(execution(tool_id, "rustfmt requires a config".to_owned()));
                };
                commands::rustfmt(
                    &tool.binary,
                    &refs,
                    cfg,
                    Self::rustfmt_edition(tool)?,
                    false,
                )
            }
            "buildifier" => commands::buildifier_fix(
                &tool.binary,
                &refs,
                hint_dir(tool.config_rel.as_deref(), &cwd_rel),
            ),
            "google_java_format" => commands::google_java_format_fix(&tool.binary, &refs),
            "ktfmt" => commands::ktfmt_fix(&tool.binary, &refs),
            _ => commands::taplo_format(&tool.binary, &refs, config.as_deref(), false),
        };
        let out = self.run(tool_id, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(tool_id, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(tool_id, &absolute)?;
        cleaned(tool_id, scratch, fixed)
    }

    fn run_ruff_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
        format: bool,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "ruff";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config = self.config_abs(TOOL_ID, tool, &scratch)?;
        let invocation = if format {
            commands::ruff_format_fix(&tool.binary, &refs, config.as_deref())
        } else {
            commands::ruff_fix(&tool.binary, &refs, config.as_deref())
        };
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        let keep_input = if format {
            out.code != Some(0)
        } else {
            out.code != Some(0) && out.code != Some(1)
        };
        if keep_input {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_biome_format_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "biome";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config_dir = Self::biome_config_dir(tool, &scratch)?;
        let invocation = commands::biome_format_fix(&tool.binary, &refs, &config_dir);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_prettier_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "prettier";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::prettier_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_eslint_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "eslint";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config = self.config_abs(TOOL_ID, tool, &scratch)?;
        let Some(cfg) = config.as_deref() else {
            return Err(execution(TOOL_ID, "eslint requires a config".to_owned()));
        };
        let invocation = commands::eslint_fix(&tool.binary, &refs, cfg);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) && out.code != Some(1) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_scalafmt_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "scalafmt";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config = self.config_abs(TOOL_ID, tool, &scratch)?;
        let invocation = commands::scalafmt_fix(&tool.binary, &refs, config.as_deref());
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_csharpier_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "csharpier";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config = self.config_abs(TOOL_ID, tool, &scratch)?;
        let invocation = commands::csharpier_fix(&tool.binary, &refs, config.as_deref());
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_fantomas_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "fantomas";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::fantomas_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_ktlint_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "ktlint";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::ktlint_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) && out.code != Some(1) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_buf_format_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "buf";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::buf_format_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_clang_format_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "clang_format";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let config = self.config_abs(TOOL_ID, tool, &scratch)?;
        let invocation = commands::clang_format_fix(&tool.binary, &refs, config.as_deref());
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_gofumpt_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "gofumpt";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::gofumpt_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_qmlformat_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "qmlformat";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::qmlformat_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_cue_fix(&self, tool: &RealTool, path: &str, text: &str) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "cue";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::cue_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_jsonnetfmt_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "jsonnetfmt";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::jsonnetfmt_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_pkl_fix(&self, tool: &RealTool, path: &str, text: &str) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "pkl";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::pkl_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_modfmt_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "modfmt";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::modfmt_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_terraform_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "terraform";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::terraform_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_yamlfmt_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "yamlfmt";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::yamlfmt_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_shfmt_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "shfmt";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::shfmt_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_standardrb_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "standardrb";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::standardrb_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }

    fn run_djlint_format_fix(
        &self,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        const TOOL_ID: &str = "djlint";
        let (scratch, absolute) = self.fix_scratch(TOOL_ID, tool, path, text)?;
        let refs = [absolute.as_path()];
        let invocation = commands::djlint_format_fix(&tool.binary, &refs);
        let out = self.run(TOOL_ID, tool, &invocation, &scratch)?;
        if out.code != Some(0) {
            return cleaned(TOOL_ID, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(TOOL_ID, &absolute)?;
        cleaned(TOOL_ID, scratch, fixed)
    }
}
