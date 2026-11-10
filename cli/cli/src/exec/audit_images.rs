//! The images section of `dx security`: every `oci_deploy` image in scope is
//! built and scanned with the pinned trivy, one row per image.

use super::super::pinned_tool::{PinnedTool, PinnedToolError, QueryBazel, resolve_pinned};
use super::audit_accept::VulnCandidate;
use dx_audit::exception::{RiskException, SecurityPolicy};
use dx_audit::image_scan::{
    IMAGE_SET, ImageFinding, covers, parse_report, severity_list, trivy_args,
};
use dx_error::DxError;
use dx_output::{DiagnosticEvent, Severity, Threshold};
use dx_tool_resolve::Bazel;
use std::path::Path;

const TRIVY: PinnedTool<'static> = PinnedTool {
    var: "DX_TRIVY",
    label: "@rules_dx//quality/artifacts:trivy",
};

/// The private rule `oci_deploy` wraps; its targets carry the image output group.
const CONFIG_KIND: &str = "_oci_config";
const IMAGE_GROUP: &str = "--output_groups=dx_oci_image";

pub(super) struct ImagesInputs<'a> {
    pub(super) workspace: &'a Path,
    pub(super) runner: &'a dyn dx_process::Runner,
    pub(super) bazel: &'a QueryBazel<'a>,
    pub(super) temp_dir: &'a Path,
    pub(super) pid: u32,
    pub(super) nonce: u64,
    pub(super) scopes: &'a [String],
    pub(super) excludes: &'a [String],
    pub(super) policy: &'a SecurityPolicy,
    pub(super) exceptions: &'a [&'a RiskException],
    pub(super) fail_on: Threshold,
    pub(super) today: &'a str,
    pub(super) offline: bool,
}

#[derive(Default)]
pub(super) struct ImagesResult {
    pub(super) diagnostics: Vec<DiagnosticEvent>,
    pub(super) rows: Vec<String>,
    pub(super) failing: usize,
    pub(super) incomplete: Option<String>,
    pub(super) candidates: Vec<VulnCandidate>,
}

pub(super) fn run_images(inputs: &ImagesInputs<'_>) -> ImagesResult {
    let mut result = ImagesResult::default();
    let configs = match image_configs(inputs) {
        Ok(configs) => configs,
        Err(error) => {
            result.incomplete = Some(error.message().to_owned());
            return result;
        }
    };
    if configs.is_empty() {
        return result;
    }
    if let Err(error) = build_images(inputs, &configs) {
        result.incomplete = Some(error.message().to_owned());
        return result;
    }
    let trivy = match resolve_pinned(inputs.runner, inputs.bazel, &TRIVY, inputs.offline) {
        Ok(path) => path,
        Err(PinnedToolError::Invalid) => {
            result.incomplete = Some(format!(
                "image scanner unavailable: {} must be an absolute path to the pinned {}",
                TRIVY.var, TRIVY.label
            ));
            return result;
        }
        Err(PinnedToolError::Unresolved { detail }) => {
            result.incomplete = Some(format!(
                "image scanner unavailable: could not resolve {} through Bazel ({detail})",
                TRIVY.label
            ));
            return result;
        }
    };
    for config in &configs {
        let label = config.strip_suffix("_config").unwrap_or(config);
        match scan_image(inputs, &trivy, config) {
            Ok(findings) => report_image(inputs, label, findings, &mut result),
            Err(error) => {
                if result.incomplete.is_none() {
                    result.incomplete = Some(format!("failed to scan image {label}: {error}"));
                }
            }
        }
    }
    result
}

fn image_configs(inputs: &ImagesInputs<'_>) -> Result<Vec<String>, DxError> {
    let scopes: Vec<&str> = inputs
        .scopes
        .iter()
        .map(String::as_str)
        .filter(|scope| dx_config::conventions::is_valid_pattern(scope))
        .collect();
    if scopes.is_empty() {
        return Ok(Vec::new());
    }
    let expression = format!("kind(\"{CONFIG_KIND}\", {})", scopes.join(" + "));
    let labels = dx_plan::resolve::run_label_query_file(
        &expression,
        &expression,
        inputs.workspace,
        inputs.bazel.query_runner,
        inputs.bazel.startup,
    )
    .map_err(|error| DxError::failure(format!("failed to find images: {error}")))?;
    Ok(labels
        .into_iter()
        .filter(|label| !dx_plan::resolve::exclude::covered(inputs.excludes, label))
        .collect())
}

