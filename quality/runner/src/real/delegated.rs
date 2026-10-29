use quality_adapter::parsers::{self, FileFinding};

use super::*;

type LogParser = fn(&[u8], Option<i32>, &[&str]) -> Result<Vec<FileFinding>, parsers::ParseError>;

/// Where a .NET compiler log comes from and how to read it.
struct DotnetLog {
    what: &'static str,
    aspect: &'static str,
    compiler: &'static str,
    exit_prefix: &'static str,
    parse: LogParser,
}

impl DotnetLog {
    /// The csc SARIF log of the C# aspect.
    fn csc_sarif(parse: LogParser) -> Self {
        Self {
            what: "SARIF log",
            aspect: "C#",
            compiler: "csc",
            exit_prefix: "dx-roslyn",
            parse,
        }
    }
}

type CompileLogParser =
    fn(&[u8], Option<i32>, &[&str]) -> Result<Vec<FileFinding>, parsers::ParseError>;

impl RealBackend {
    pub(super) fn check_clippy_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_clippy(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_rustc_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_rustc(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_roslyn_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        self.check_dotnet_log(
            tool_id,
            tool,
            pairs,
            &DotnetLog::csc_sarif(parsers::parse_roslyn),
        )
    }

    pub(super) fn check_csc_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        self.check_dotnet_log(
            tool_id,
            tool,
            pairs,
            &DotnetLog::csc_sarif(parsers::parse_csc),
        )
    }

