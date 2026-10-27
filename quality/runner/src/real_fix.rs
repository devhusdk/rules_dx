use quality_adapter::commands;
use quality_adapter::exec::MirrorContents;

use crate::StageSpec;
use quality_adapter::launch::ResolvedTool;

use super::*;

/// Empty stand-ins for the other known files with these extensions.
#[cfg(test)]
pub(super) fn placeholders(
    path: &str,
    known: &BTreeSet<String>,
    extensions: &[&str],
) -> BTreeMap<String, String> {
    placeholders_except(&[path.to_owned()], known, extensions)
}

/// Empty stand-ins for the known files outside `staged` with these extensions.
fn placeholders_except(
    staged: &[String],
    known: &BTreeSet<String>,
    extensions: &[&str],
) -> BTreeMap<String, String> {
    known
        .iter()
        .filter(|candidate| !staged.contains(candidate))
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
        config: FixConfig::Optional,
        keep_exit_one: false,
        crash_stderr: None,
    },
    FixSpec {
        tool: "remark",
        config: FixConfig::Optional,
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

/// How one tool fixes files in one capability.
#[derive(Clone, Copy)]
pub(super) enum Route {
    Keep,
    Spec {
        format: bool,
        placeholders: &'static [&'static str],
    },
    Rustfmt,
    Scalafix,
    Rewrite(fn(&Path, &[&Path]) -> Invocation),
}

/// What one fix spawn did to its targets.
pub(super) enum Outcome {
    Fixed,
    Kept,
    Failed(RunnerError),
}

/// The single routing table from a tool and capability to its fixer.
fn fix_route(tool_id: &str, tool: &RealTool, capability: &str) -> Result<Route, RunnerError> {
    if capability == "audit" || capability == "typecheck" {
        return Ok(Route::Keep);
    }
    let spec = |format: bool| -> Result<Route, RunnerError> {
        match fix_spec(tool_id, format) {
            Some(_) => Ok(Route::Spec {
                format,
                placeholders: &[],
            }),
            None => Err(execution(
                tool_id,
                format!("unsupported real tool: {tool_id}"),
            )),
        }
    };
    match tool_id {
        "vale" | "markdown_check" | "rustc" | "ty" | "pydoclint" | "flake8" | "pylint"
        | "clippy" | "roslyn" | "fsharplint" | "checkstyle" | "pmd" | "spotbugs" | "qmllint"
        | "clang_tidy" | "cppcheck" | "staticcheck" | "govet" | "errcheck" | "stylelint"
        | "rubocop" | "psscriptanalyzer" | "yamllint" | "shellcheck" | "hadolint" | "redocly" => {
            Ok(Route::Keep)
        }
        "graphql_eslint" => match tool.config_rel {
            Some(_) => Ok(Route::Keep),
            None => Err(execution(
                tool_id,
                "graphql_eslint requires a config".to_owned(),
            )),
        },
        "buf" | "djlint" | "biome" | "prettier" | "remark" => {
            if capability == "format" {
                spec(true)
            } else {
                Ok(Route::Keep)
            }
        }
        "rustfmt" => Ok(Route::Rustfmt),
        "ruff" => Ok(Route::Spec {
            format: capability == "format",
            placeholders: &["py", "pyi"],
        }),
        "scalafix" => Ok(Route::Scalafix),
        "erlfmt" => Ok(Route::Rewrite(commands::erlfmt_fix)),
        "mix_format" => Ok(Route::Rewrite(commands::mix_format_fix)),
        "gleam_format" => Ok(Route::Rewrite(commands::gleam_format_fix)),
        "keep_sorted" => {
            if tool.upstream_diagnostics.is_empty() {
                spec(false)
            } else {
                Ok(Route::Keep)
            }
        }
        _ => spec(false),
    }
}

impl Route {
    fn placeholder_extensions(self) -> &'static [&'static str] {
        match self {
            Route::Spec { placeholders, .. } => placeholders,
            Route::Rustfmt => &["rs"],
            Route::Keep | Route::Scalafix | Route::Rewrite(_) => &[],
        }
    }
}

