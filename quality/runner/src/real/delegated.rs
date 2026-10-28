use quality_adapter::parsers::{self, FileFinding};

use super::*;

type SarifParser = fn(&[u8], Option<i32>, &[&str]) -> Result<Vec<FileFinding>, parsers::ParseError>;

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
        self.check_dotnet_sarif(tool_id, tool, pairs, parsers::parse_roslyn)
    }

    pub(super) fn check_csc_delegated(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
    ) -> Result<Vec<FileFinding>, RunnerError> {
        self.check_dotnet_sarif(tool_id, tool, pairs, parsers::parse_csc)
    }

    /// Parses the per-TFM csc SARIF logs of the C# aspect with `parse` and merges them.
    fn check_dotnet_sarif(
        &self,
        tool_id: &str,
        tool: &RealTool,
        pairs: &[(String, PathBuf)],
        parse: SarifParser,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        if tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                format!(
                    "{tool_id} needs the SARIF log of the target compilation; the C# aspect did not supply it"
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
                            "{tool_id} SARIF log {} names no target framework",
                            path.display()
                        ),
                    )
                })?;
            let key = format!("dx-roslyn/{tfm}/exit");
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
                            "{tool_id} needs the csc exit status for {tfm}; the C# aspect did not supply it"
                        ),
                    )
                })?;
            let found = parsed(tool_id, parse(&bytes, Some(code), &workspaces))?;
            pivots.push((tfm.to_owned(), found));
        }
        let mut findings = merge_pivot_findings(pivots);
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
        let workspaces: Vec<&str> = pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        if tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "clang_syntax requires upstream diagnostics from a declared delegated action: direct execution without the authoritative compilation context is rejected"
                    .to_owned(),
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
                        format!("clang_syntax log {} has no name", path.display()),
                    )
                })?;
            let key = format!("dx-clang-syntax/{stem}/exit");
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
                            "clang_syntax needs the compiler exit status for {stem}; the clang_syntax aspect did not supply it"
                        ),
                    )
                })?;
            findings.extend(parsed(
                tool_id,
                parsers::parse_clang_syntax(&bytes, Some(code), &workspaces),
            )?);
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

    pub(super) fn check_govet_delegated(
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
                parsers::parse_govet(&bytes, Some(0), &workspaces),
            )?);
        }
        for found in &mut findings {
            let absolute = reanchor(tool_id, pairs, &found.file)?;
            found.file = absolute.to_string_lossy().into_owned();
        }
        Ok(findings)
    }

    pub(super) fn check_errcheck_delegated(
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
                parsers::parse_errcheck(&bytes, Some(0), &workspaces),
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
