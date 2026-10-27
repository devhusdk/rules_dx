use quality_adapter::commands;
use quality_adapter::exec::MirrorContents;
use quality_adapter::launch::ResolvedTool;

use super::*;

/// Empty stand-ins for the other known files with these extensions.
pub(super) fn placeholders(
    path: &str,
    known: &BTreeSet<String>,
    extensions: &[&str],
) -> BTreeMap<String, String> {
    known
        .iter()
        .filter(|candidate| candidate.as_str() != path)
        .filter(|candidate| {
            Path::new(candidate.as_str())
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| extensions.contains(&ext))
        })
        .map(|candidate| (candidate.clone(), String::new()))
        .collect()
}

enum FixConfig {
    None,
    Optional,
    Required,
    BiomeDir,
    Buildifier,
}

/// Finds the stderr line that shows a tool crashed.
type CrashFinder = fn(&[u8]) -> Option<String>;

struct FixSpec {
    tool: &'static str,
    config: FixConfig,
    keep_exit_one: bool,
    /// Names a tool crash in stderr that must fail the fix even when the exit code is kept.
    crash_stderr: Option<CrashFinder>,
}

const RUFF_LINT_SPEC: FixSpec = FixSpec {
    tool: "ruff",
    config: FixConfig::Optional,
    keep_exit_one: true,
    crash_stderr: None,
};

const RUFF_FORMAT_SPEC: FixSpec = FixSpec {
    tool: "ruff",
    config: FixConfig::Optional,
    keep_exit_one: false,
    crash_stderr: None,
};

const FIX_SPECS: &[FixSpec] = &[
    FixSpec {
        tool: "buildifier",
        config: FixConfig::Buildifier,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "taplo",
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "google_java_format",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "ktfmt",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "biome",
        config: FixConfig::BiomeDir,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "prettier",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "eslint",
        config: FixConfig::Required,
        keep_exit_one: true,
        crash_stderr: None,
    },
    FixSpec {
        tool: "scalafmt",
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "csharpier",
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "fantomas",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "ktlint",
        config: FixConfig::Optional,
        keep_exit_one: true,
        crash_stderr: None,
    },
    FixSpec {
        tool: "buf",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "clang_format",
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "gofumpt",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "qmlformat",
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "cue",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "jsonnetfmt",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "zig_fmt",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "pkl",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "modfmt",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "terraform",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "yamlfmt",
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "shfmt",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "standardrb",
        config: FixConfig::Optional,
        keep_exit_one: true,
        crash_stderr: Some(quality_adapter::parsers::rubocop_crash),
    },
    FixSpec {
        tool: "djlint",
        config: FixConfig::Optional,
        keep_exit_one: true,
        crash_stderr: None,
    },
    FixSpec {
        tool: "keep_sorted",
        config: FixConfig::None,
        keep_exit_one: false,
        crash_stderr: None,
    },
];

fn fix_spec(tool_id: &str, format: bool) -> Option<&'static FixSpec> {
    if tool_id == "ruff" {
        return Some(if format {
            &RUFF_FORMAT_SPEC
        } else {
            &RUFF_LINT_SPEC
        });
    }
    FIX_SPECS.iter().find(|spec| spec.tool == tool_id)
}

impl super::RealBackend {
    pub fn apply_fix(
        &self,
        tool_id: &str,
        path: &str,
        text: &str,
        capability: &str,
    ) -> Result<String, RunnerError> {
        self.apply_fix_in_context(tool_id, path, text, capability, &BTreeSet::new())
    }

    pub fn apply_fix_in_context(
        &self,
        tool_id: &str,
        path: &str,
        text: &str,
        capability: &str,
        known: &BTreeSet<String>,
    ) -> Result<String, RunnerError> {
        if capability == "audit" || capability == "typecheck" {
            self.tool(tool_id)?;
            return Ok(text.to_owned());
        }
        let tool = self.tool(tool_id)?;
        match tool_id {
            "vale" | "markdown_check" | "rustc" | "ty" | "pydoclint" | "flake8" | "pylint"
            | "clippy" | "roslyn" | "fsharplint" | "checkstyle" | "pmd" | "spotbugs"
            | "qmllint" | "clang_tidy" | "cppcheck" | "staticcheck" | "govet" | "errcheck"
            | "stylelint" | "rubocop" | "psscriptanalyzer" | "yamllint" | "shellcheck"
            | "hadolint" => Ok(text.to_owned()),
            "graphql_eslint" => match tool.config_rel {
                Some(_) => Ok(text.to_owned()),
                None => Err(execution(
                    tool_id,
                    "graphql_eslint requires a config".to_owned(),
                )),
            },
            "buf" | "djlint" | "biome" | "prettier" => {
                if capability == "format" {
                    self.run_spec_fix(tool_id, tool, path, text, true)
                } else {
                    Ok(text.to_owned())
                }
            }
            "rustfmt" => self.fix_rustfmt(tool, path, text, known),
            "ruff" => self.run_spec_fix_with(
                tool_id,
                tool,
                path,
                text,
                capability == "format",
                &placeholders(path, known, &["py", "pyi"]),
            ),
            "scalafix" => self.run_scalafix_fix(tool_id, tool, path, text),
            "erlfmt" => self.run_rewrite_fix(tool_id, tool, path, text, commands::erlfmt_fix),
            "mix_format" => {
                self.run_rewrite_fix(tool_id, tool, path, text, commands::mix_format_fix)
            }
            "gleam_format" => {
                self.run_rewrite_fix(tool_id, tool, path, text, commands::gleam_format_fix)
            }
            "keep_sorted" => {
                if tool.upstream_diagnostics.is_empty() {
                    self.run_spec_fix(tool_id, tool, path, text, false)
                } else {
                    Ok(text.to_owned())
                }
            }
            _ => self.run_spec_fix(tool_id, tool, path, text, false),
        }
    }