struct SessionScratch {
    tool_id: String,
    scratch: Scratch,
    placeholders: BTreeSet<String>,
    live: Vec<String>,
}

impl SessionScratch {
    /// Stages one spawn: its targets at their current text, every other placeholder empty.
    fn stage(
        &mut self,
        current: &BTreeMap<String, String>,
        targets: &[String],
    ) -> Result<(), RunnerError> {
        let mut mirrors = Vec::with_capacity(self.live.len() + targets.len());
        for path in std::mem::take(&mut self.live) {
            if targets.contains(&path) {
                continue;
            }
            if self.placeholders.contains(&path) {
                mirrors.push(MirrorFile {
                    mirror_rel: PathBuf::from(&path),
                    contents: MirrorContents::Bytes(Vec::new()),
                });
                continue;
            }
            let absolute = self.scratch.root().join(&path);
            match std::fs::remove_file(&absolute) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(execution(&self.tool_id, format!("unstage {path}: {err}"))),
            }
        }
        for path in targets {
            let text = current
                .get(path)
                .ok_or(RunnerError::MissingFile { path: path.clone() })?;
            mirrors.push(MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(text.as_bytes().to_vec()),
            });
        }
        self.live = targets.to_vec();
        write_all(&self.scratch, &self.tool_id, &mirrors)
    }
}

/// Fix state for one convergence run: one scratch per stage, reused across rounds.
pub(super) struct FixSession<'a> {
    backend: &'a RealBackend,
    capability: &'a str,
    known: &'a BTreeSet<String>,
    scratches: BTreeMap<usize, SessionScratch>,
}

impl<'a> FixSession<'a> {
    pub(super) fn new(
        backend: &'a RealBackend,
        capability: &'a str,
        known: &'a BTreeSet<String>,
    ) -> Self {
        Self {
            backend,
            capability,
            known,
            scratches: BTreeMap::new(),
        }
    }

