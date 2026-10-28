use std::path::{Component, Path, PathBuf};

use quality_adapter::kubernetes_preflight::{self, Document};
use quality_adapter::parsers::FileFinding;
use quality_adapter::{commands, parsers, TextPosition};

use super::kubeconform::preflight_finding;
use super::*;

const CONTEXT_REL: &str = "dx-helm/context.json";
const STATE_REL: &str = "dx-helm";

struct HelmContext {
    chart_root: String,
    values: Vec<String>,
    release: String,
    namespace: String,
}

fn clean_rel(value: &str, allow_empty: bool) -> bool {
    if value.is_empty() {
        return allow_empty;
    }
    !value.contains('\\')
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn moved_finding(mut found: FileFinding, rendered: &str) -> FileFinding {
    found.finding.message = format!("rendered {rendered}: {}", found.finding.message);
    found.finding.start = TextPosition { line: 1, column: 1 };
    found.finding.end = None;
    found
}

fn reset_position(mut found: FileFinding) -> FileFinding {
    found.finding.start = TextPosition { line: 1, column: 1 };
    found.finding.end = None;
    found
}

fn join_rel(root: &str, rel: &str) -> String {
    if root.is_empty() {
        rel.to_owned()
    } else {
        format!("{root}/{rel}")
    }
}

impl RealBackend {
    /// Lints the chart, renders it offline, and validates the rendered documents with kubeconform.
    pub(super) fn check_helm(
        &self,
        tool_id: &str,
        tool: &RealTool,
        staged: &StagedScratch,
        config: Option<&Path>,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let scratch = &staged.scratch;
        if !tool.upstream_diagnostics.is_empty() {
            return Err(execution(
                tool_id,
                "helm has no delegated mode; upstream diagnostics are not accepted".to_owned(),
            ));
        }
        let context = helm_context(tool_id, tool)?;
        let Some(config_path) = config else {
            return Err(execution(
                tool_id,
                "helm needs a kubernetes_schemas config".to_owned(),
            ));
        };
        let config_bytes = std::fs::read(config_path)
            .map_err(|err| execution(tool_id, format!("read config: {err}")))?;
        let kube = commands::kubeconform_config(config_path, &config_bytes)
            .map_err(|err| execution(tool_id, err))?;
        let kubeconform_binary = tool
            .path_tools
            .iter()
            .find(|(name, _)| name == "kubeconform")
            .map(|(_, binary)| binary.clone())
            .ok_or_else(|| {
                execution(
                    tool_id,
                    "helm needs the kubeconform companion binary; the aspect did not supply it"
                        .to_owned(),
                )
            })?;
        let chart_abs = if context.chart_root.is_empty() {
            scratch.root().to_path_buf()
        } else {
            scratch
                .resolve(Path::new(&context.chart_root))
                .map_err(|err| execution(tool_id, format!("chart root: {err}")))?
        };
        let mut value_paths: Vec<PathBuf> = Vec::new();
        for value in &context.values {
            value_paths.push(
                scratch
                    .resolve(Path::new(value))
                    .map_err(|err| execution(tool_id, format!("values file: {err}")))?,
            );
        }
        let value_refs: Vec<&Path> = value_paths.iter().map(PathBuf::as_path).collect();
        let state = |name: &str| -> Result<String, RunnerError> {
            let dir = scratch
                .resolve(Path::new(&format!("{STATE_REL}/{name}")))
                .map_err(|err| execution(tool_id, format!("helm state: {err}")))?;
            Ok(dir.to_string_lossy().into_owned())
        };
        let state_dir = scratch
            .resolve(Path::new(STATE_REL))
            .map_err(|err| execution(tool_id, format!("helm state: {err}")))?;
        std::fs::create_dir_all(&state_dir)
            .map_err(|err| execution(tool_id, format!("helm state: {err}")))?;
        let mut helm = RealTool {
            binary: tool.binary.clone(),
            extra_env: tool.extra_env.clone(),
            config_rel: None,
            edition: None,
            dialect: None,
            tool_files: Vec::new(),
            upstream_diagnostics: Vec::new(),
            search_paths: tool.search_paths.clone(),
            path_tools: Vec::new(),
        };
        for (key, name) in [
            ("HELM_CACHE_HOME", "cache"),
            ("HELM_CONFIG_HOME", "config"),
            ("HELM_DATA_HOME", "data"),
            ("HELM_REPOSITORY_CONFIG", "repositories.yaml"),
            ("HELM_REPOSITORY_CACHE", "repository"),
            ("HELM_REGISTRY_CONFIG", "registry.json"),
            ("HELM_PLUGINS", "plugins"),
            ("KUBECONFIG", "kubeconfig"),
        ] {
            helm.extra_env.push((key.to_owned(), state(name)?));
        }
        helm.extra_env
            .push(("HELM_NO_PLUGINS".to_owned(), "1".to_owned()));

        let names: Vec<&str> = staged
            .pairs
            .iter()
            .map(|(workspace, _)| workspace.as_str())
            .collect();
        let absolute_of = |workspace: &str| {
            staged
                .pairs
                .iter()
                .find(|(name, _)| name == workspace)
                .map(|(_, absolute)| absolute.clone())
        };

        let lint = commands::helm_lint(
            &helm.binary,
            &chart_abs,
            &value_refs,
            &kube.kubernetes_version,
        );
        let out = self.run(tool_id, &helm, &lint, scratch)?;
        let mut findings = parsed(
            tool_id,
            parsers::parse_helm_lint(&out.stdout, out.code, &context.chart_root, &names),
        )?;

        let template = commands::helm_template(
            &helm.binary,
            &context.release,
            &context.namespace,
            &chart_abs,
            &value_refs,
            &kube.kubernetes_version,
        );
        let out = self.run(tool_id, &helm, &template, scratch)?;
        if out.code != Some(0) {
            findings.extend(parsed(
                tool_id,
                parsers::parse_helm_template_error(&out.stderr, &context.chart_root, &names),
            )?);
            return self.finish_helm(tool_id, staged, findings, &context);
        }
        let rendered = parsed(tool_id, parsers::split_rendered(&out.stdout))?;
        let chart_yaml = join_rel(&context.chart_root, "Chart.yaml");
        let mut owned: Vec<(String, String, Vec<Document>)> = Vec::new();
        #[allow(clippy::type_complexity)]
        let mut unowned: Vec<(String, (String, String, Vec<Document>))> = Vec::new();
        for file in &rendered {
            let workspace = join_rel(&context.chart_root, &file.path);
            let known = absolute_of(&workspace);
            let owner = known.is_some();
            let (workspace, absolute) = match known {
                Some(absolute) => (workspace, absolute),
                None => {
                    let absolute = absolute_of(&chart_yaml).ok_or_else(|| {
                        execution(
                            tool_id,
                            format!("{chart_yaml} is not among the staged files"),
                        )
                    })?;
                    (chart_yaml.clone(), absolute)
                }
            };
            let checked = kubernetes_preflight::check_file(&file.path, file.text.as_bytes());
            let entry = (
                workspace,
                absolute.to_string_lossy().into_owned(),
                checked.documents,
            );
            if owner {
                findings.extend(
                    checked
                        .problems
                        .into_iter()
                        .map(|problem| reset_position(preflight_finding(&absolute, problem))),
                );
                owned.push(entry);
            } else {
                for problem in checked.problems {
                    findings.push(moved_finding(
                        preflight_finding(&absolute, problem),
                        &file.path,
                    ));
                }
                unowned.push((file.path.clone(), entry));
            }
        }
        let kubeconform = RealTool {
            binary: kubeconform_binary,
            extra_env: Vec::new(),
            config_rel: None,
            edition: None,
            dialect: None,
            tool_files: Vec::new(),
            upstream_diagnostics: Vec::new(),
            search_paths: Vec::new(),
            path_tools: Vec::new(),
        };
        findings.extend(
            self.validate_documents("kubeconform", &kubeconform, scratch, config_path, &owned)?
                .into_iter()
                .map(reset_position),
        );
        for (path, entry) in unowned {
            let found = self.validate_documents(
                "kubeconform",
                &kubeconform,
                scratch,
                config_path,
                std::slice::from_ref(&entry),
            )?;
            findings.extend(found.into_iter().map(|found| moved_finding(found, &path)));
        }
        self.finish_helm(tool_id, staged, findings, &context)
    }

    fn finish_helm(
        &self,
        tool_id: &str,
        staged: &StagedScratch,
        mut findings: Vec<FileFinding>,
        context: &HelmContext,
    ) -> Result<Vec<FileFinding>, RunnerError> {
        let chart_yaml = join_rel(&context.chart_root, "Chart.yaml");
        let fallback = staged
            .pairs
            .iter()
            .find(|(name, _)| *name == chart_yaml)
            .map(|(_, absolute)| absolute.to_string_lossy().into_owned())
            .ok_or_else(|| {
                execution(
                    tool_id,
                    format!("{chart_yaml} is not among the staged files"),
                )
            })?;
        for found in &mut findings {
            "helm".clone_into(&mut found.finding.tool_id);
            if let Some((_, absolute)) = staged.pairs.iter().find(|(name, _)| *name == found.file) {
                found.file = absolute.to_string_lossy().into_owned();
            } else if !staged
                .pairs
                .iter()
                .any(|(_, absolute)| absolute.to_string_lossy() == found.file.as_str())
            {
                found.finding.message = format!("{} ({})", found.finding.message, found.file);
                found.file.clone_from(&fallback);
                found.finding.start = TextPosition { line: 1, column: 1 };
            }
        }
        Ok(findings)
    }
}

fn helm_context(tool_id: &str, tool: &RealTool) -> Result<HelmContext, RunnerError> {
    let bytes = tool
        .tool_files
        .iter()
        .find(|(rel, _)| rel == CONTEXT_REL)
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| {
            execution(
                tool_id,
                "helm needs the chart context; the aspect did not supply it".to_owned(),
            )
        })?;
    let doc: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|err| execution(tool_id, format!("helm context: {err}")))?;
    if doc
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        return Err(execution(
            tool_id,
            "helm context schema_version must be 1".to_owned(),
        ));
    }
    let text = |key: &str| -> Result<String, RunnerError> {
        doc.get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| execution(tool_id, format!("helm context: missing {key}")))
    };
    let chart_root = text("chart_root")?;
    if !clean_rel(&chart_root, true) {
        return Err(execution(
            tool_id,
            format!("helm context: chart_root {chart_root:?} must stay inside the workspace"),
        ));
    }
    let values = doc
        .get("values")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| execution(tool_id, "helm context: missing values".to_owned()))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|path| clean_rel(path, false))
                .map(str::to_owned)
                .ok_or_else(|| execution(tool_id, "helm context: bad values path".to_owned()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let release = text("release")?;
    let namespace = text("namespace")?;
    for (label, value) in [("release", &release), ("namespace", &namespace)] {
        if value.is_empty() || value.starts_with('-') || value.chars().any(char::is_whitespace) {
            return Err(execution(
                tool_id,
                format!("helm context: bad {label} {value:?}"),
            ));
        }
    }
    Ok(HelmContext {
        chart_root,
        values,
        release,
        namespace,
    })
}