    fn fix_scratch(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<(Scratch, PathBuf), RunnerError> {
        self.fix_scratch_with(tool_id, tool, path, text, &BTreeMap::new())
    }

    fn fix_scratch_with(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
        supporting: &BTreeMap<String, String>,
    ) -> Result<(Scratch, PathBuf), RunnerError> {
        let scratch = fresh_scratch(&self.scratch_parent, tool_id)?;
        let mut mirrors = vec![MirrorFile {
            mirror_rel: PathBuf::from(path),
            contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
        }];
        mirrors.extend(supporting.iter().map(|(rel, bytes)| MirrorFile {
            mirror_rel: PathBuf::from(rel),
            contents: MirrorContents::Bytes(bytes.as_bytes().to_vec()),
        }));
        let tools = Self::mirror_tool_files(tool_id, tool, &mirrors);
        mirrors.extend(tools);
        write_all(&scratch, tool_id, &mirrors)?;
        let absolute = scratch.root().join(path);
        Ok((scratch, absolute))
    }

    pub(super) fn reread_fixed(tool_id: &str, absolute: &Path) -> Result<String, RunnerError> {
        let fixed = std::fs::read(absolute)
            .map_err(|err| execution(tool_id, format!("re-read fixed file: {err}")))?;
        String::from_utf8(fixed).map_err(|err| RunnerError::ToolOutput {
            tool_id: tool_id.to_owned(),
            detail: format!("fixed file is not UTF-8: {err}"),
        })
    }

    fn build_fix_invocation(
        &self,
        spec: &FixSpec,
        tool_id: &str,
        tool: &RealTool,
        refs: &[&Path],
        scratch: &Scratch,
        format: bool,
    ) -> Result<Invocation, RunnerError> {
        match spec.config {
            FixConfig::Buildifier => {
                let cwd_rel = Self::cwd_rel(tool_id, tool.config_rel.as_deref());
                Ok(commands::buildifier_fix(
                    &tool.binary,
                    refs,
                    hint_dir(tool.config_rel.as_deref(), &cwd_rel),
                ))
            }
            FixConfig::BiomeDir => {
                let config_dir = Self::biome_config_dir(tool, scratch)?;
                Ok(commands::biome_format_fix(&tool.binary, refs, &config_dir))
            }
            FixConfig::Required => {
                let config = self.config_abs(tool_id, tool, scratch)?;
                let Some(cfg) = config.as_deref() else {
                    return Err(execution(tool_id, format!("{tool_id} requires a config")));
                };
                match tool_id {
                    "eslint" => {
                        let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                        Ok(commands::eslint_fix(&launch, refs, cfg)
                            .map_err(|err| execution(tool_id, err.to_string()))?)
                    }
                    _ => Err(execution(
                        tool_id,
                        format!("unsupported real tool: {tool_id}"),
                    )),
                }
            }
            FixConfig::Optional => {
                let config = self.config_abs(tool_id, tool, scratch)?;
                match tool_id {
                    "taplo" => Ok(commands::taplo_format(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                        false,
                    )),
                    "ruff" => Ok(if format {
                        commands::ruff_format_fix(&tool.binary, refs, config.as_deref())
                    } else {
                        commands::ruff_fix(&tool.binary, refs, config.as_deref())
                    }),
                    "scalafmt" => Ok(commands::scalafmt_fix(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                    )),
                    "yamlfmt" => Ok(commands::yamlfmt_fix(&tool.binary, refs, config.as_deref())),
                    "djlint" => Ok(commands::djlint_format_fix(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                    )),
                    "standardrb" => Ok(commands::standardrb_fix(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                    )),
                    "csharpier" => Ok(commands::csharpier_fix(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                    )),
                    "qmlformat" => Ok(commands::qmlformat_fix(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                    )),
                    "ktlint" => Ok(commands::ktlint_fix(&tool.binary, refs, config.as_deref())),
                    "clang_format" => Ok(commands::clang_format_fix(
                        &tool.binary,
                        refs,
                        config.as_deref(),
                    )),
                    _ => Err(execution(
                        tool_id,
                        format!("unsupported real tool: {tool_id}"),
                    )),
                }
            }
            FixConfig::None => match tool_id {
                "google_java_format" => Ok(commands::google_java_format_fix(&tool.binary, refs)),
                "ktfmt" => Ok(commands::ktfmt_fix(&tool.binary, refs)),
                "prettier" => {
                    let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                    Ok(commands::prettier_fix(&launch, refs)
                        .map_err(|err| execution(tool_id, err.to_string()))?)
                }
                "fantomas" => Ok(commands::fantomas_fix(&tool.binary, refs)),
                "buf" => Ok(commands::buf_format_fix(&tool.binary, refs, scratch.root())),
                "gofumpt" => Ok(commands::gofumpt_fix(&tool.binary, refs)),
                "cue" => Ok(commands::cue_fix(&tool.binary, refs)),
                "jsonnetfmt" => Ok(commands::jsonnetfmt_fix(&tool.binary, refs)),
                "pkl" => Ok(commands::pkl_fix(&tool.binary, refs)),
                "zig_fmt" => Ok(commands::zig_fmt_fix(&tool.binary, refs)),
                "modfmt" => Ok(commands::modfmt_fix(&tool.binary, refs)),
                "terraform" => Ok(commands::terraform_fix(&tool.binary, refs)),
                "shfmt" => Ok(commands::shfmt_fix(&tool.binary, refs)),
                "keep_sorted" => Ok(commands::keep_sorted_fix(&tool.binary, refs)),
                _ => Err(execution(
                    tool_id,
                    format!("unsupported real tool: {tool_id}"),
                )),
            },
        }
    }

    fn run_scalafix_fix(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
    ) -> Result<String, RunnerError> {
        let (scratch, absolute) = self.fix_scratch(tool_id, tool, path, text)?;
        let Some(config) = self.config_abs(tool_id, tool, &scratch)? else {
            return Err(execution(tool_id, "scalafix requires a config".to_owned()));
        };
        let classpath = Self::scalafix_classpath(tool, &scratch);
        let jars: Vec<&Path> = classpath.iter().map(PathBuf::as_path).collect();
        let context = commands::ScalafixContext {
            config: &config,
            sourceroot: scratch.root(),
            classpath: &jars,
        };
        let invocation = commands::scalafix_fix(&tool.binary, &[absolute.as_path()], &context)
            .map_err(|err| execution(tool_id, err.to_string()))?;
        let out = self.run(tool_id, tool, &invocation, &scratch)?;
        if !matches!(out.code, Some(0) | Some(64)) {
            return cleaned(tool_id, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(tool_id, &absolute)?;
        cleaned(tool_id, scratch, fixed)
    }

    fn run_rewrite_fix(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
        fix: fn(&Path, &[&Path]) -> Invocation,
    ) -> Result<String, RunnerError> {
        let (scratch, absolute) = self.fix_scratch(tool_id, tool, path, text)?;
        let invocation = fix(&tool.binary, &[absolute.as_path()]);
        let out = self.run(tool_id, tool, &invocation, &scratch)?;
        if out.code != Some(0) || !out.stderr.is_empty() {
            return cleaned(tool_id, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(tool_id, &absolute)?;
        cleaned(tool_id, scratch, fixed)
    }

    fn run_spec_fix(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
        format: bool,
    ) -> Result<String, RunnerError> {
        self.run_spec_fix_with(tool_id, tool, path, text, format, &BTreeMap::new())
    }

    fn run_spec_fix_with(
        &self,
        tool_id: &str,
        tool: &RealTool,
        path: &str,
        text: &str,
        format: bool,
        supporting: &BTreeMap<String, String>,
    ) -> Result<String, RunnerError> {
        let Some(spec) = fix_spec(tool_id, format) else {
            return Err(execution(
                tool_id,
                format!("unsupported real tool: {tool_id}"),
            ));
        };
        let (scratch, absolute) = self.fix_scratch_with(tool_id, tool, path, text, supporting)?;
        let refs = [absolute.as_path()];
        let invocation = self.build_fix_invocation(spec, tool_id, tool, &refs, &scratch, format)?;
        let out = self.run(tool_id, tool, &invocation, &scratch)?;
        if let Some(crash) = spec.crash_stderr.and_then(|find| find(&out.stderr)) {
            return Err(execution(tool_id, format!("cop crashed: {crash}")));
        }
        let keep_input = out.code != Some(0) && !(spec.keep_exit_one && out.code == Some(1));
        if keep_input {
            return cleaned(tool_id, scratch, text.to_owned());
        }
        let fixed = Self::reread_fixed(tool_id, &absolute)?;
        cleaned(tool_id, scratch, fixed)
    }
}