    /// Fixes the targets of one stage, batching spawns and isolating files only on failure.
    pub(super) fn fix(
        &mut self,
        index: usize,
        stage: &StageSpec,
        current: &BTreeMap<String, String>,
        targets: &[String],
    ) -> Result<BTreeMap<String, String>, RunnerError> {
        let tool_id = stage.tool_id.as_str();
        let backend = self.backend;
        let tool = backend.tool(tool_id)?;
        let route = fix_route(tool_id, tool, self.capability)?;
        let text_of = |path: &String| {
            current
                .get(path)
                .cloned()
                .ok_or(RunnerError::MissingFile { path: path.clone() })
        };
        if matches!(route, Route::Keep) || targets.is_empty() {
            return targets
                .iter()
                .map(|path| Ok((path.clone(), text_of(path)?)))
                .collect();
        }
        if !self.scratches.contains_key(&index) {
            let created = self.open(tool_id, tool, route, stage)?;
            self.scratches.insert(index, created);
        }
        let Some(session) = self.scratches.get_mut(&index) else {
            return Err(execution(tool_id, "fix scratch missing".to_owned()));
        };
        let mut absolute = Vec::with_capacity(targets.len());
        for path in targets {
            absolute.push(
                session
                    .scratch
                    .resolve(Path::new(path))
                    .map_err(|err| execution(tool_id, format!("scratch file: {err}")))?,
            );
        }
        let names: Vec<String> = stage
            .source_paths
            .iter()
            .chain(&session.placeholders)
            .map(|rel| {
                session
                    .scratch
                    .root()
                    .join(rel)
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        let refs: Vec<&Path> = absolute.iter().map(PathBuf::as_path).collect();
        let batches = commands::plan_invocation_batches(&refs, |batch| {
            backend.fix_invocation(route, tool_id, tool, batch, &session.scratch)
        })?;
        let mut fixed = BTreeMap::new();
        let mut offset = 0;
        for batch in batches {
            let batch_targets = &targets[offset..offset + batch.len()];
            offset += batch.len();
            session.stage(current, batch_targets)?;
            let outcome =
                backend.spawn_fix(route, tool_id, tool, &batch, &session.scratch, &names)?;
            match outcome {
                Outcome::Fixed => {
                    for (path, file) in batch_targets.iter().zip(&batch) {
                        fixed.insert(path.clone(), RealBackend::reread_fixed(tool_id, file)?);
                    }
                }
                Outcome::Kept if batch.len() == 1 => {
                    for path in batch_targets {
                        fixed.insert(path.clone(), text_of(path)?);
                    }
                }
                Outcome::Failed(err) if batch.len() == 1 => return Err(err),
                Outcome::Kept | Outcome::Failed(_) => {
                    for (path, file) in batch_targets.iter().zip(&batch) {
                        session.stage(current, std::slice::from_ref(path))?;
                        let alone = [*file];
                        let text = match backend.spawn_fix(
                            route,
                            tool_id,
                            tool,
                            &alone,
                            &session.scratch,
                            &names,
                        )? {
                            Outcome::Fixed => RealBackend::reread_fixed(tool_id, file)?,
                            Outcome::Kept => text_of(path)?,
                            Outcome::Failed(err) => return Err(err),
                        };
                        fixed.insert(path.clone(), text);
                    }
                }
            }
        }
        Ok(fixed)
    }

    fn open(
        &self,
        tool_id: &str,
        tool: &RealTool,
        route: Route,
        stage: &StageSpec,
    ) -> Result<SessionScratch, RunnerError> {
        let scratch = fresh_scratch(&self.backend.scratch_parent, tool_id)?;
        let mut candidates = self.known.clone();
        candidates.extend(stage.source_paths.iter().cloned());
        let placeholders: BTreeSet<String> =
            placeholders_except(&[], &candidates, route.placeholder_extensions())
                .into_keys()
                .collect();
        let mut mirrors: Vec<MirrorFile> = placeholders
            .iter()
            .map(|path| MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(Vec::new()),
            })
            .collect();
        let reserved: Vec<MirrorFile> = stage
            .source_paths
            .iter()
            .map(|path| MirrorFile {
                mirror_rel: PathBuf::from(path),
                contents: MirrorContents::Bytes(Vec::new()),
            })
            .chain(mirrors.iter().cloned())
            .collect();
        mirrors.extend(RealBackend::mirror_tool_files(tool_id, tool, &reserved));
        write_all(&scratch, tool_id, &mirrors)?;
        Ok(SessionScratch {
            tool_id: tool_id.to_owned(),
            scratch,
            placeholders,
            live: Vec::new(),
        })
    }

    /// Removes every stage scratch and reports the first cleanup failure.
    pub(super) fn close(self) -> Result<(), RunnerError> {
        let mut first = Ok(());
        for (_, session) in self.scratches {
            let closed = cleaned(&session.tool_id, session.scratch, ());
            if first.is_ok() {
                first = closed;
            }
        }
        first
    }
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
        let stage = StageSpec {
            tool_id: tool_id.to_owned(),
            class_ids: Vec::new(),
            source_paths: vec![path.to_owned()],
        };
        let current = BTreeMap::from([(path.to_owned(), text.to_owned())]);
        let mut session = FixSession::new(self, capability, known);
        let mut fixed = session.fix(0, &stage, &current, &stage.source_paths)?;
        session.close()?;
        fixed.remove(path).ok_or(RunnerError::MissingFile {
            path: path.to_owned(),
        })
    }

    pub(super) fn reread_fixed(tool_id: &str, absolute: &Path) -> Result<String, RunnerError> {
        let fixed = std::fs::read(absolute)
            .map_err(|err| execution(tool_id, format!("re-read fixed file: {err}")))?;
        String::from_utf8(fixed).map_err(|err| RunnerError::ToolOutput {
            tool_id: tool_id.to_owned(),
            detail: format!("fixed file is not UTF-8: {err}"),
        })
    }