    pub(super) fn check_fsc_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let source = DotnetLog {
            what: "fsc output",
            aspect: "F#",
            compiler: "fsc",
            exit_prefix: "dx-fsc",
            parse: parsers::parse_fsc,
        };
        self.check_dotnet_log(tool_id, tool, pairs, &source)
    }

    /// Parses the per-TFM compiler logs of the C# or F# aspect and merges them.
    fn check_dotnet_log(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
        source: &DotnetLog,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let DotnetLog {
            what,
            aspect,
            compiler,
            exit_prefix,
            parse,
        } = source;
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        if tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                format!(
                    "{tool_id} needs the {what} of the target compilation; the {aspect} aspect did not supply it"
                ),
            ));
        }
        let mut pivots = Vec::with_capacity(tool.upstream_diagnostics.len());
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let tfm = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    execution(
                        tool_id,
                        format!(
                            "{tool_id} {what} {} names no target framework",
                            path.display()
                        ),
                    )
                })?;
            let key = format!("{exit_prefix}/{tfm}/exit");
            let code = tool
                .tool_files
                .iter()
                .find(|(rel, _)| *rel == key)
                .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
                .and_then(|text| text.trim().parse::<i32>().ok())
                .ok_or_else(|| {
                    execution(
                        tool_id,
                        format!(
                            "{tool_id} needs the {compiler} exit status for {tfm}; the {aspect} aspect did not supply it"
                        ),
                    )
                })?;
            let found = parsed(tool_id, (parse)(&bytes, Some(code), &workspaces))?;
            pivots.push((tfm.to_owned(), found));
        }
        let mut findings = merge_pivot_findings(pivots);
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    /// Reads the versioned JSON log of a JVM compiler check (javac today).
    ///
    /// A failed driver fails its Bazel action, so the log's own exit status carries javac's result
    /// and the process exit status is always zero here.
    pub(super) fn check_jvm_log_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let owned: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let name: &'static str = match tool_id {
            "javac" => "javac",
            other => {
                return Err(execution(
                    other,
                    "no JVM check log parser is registered for this tool".to_owned(),
                ));
            }
        };
        let [path] = tool.upstream_diagnostics.as_slice() else {
            return Err(execution(
                tool_id,
                format!(
                    "{tool_id} needs the check log of the target; the JVM aspect did not supply it"
                ),
            ));
        };
        let bytes = std::fs::read(path)
            .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
        let mut findings = parsed(
            tool_id,
            parsers::parse_jvm_diagnostics(name, &bytes, Some(0), &owned),
        )?;
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_go_analysis_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let [path] = tool.upstream_diagnostics.as_slice() else {
            return Err(execution(
                tool_id,
                format!(
                    "{tool_id} needs the report of the target package; the Go aspect did not supply it"
                ),
            ));
        };
        let bytes = std::fs::read(path)
            .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
        let mut findings = parsed(
            tool_id,
            match tool_id {
                "gotype" => parsers::parse_gotype("gotype", &bytes, Some(0), &workspaces),
                "errcheck" => parsers::parse_go_analysis("errcheck", &bytes, Some(0), &workspaces),
                _ => parsers::parse_go_analysis("govet", &bytes, Some(0), &workspaces),
            },
        )?;
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_tsc_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let [path] = tool.upstream_diagnostics.as_slice() else {
            return Err(execution(
                tool_id,
                "tsc needs the diagnostics of the target project; the TypeScript aspect did not supply them"
                    .to_owned(),
            ));
        };
        let code = tool
            .tool_files
            .iter()
            .find(|(rel, _)| rel == "dx-tsc/exit")
            .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
            .and_then(|text| text.trim().parse::<i32>().ok())
            .ok_or_else(|| {
                execution(
                    tool_id,
                    "tsc needs its exit status; the TypeScript aspect did not supply it".to_owned(),
                )
            })?;
        let bytes = std::fs::read(path)
            .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
        let mut findings = parsed(tool_id, parsers::parse_tsc(&bytes, Some(code), &workspaces))?;
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_fsharplint_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_fsharplint(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_qmllint_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
        view: parsers::QmllintView,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_qmllint_report(&bytes, &workspaces, view),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_clang_tidy_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_clang_tidy(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_clang_syntax_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        self.check_compile_replay_delegated(
            tool_id,
            tool,
            pairs,
            "dx-clang-syntax",
            parsers::parse_clang_syntax,
        )
    }

    pub(super) fn check_nvcc_syntax_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        self.check_compile_replay_delegated(
            tool_id,
            tool,
            pairs,
            "dx-nvcc-syntax",
            parsers::parse_nvcc_syntax,
        )
    }

    /// Reads the logs and exit codes of a compile-replay aspect (`<prefix>/<stem>/exit`).
    fn check_compile_replay_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
        prefix: &str,
        parse: LogParser,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        if tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                format!("{tool_id} requires upstream diagnostics from a declared delegated action: direct execution without the authoritative compilation context is rejected"),
            ));
        }
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    execution(
                        tool_id,
                        format!("{tool_id} log {} has no name", path.display()),
                    )
                })?;
            let key = format!("{prefix}/{stem}/exit");
            let code = tool
                .tool_files
                .iter()
                .find(|(rel, _)| *rel == key)
                .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
                .and_then(|text| text.trim().parse::<i32>().ok())
                .ok_or_else(|| {
                    execution(
                        tool_id,
                        format!(
                            "{tool_id} needs the compiler exit status for {stem}; the {tool_id} aspect did not supply it"
                        ),
                    )
                })?;
            findings.extend(parsed(tool_id, parse(&bytes, Some(code), &workspaces))?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_compile_log_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let parse: CompileLogParser = match tool_id {
            "flang" => parsers::parse_flang,
            "ldc" => parsers::parse_ldc,
            other => {
                return Err(execution(
                    other,
                    "no compile-check parser is registered for this tool".to_owned(),
                ));
            }
        };
        if tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                format!(
                    "{tool_id} requires upstream diagnostics from a declared delegated compile replay: direct execution without the authoritative compilation context is rejected"
                ),
            ));
        }
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| {
                    execution(
                        tool_id,
                        format!("{tool_id} log {} has no name", path.display()),
                    )
                })?;
            let key = format!("dx-compile-check/{stem}/exit");
            let code = tool
                .tool_files
                .iter()
                .find(|(rel, _)| *rel == key)
                .and_then(|(_, bytes)| std::str::from_utf8(bytes).ok())
                .and_then(|text| text.trim().parse::<i32>().ok())
                .ok_or_else(|| {
                    execution(
                        tool_id,
                        format!(
                            "{tool_id} needs the compiler exit status for {stem}; the compile-check aspect did not supply it"
                        ),
                    )
                })?;
            findings.extend(parsed(tool_id, parse(&bytes, Some(code), &workspaces))?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_cppcheck_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_cppcheck(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_staticcheck_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_staticcheck(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_file_family_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let mut findings = Vec::new();
        for path in &tool.upstream_diagnostics {
            let bytes = std::fs::read(path)
                .map_err(|err| execution(tool_id, format!("upstream diagnostics: {err}")))?;
            let report = match tool_id {
                "rubocop" => parsers::parse_rubocop(
                    &bytes,
                    &[],
                    Some(0),
                    &workspaces,
                    std::path::Path::new(""),
                ),
                "yamllint" => parsers::parse_yamllint(&bytes, Some(0), &workspaces),
                "shellcheck" => parsers::parse_shellcheck(&bytes, Some(0), &workspaces),
                "checkmake" => parsers::parse_checkmake(&bytes, &[], Some(0), &workspaces, None),
                "hadolint" => parsers::parse_hadolint(&bytes, &[], Some(0), &workspaces),
                "statix" => {
                    let code = i32::from(!String::from_utf8_lossy(&bytes).trim().is_empty());
                    parsers::parse_statix(&bytes, &[], Some(code), &workspaces)
                }
                "deadnix" => parsers::parse_deadnix(&bytes, &[], Some(0), &workspaces),
                "kubeconform" => {
                    let targets: Vec<parsers::KubeconformTarget<'_>> = workspaces
                        .iter()
                        .map(|workspace| parsers::KubeconformTarget {
                            reported: workspace,
                            file: workspace,
                            documents: &[],
                        })
                        .collect();
                    let text = String::from_utf8_lossy(&bytes);
                    let failing =
                        text.contains("\"statusInvalid\"") || text.contains("\"statusError\"");
                    parsers::parse_kubeconform(&bytes, &[], Some(i32::from(failing)), &targets)
                }
                "keep_sorted" => parsers::parse_keep_sorted(&bytes, Some(0), &workspaces),
                "djlint" => parsers::parse_djlint(&bytes, Some(0), &workspaces),
                _ => {
                    return Err(execution(
                        tool_id,
                        format!("unsupported file-family delegated tool: {tool_id}"),
                    ))
                }
            };
            findings.extend(parsed(tool_id, report)?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }
}

/// Merges the findings of one tool run once per pivot (for example one
/// target framework each) into a union that reports each finding once.
///
/// Pivots arrive in their reporting order. A finding that only some pivots
/// report gets the sorted list of those pivots appended to its message, as
/// ` [net8.0]`. A finding every pivot reports keeps its message, so a single
/// pivot reproduces its own findings byte for byte.
pub(super) fn merge_pivot_findings(pivots: Vec<(String, Vec<FileFinding>)>) -> Vec<FileFinding> {
    let total = pivots.len();
    let mut merged: Vec<(FileFinding, Vec<String>)> = Vec::new();
    for (pivot, findings) in pivots {
        for found in findings {
            // A repeat inside one pivot is the tool's own output, so it stays.
            let existing = merged
                .iter_mut()
                .find(|(seen, seen_in)| !seen_in.contains(&pivot) && same_finding(seen, &found));
            match existing {
                Some((_, seen_in)) => seen_in.push(pivot.clone()),
                None => merged.push((found, vec![pivot.clone()])),
            }
        }
    }
    merged
        .into_iter()
        .map(|(mut found, seen_in)| {
            if seen_in.len() < total {
                found.finding.message =
                    format!("{} [{}]", found.finding.message, seen_in.join(", "));
            }
            found
        })
        .collect()
}

fn same_finding(left: &FileFinding, right: &FileFinding) -> bool {
    left.file == right.file
        && left.finding.start == right.finding.start
        && left.finding.rule_id == right.finding.rule_id
        && left.finding.message == right.finding.message
}