fn build_images(inputs: &ImagesInputs<'_>, configs: &[String]) -> Result<(), DxError> {
    let mut required = vec![IMAGE_GROUP.to_owned()];
    if inputs.offline {
        required.push("--repository_disable_download".to_owned());
    }
    let argv =
        dx_plan::plan::bazel_argv("build", &[], &required, &[], configs, inputs.bazel.startup)
            .map_err(|error| {
                DxError::failure(format!("failed to plan the image build: {error}"))
            })?;
    let status = inputs
        .runner
        .run(&argv, inputs.workspace, &[])
        .map_err(|error| DxError::infra(format!("failed to build images: {error}")))?;
    match status.code {
        Some(0) => Ok(()),
        code => Err(DxError::failure(format!(
            "failed to build images: bazel build exited with {}",
            code.map_or_else(|| "a signal".to_owned(), |code| code.to_string())
        ))),
    }
}

fn scan_image(
    inputs: &ImagesInputs<'_>,
    trivy: &Path,
    config: &str,
) -> Result<Vec<ImageFinding>, DxError> {
    let files = inputs
        .bazel
        .cquery_files(config, &[IMAGE_GROUP.to_owned()])
        .map_err(|error| DxError::infra(error.to_string()))?;
    let image = files
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| DxError::failure("the image output group is empty"))?;
    let root = inputs
        .bazel
        .execution_root()
        .map_err(|error| DxError::infra(error.to_string()))?;
    let image = Path::new(&root).join(image);
    let cache =
        dx_audit::image_scan::trivy_cache(Some(inputs.workspace), inputs.today, inputs.offline)?;
    let report = inputs.temp_dir.join(format!(
        "audit-trivy-{}-{}-{}.json",
        inputs.pid,
        inputs.nonce,
        config.replace(['/', ':', '@'], "_")
    ));
    let severities = severity_list("LOW")?;
    let mut argv = vec![trivy.to_string_lossy().into_owned()];
    argv.extend(
        trivy_args(&image, &severities, cache.as_deref(), Some(&report))
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned()),
    );
    let status = inputs
        .runner
        .run_hermetic(&argv, inputs.workspace, &[])
        .map_err(|error| DxError::infra(format!("cannot run trivy: {error}")))?;
    let text = std::fs::read_to_string(&report);
    let _ = std::fs::remove_file(&report);
    if status.code != Some(0) {
        return Err(DxError::failure(format!(
            "trivy exited with {:?}",
            status.code
        )));
    }
    parse_report(&text.map_err(|error| DxError::from_io(format!("trivy report: {error}"), &error))?)
}

fn image_severity(trivy: &str, policy: &SecurityPolicy) -> Severity {
    if !policy.fails_on(dx_audit::vuln::AdvisoryKind::Vulnerability) {
        return Severity::Info;
    }
    match trivy {
        "CRITICAL" | "HIGH" => Severity::Error,
        "MEDIUM" => Severity::Warning,
        _ => Severity::Info,
    }
}

fn report_image(
    inputs: &ImagesInputs<'_>,
    label: &str,
    findings: Vec<ImageFinding>,
    result: &mut ImagesResult,
) {
    let mut counts = [0_usize; 4];
    for found in findings {
        result.candidates.push(VulnCandidate {
            advisory: found.id.clone(),
            package: found.package.clone(),
            set: label.to_owned(),
            lock_set: IMAGE_SET.to_owned(),
            version: found.installed.clone(),
        });
        if inputs
            .exceptions
            .iter()
            .any(|exception| covers(exception, Some(label), &found))
        {
            continue;
        }
        if let Some(at) = ["CRITICAL", "HIGH", "MEDIUM", "LOW"]
            .iter()
            .position(|name| *name == found.severity)
        {
            counts[at] += 1;
        }
        let severity = image_severity(&found.severity, inputs.policy);
        if dx_output::meets_threshold(severity, inputs.fail_on)
            && inputs
                .policy
                .fails_on(dx_audit::vuln::AdvisoryKind::Vulnerability)
        {
            result.failing += 1;
        }
        let fixed: Vec<String> = found
            .fixed
            .split(',')
            .map(str::trim)
            .filter(|version| !version.is_empty())
            .map(str::to_owned)
            .collect();
        let fix_text = if fixed.is_empty() {
            "no fixed version available".to_owned()
        } else {
            format!("fixed in {}", fixed.join(", "))
        };
        result.diagnostics.push(DiagnosticEvent {
            rule: Some(format!("trivy/{}", found.id)),
            package: super::package_ref(&found.package, &found.installed, Some(fixed), None),
            ..DiagnosticEvent::new(
                severity,
                "trivy",
                format!(
                    "{}@{} in image {label} vulnerable to {} (severity {}; {fix_text})",
                    found.package, found.installed, found.id, found.severity
                ),
            )
        });
    }
    let [critical, high, medium, low] = counts;
    result.rows.push(format!(
        "image {label}: {critical} critical, {high} high, {medium} medium, {low} low"
    ));
}