    fn fix_invocation(
        &self,
        route: Route,
        tool_id: &str,
        tool: &RealTool,
        refs: &[&Path],
        scratch: &Scratch,
    ) -> Result<Invocation, RunnerError> {
        match route {
            Route::Keep => Err(execution(tool_id, format!("{tool_id} has no fixer"))),
            Route::Spec { format, .. } => {
                let Some(spec) = fix_spec(tool_id, format) else {
                    return Err(execution(
                        tool_id,
                        format!("unsupported real tool: {tool_id}"),
                    ));
                };
                self.build_fix_invocation(spec, tool_id, tool, refs, scratch, format)
            }
            Route::Rustfmt => self.rustfmt_fix_invocation(tool, refs, scratch),
            Route::Scalafix => {
                let Some(config) = self.config_abs(tool_id, tool, scratch)? else {
                    return Err(execution(tool_id, "scalafix requires a config".to_owned()));
                };
                let classpath = Self::scalafix_classpath(tool_id, tool, scratch)?;
                let jars: Vec<&Path> = classpath.iter().map(PathBuf::as_path).collect();
                let context = commands::ScalafixContext {
                    config: &config,
                    sourceroot: scratch.root(),
                    classpath: &jars,
                };
                commands::scalafix_fix(&tool.binary, refs, &context)
                    .map_err(|err| execution(tool_id, err.to_string()))
            }
            Route::Rewrite(fix) => Ok(fix(&tool.binary, refs)),
        }
    }

    /// Runs one fix spawn and judges it by the per-file rule of its route.
    fn spawn_fix(
        &self,
        route: Route,
        tool_id: &str,
        tool: &RealTool,
        refs: &[&Path],
        scratch: &Scratch,
        names: &[String],
    ) -> Result<Outcome, RunnerError> {
        let invocation = self.fix_invocation(route, tool_id, tool, refs, scratch)?;
        let out = self.run(tool_id, tool, &invocation, scratch)?;
        Ok(match route {
            Route::Keep => Outcome::Kept,
            Route::Spec { format, .. } => {
                let crash = fix_spec(tool_id, format)
                    .and_then(|spec| spec.crash_stderr)
                    .and_then(|find| find(&out.stderr));
                let keep_exit_one =
                    fix_spec(tool_id, format).is_some_and(|spec| spec.keep_exit_one);
                if let Some(crash) = crash {
                    Outcome::Failed(execution(tool_id, format!("cop crashed: {crash}")))
                } else if out.code != Some(0) && !(keep_exit_one && out.code == Some(1)) {
                    Outcome::Kept
                } else {
                    Outcome::Fixed
                }
            }
            Route::Rustfmt => super::rustfmt::fix_outcome(scratch.root(), names, &out),
            Route::Scalafix => {
                if matches!(out.code, Some(0) | Some(64)) {
                    Outcome::Fixed
                } else {
                    Outcome::Kept
                }
            }
            Route::Rewrite(_) => {
                if out.code == Some(0) && out.stderr.is_empty() {
                    Outcome::Fixed
                } else {
                    Outcome::Kept
                }
            }
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
                    "prettier" => {
                        let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                        let ignore = Self::prettier_ignore_abs(scratch)?;
                        Ok(
                            commands::prettier_fix(&launch, refs, config.as_deref(), &ignore)
                                .map_err(|err| execution(tool_id, err.to_string()))?,
                        )
                    }
                    "remark" => {
                        let launch = ResolvedTool::javascript(tool_id, &tool.binary);
                        Ok(commands::remark_fix(&launch, refs, config.as_deref())
                            .map_err(|err| execution(tool_id, err.to_string()))?)
                    }
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
                "fantomas" => Ok(commands::fantomas_fix(&tool.binary, refs)),
                "buf" => Ok(commands::buf_format_fix(&tool.binary, refs, scratch.root())),
                "gofumpt" => Ok(commands::gofumpt_fix(
                    &tool.binary,
                    refs,
                    Self::gofumpt_lang(tool)?,
                )),
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
}
