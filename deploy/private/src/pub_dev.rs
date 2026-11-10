//! pub.dev publisher: package staging, pubspec checks and the dart or flutter publish.

use crate::api_compat::{
    ApiCompatMode, Baseline, Outcome, changes_or_failure, conclude, export_git_dir, fetch_with,
    highest_below, previous_git_tag, workspace_dir,
};
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty,
    exit_text, placeholder_refusal, resolve_version, run_publish, scrubbed_command,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Maps package input files to package-relative paths under the pubspec.yaml directory.
pub fn pub_package_paths(
    package_files: &[std::path::PathBuf],
) -> io::Result<Vec<(std::path::PathBuf, String)>> {
    package_relative_paths(package_files, "pubspec.yaml", "pub package")
}

/// Owner approval gate for pub_deploy.
pub(crate) const PUB_GATE: OwnerGate = OwnerGate {
    tool: "pub_deploy",
    action: "publish",
    credentials: &["PUB_API_TOKEN"],
    approved: "PUB_PUBLISH_APPROVED",
};

/// Extra env names dart and flutter keep beyond the publish base list.
const PUB_CHILD_ENV_EXTRA: &[&str] = &["PUB_CACHE", "PUB_HOSTED_URL", "FLUTTER_ROOT"];

/// Most tool output lines shown when a step fails.
const PUB_FAILURE_LINES: usize = 40;

/// Returns the unquoted value of a top-level `key:` line, None when absent or not a plain scalar.
fn pubspec_field(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(key).and_then(|r| r.strip_prefix(':')) else {
            continue;
        };
        let value = rest.split(" #").next().unwrap_or("").trim();
        let unquoted = ['"', '\'']
            .iter()
            .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
            .unwrap_or(value);
        return (!unquoted.is_empty()).then(|| unquoted.to_owned());
    }
    None
}

/// True when the pubspec has a top-level `flutter:` section, so `flutter pub` must publish it.
fn pubspec_is_flutter(text: &str) -> bool {
    text.lines().any(|line| line.starts_with("flutter:"))
}

/// Returns a refusal when the pubspec is not the target package.
fn pubspec_refusal(text: &str, package_name: &str) -> Option<String> {
    let name = pubspec_field(text, "name");
    if name.as_deref() == Some(package_name) {
        return None;
    }
    Some(format!(
        "pub_deploy: pubspec.yaml is {}, target is {package_name}; refusing",
        name.as_deref().unwrap_or("(missing)")
    ))
}

/// Returns a refusal when CHANGELOG.md has no heading for the version, which pub.dev warns about.
fn changelog_refusal(changelog: Option<&str>, version: &str) -> Option<String> {
    let Some(text) = changelog else {
        return Some("pub_deploy: package has no CHANGELOG.md; pub.dev wants one".to_owned());
    };
    let has_heading = text.lines().any(|line| {
        line.starts_with('#')
            && line
                .trim_start_matches('#')
                .split(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | '(' | ')'))
                .any(|word| word.trim_start_matches('v') == version)
    });
    (!has_heading).then(|| format!("pub_deploy: CHANGELOG.md has no heading for version {version}"))
}

fn pub_stage(mapped: &[(std::path::PathBuf, String)], staging: &Path) -> Result<(), String> {
    for (src, rel) in mapped {
        let dest = staging.join(rel);
        let staged = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| copy_verified(src, &dest).map(|_| ()));
        if staged.is_err() {
            return Err(format!("cannot stage {}", src.display()));
        }
    }
    Ok(())
}

/// Stages the package, checks pubspec.yaml and CHANGELOG.md against the target and returns
/// whether it is a flutter package with the resolved version.
fn pub_checked_stage(
    mapped: &[(std::path::PathBuf, String)],
    staging: &Path,
    package_name: &str,
    version: &str,
) -> Result<(bool, String), String> {
    pub_stage(mapped, staging).map_err(|error| format!("pub_deploy: {error}"))?;
    let pubspec = std::fs::read_to_string(staging.join("pubspec.yaml"))
        .map_err(|error| format!("pub_deploy: cannot read pubspec.yaml: {error}"))?;
    if let Some(error) = pubspec_refusal(&pubspec, package_name) {
        return Err(error);
    }
    let version = resolve_version(
        "pub_deploy",
        "pubspec.yaml",
        pubspec_field(&pubspec, "version").as_deref(),
        version,
    )?;
    let changelog = std::fs::read_to_string(staging.join("CHANGELOG.md")).ok();
    if let Some(error) = changelog_refusal(changelog.as_deref(), &version) {
        return Err(error);
    }
    Ok((pubspec_is_flutter(&pubspec), version))
}

fn tool_name(flutter: bool) -> &'static str {
    if flutter { "flutter" } else { "dart" }
}

fn failure_message(
    step: &str,
    status: &str,
    out: &[u8],
    token: &str,
) -> String {
    let text = crate::run::pages_redact(&String::from_utf8_lossy(out), token);
    let lines: Vec<&str> = text.lines().collect();
    let mut message = format!("pub_deploy: {step} failed with {status}");
    for line in &lines[lines.len().saturating_sub(PUB_FAILURE_LINES)..] {
        message.push_str("\npub: ");
        message.push_str(line);
    }
    message
}

/// Runs `<tool> pub publish --dry-run` in the staged package.
fn pub_dry_run(program: &Path, staging: &Path) -> Result<(), String> {
    let mut cmd = scrubbed_command(program, &[PUBLISH_CHILD_ENV_KEEP, PUB_CHILD_ENV_EXTRA], &[]);
    cmd.current_dir(staging)
        .args(["pub", "publish", "--dry-run"]);
    let output = run_publish(&mut cmd).map_err(|error| format!("pub_deploy: {error}"))?;
    if output.success() {
        return Ok(());
    }
    let status = exit_text(&output);
    let mut both = output.stdout;
    both.extend_from_slice(&output.stderr);
    Err(failure_message(
        "pub publish --dry-run",
        &status,
        &both,
        "",
    ))
}

/// Publishes the staged package with a token held only in a temporary pub cache.
fn pub_publish(
    program: &Path,
    staging: &Path,
    cache: &Path,
    registry_url: &str,
    token: &str,
) -> Result<(), String> {
    let command = || {
        let mut cmd = scrubbed_command(
            program,
            &[PUBLISH_CHILD_ENV_KEEP, &["FLUTTER_ROOT"]],
            &[
                ("PUB_CACHE", cache.to_str().unwrap_or("")),
                ("PUB_TOKEN", token),
            ],
        );
        cmd.current_dir(staging);
        cmd
    };
    let mut add = command();
    add.args(["pub", "token", "add", registry_url, "--env-var", "PUB_TOKEN"]);
    let added = run_publish(&mut add).map_err(|error| format!("pub_deploy: {error}"))?;
    if !added.success() {
        let status = exit_text(&added);
        let mut both = added.stdout;
        both.extend_from_slice(&added.stderr);
        return Err(failure_message("pub token add", &status, &both, token));
    }
    let mut publish = command();
    publish.args(["pub", "publish", "--force"]);
    let output = run_publish(&mut publish).map_err(|error| format!("pub_deploy: {error}"))?;
    let status = exit_text(&output);
    let success = output.success();
    let mut both = output.stdout;
    both.extend_from_slice(&output.stderr);
    if success {
        return Ok(());
    }
    Err(failure_message("pub publish", &status, &both, token))
}

/// Copies the package files into `<name>-package` under the output directory.
fn pub_build_package(
    mapped: &[(std::path::PathBuf, String)],
    outdir: &Path,
    package_name: &str,
) -> Result<std::path::PathBuf, String> {
    let house = outdir.join(package_name.to_owned() + "-package");
    std::fs::create_dir_all(&house).map_err(|error| format!("pub_deploy: {error}"))?;
    pub_stage(mapped, &house).map_err(|error| format!("pub_deploy: {error}"))?;
    Ok(house)
}

/// Public API check settings of one pub deployment.
pub struct PubApiCheck<'a> {
    pub mode: &'a str,
    pub baseline: &'a str,
    pub dart_rloc: &'a str,
    pub apitool_rloc: &'a str,
    pub flutter_dart_rloc: &'a str,
    pub flutter_root_rloc: &'a str,
    pub package_dir: &'a str,
}

/// Returns one line per breaking change in a dart_apitool JSON report.
fn apitool_changes(report: &str) -> Option<Vec<String>> {
    fn walk(node: &serde_json::Value, path: &mut Vec<String>, out: &mut Vec<String>) {
        for child in node["children"].as_array().into_iter().flatten() {
            if let Some(description) = child["changeDescription"].as_str() {
                let code = child["changeCode"].as_str().unwrap_or("?");
                let at = if path.is_empty() {
                    String::new()
                } else {
                    format!("{}: ", path.join(" > "))
                };
                out.push(format!("{at}{description} ({code})"));
            } else {
                path.push(child["label"].as_str().unwrap_or("?").to_owned());
                walk(child, path, out);
                path.pop();
            }
        }
    }
    let json: serde_json::Value = serde_json::from_str(report).ok()?;
    let mut out = Vec::new();
    walk(
        &json["report"]["breakingChanges"],
        &mut Vec::new(),
        &mut out,
    );
    Some(out)
}

/// Returns the highest non-retracted version below `next` in a pub `api/packages` body.
fn pub_previous_version(body: &[u8], next: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_slice(body).ok()?;
    let versions = json["versions"]
        .as_array()?
        .iter()
        .filter(|entry| !entry["retracted"].as_bool().unwrap_or(false));
    highest_below(versions.filter_map(|entry| entry["version"].as_str()), next)
}

/// Compares the staged package with the earlier version `api_baseline` picks.
fn pub_api_check(
    agent: &ureq::Agent,
    (dart, apitool): (&Path, &Path),
    flutter: Option<(&Path, &Path)>,
    (package_name, version): (&str, &str),
    (staging, registry_url): (&Path, &str),
    (baseline, package_dir): (&str, &str),
    workspace: &Path,
) -> Result<Outcome, String> {
    let baseline = Baseline::parse(baseline)
        .ok_or_else(|| format!("pub_deploy: invalid api_baseline '{baseline}'"))?;
    let pubspec = std::fs::read_to_string(staging.join("pubspec.yaml")).unwrap_or_default();
    let needs_flutter =
        pubspec_is_flutter(&pubspec) || pubspec.lines().any(|line| line.trim() == "sdk: flutter");
    if needs_flutter && (cfg!(not(unix)) || flutter.is_none()) {
        return Ok(Outcome::Skipped(
            "flutter packages need the pinned flutter SDK".to_owned(),
        ));
    }
    let work = tempfile_stage_dir("pub-api-").map_err(|error| format!("pub_deploy: {error}"))?;
    let tools = if needs_flutter {
        flutter
            .map(|(flutter_dart, root)| {
                flutter_sdk_tree(flutter_dart, &work).map(|dart| (dart, Some(root)))
            })
            .transpose()
    } else {
        Ok(Some((dart.to_path_buf(), None)))
    };
    let result = tools.and_then(|tools| {
        let (dart, flutter_root) = tools.expect("tools are chosen above");
        pub_api_compare(
            agent,
            (&dart, apitool, flutter_root),
            (package_name, version),
            (staging, registry_url),
            (baseline, package_dir),
            workspace,
            &work,
        )
    });
    let _ = std::fs::remove_dir_all(&work);
    result
}

/// Builds a Flutter SDK shaped tree under `work` whose `bin/flutter` runs `dart pub`.
///
/// dart_apitool looks for `flutter` in the `bin` directories above its own dart, so the
/// pinned dart is copied to `bin/cache/dart-sdk/bin/dart` with the rest of the SDK linked.
/// Returns the copied dart.
#[cfg(unix)]
fn flutter_sdk_tree(flutter_dart: &Path, work: &Path) -> Result<std::path::PathBuf, String> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let fail = |error: std::io::Error| format!("pub_deploy: flutter SDK: {error}");
    let sdk = flutter_dart
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("pub_deploy: bad flutter dart {}", flutter_dart.display()))?;
    let bin = work.join("flutter/bin");
    let dart_sdk = bin.join("cache/dart-sdk");
    std::fs::create_dir_all(dart_sdk.join("bin")).map_err(fail)?;
    for entry in std::fs::read_dir(sdk).map_err(fail)? {
        let name = entry.map_err(fail)?.file_name();
        if name != "bin" {
            symlink(sdk.join(&name), dart_sdk.join(&name)).map_err(fail)?;
        }
    }
    let dart = dart_sdk.join("bin/dart");
    for entry in std::fs::read_dir(sdk.join("bin")).map_err(fail)? {
        let name = entry.map_err(fail)?.file_name();
        let from = sdk.join("bin").join(&name);
        let to = dart_sdk.join("bin").join(&name);
        if name == "dart" || name == "dartaotruntime" {
            std::fs::copy(&from, &to).map_err(fail)?;
        } else {
            symlink(&from, &to).map_err(fail)?;
        }
    }
    let shim = bin.join("flutter");
    let quoted = dart.display().to_string().replace('\'', "'\\''");
    std::fs::write(&shim, format!("#!/bin/sh\nexec '{quoted}' pub \"$@\"\n")).map_err(fail)?;
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).map_err(fail)?;
    Ok(dart)
}

#[cfg(not(unix))]
fn flutter_sdk_tree(_: &Path, _: &Path) -> Result<std::path::PathBuf, String> {
    Err("pub_deploy: flutter SDK trees are not supported here".to_owned())
}

fn pub_api_compare(
    agent: &ureq::Agent,
    (dart, apitool, flutter_root): (&Path, &Path, Option<&Path>),
    (package_name, version): (&str, &str),
    (staging, registry_url): (&Path, &str),
    (baseline, package_dir): (Baseline, &str),
    workspace: &Path,
    work: &Path,
) -> Result<Outcome, String> {
    let (previous, old) = if baseline == Baseline::Git {
        let tag = match previous_git_tag(workspace, version) {
            Ok(Some(tag)) => tag,
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error)),
        };
        let dest = work.join("old");
        match export_git_dir(workspace, package_dir, &tag, &dest, "pubspec.yaml") {
            Ok(true) => {}
            Ok(false) => {
                return Ok(Outcome::Skipped(format!(
                    "no pubspec.yaml at {package_dir} in {tag}"
                )));
            }
            Err(error) => return Err(format!("pub_deploy: api compat: {error}")),
        }
        (tag, dest.display().to_string())
    } else {
        let url = format!(
            "{}/api/packages/{package_name}",
            registry_url.trim_end_matches('/')
        );
        let body = match fetch_with(agent, &url, 16 << 20) {
            Ok(Some(body)) => body,
            Ok(None) => return Ok(Outcome::FirstRelease),
            Err(error) => return Ok(Outcome::Skipped(error)),
        };
        match pub_previous_version(&body, version) {
            Some(previous) => {
                let old = format!("pub://{package_name}/{previous}");
                (previous, old)
            }
            None => return Ok(Outcome::FirstRelease),
        }
    };
    let cache = work.join("pub-cache");
    let report = work.join("report.json");
    std::fs::create_dir_all(&cache).map_err(|error| format!("pub_deploy: {error}"))?;
    let mut dirs: Vec<std::path::PathBuf> =
        dart.parent().map(Path::to_path_buf).into_iter().collect();
    dirs.extend(std::env::split_paths(&env_or_empty("PATH")));
    let path = std::env::join_paths(dirs).map_err(|error| format!("pub_deploy: {error}"))?;
    let mut env = vec![
        ("PATH", path.to_str().unwrap_or("")),
        ("PUB_CACHE", cache.to_str().unwrap_or("")),
        ("PUB_HOSTED_URL", registry_url),
    ];
    if let Some(root) = flutter_root {
        env.push(("FLUTTER_ROOT", root.to_str().unwrap_or("")));
    }
    let output = run_publish(
        scrubbed_command(dart, &[PUBLISH_CHILD_ENV_KEEP], &env)
            .current_dir(work)
            .arg(apitool)
            .args(["diff", "--old", &old, "--new"])
            .arg(staging)
            .args(["--version-check-mode", "none", "--report-format", "json"])
            .arg("--report-file-path")
            .arg(&report),
    )
    .map_err(|error| format!("pub_deploy: dart_apitool: {error}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed = std::fs::read_to_string(&report)
        .ok()
        .and_then(|report| apitool_changes(&report));
    let success = output.success() && parsed.is_some();
    Ok(Outcome::Compared {
        previous,
        changes: changes_or_failure(parsed.unwrap_or_default(), success, "dart_apitool", &text),
    })
}

/// Runs the pub deploy tool without an API compatibility check.
pub fn pub_main(
    package_rlocs: &[String],
    package_name: &str,
    version: &str,
    registry_url: &str,
    argv: &[String],
) -> i32 {
    let off = PubApiCheck {
        mode: "off",
        baseline: "registry",
        dart_rloc: "",
        apitool_rloc: "",
        flutter_dart_rloc: "",
        flutter_root_rloc: "",
        package_dir: "",
    };
    pub_main_checked(package_rlocs, package_name, version, registry_url, &off, argv)
}

/// Runs the pub deploy tool.
pub fn pub_main_checked(
    package_rlocs: &[String],
    package_name: &str,
    version: &str,
    registry_url: &str,
    api: &PubApiCheck,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "pub_deploy",
        "the package is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let mut files = Vec::new();
    for rloc in package_rlocs.iter().filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        files.push(path);
    }
    let mapped = match pub_package_paths(&files) {
        Ok(mapped) => mapped,
        Err(error) => {
            eprintln!("pub_deploy: {error}");
            return 1;
        }
    };
    let Some(mode) = ApiCompatMode::parse(api.mode) else {
        eprintln!("pub_deploy: invalid api_compat '{}'", api.mode);
        return 1;
    };
    let tools =
        if mode == ApiCompatMode::Off || api.dart_rloc.is_empty() || api.apitool_rloc.is_empty() {
            None
        } else {
            run.resolve(api.dart_rloc)
                .zip(run.resolve(api.apitool_rloc))
        };
    let flutter = if tools.is_none() || api.flutter_dart_rloc.is_empty() {
        None
    } else {
        run.resolve(api.flutter_dart_rloc)
            .zip(run.resolve(api.flutter_root_rloc))
            .and_then(|(dart, manifest)| {
                let root = manifest.parent()?.parent()?.parent()?.to_path_buf();
                Some((dart, root))
            })
    };
    let api_check = |staging: &Path, version: &str| -> Result<(), String> {
        if mode == ApiCompatMode::Off {
            return Ok(());
        }
        let outcome = match &tools {
            Some((dart, apitool)) => pub_api_check(
                &crate::http::publish_agent(),
                (dart, apitool),
                flutter
                    .as_ref()
                    .map(|(dart, root)| (dart.as_path(), root.as_path())),
                (package_name, version),
                (staging, registry_url),
                (api.baseline, api.package_dir),
                &workspace_dir(),
            ),
            None => Ok(Outcome::Skipped(
                "the Dart SDK or dart_apitool is not available".to_owned(),
            )),
        };
        outcome.and_then(|outcome| conclude("pub_deploy", mode, version, outcome))
    };
    let live = PUB_GATE.requested(env_or_empty);
    let secrets = if live {
        match PUB_GATE.authorize(env_or_empty) {
            Ok(secrets) => Some(secrets),
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    } else {
        None
    };
    let staging = match tempfile_stage_dir("pub-publish-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("pub_deploy: {error}");
            return 1;
        }
    };
    let result = pub_run_checked(
        &mapped,
        &staging,
        package_name,
        version,
        registry_url,
        secrets,
        &api_check,
    );
    let _ = std::fs::remove_dir_all(&staging);
    let version = match result {
        Ok(version) => version,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };
    if live {
        println!("pub_deploy: published {package_name} {version}");
        return 0;
    }
    match pub_build_package(&mapped, run.outdir(), package_name) {
        Ok(house) => {
            println!(
                "pub_deploy: staged {package_name} {version} (build_mode: {})",
                deploy_build_mode()
            );
            println!("  package: {}", house.display());
            0
        }
        Err(message) => {
            eprintln!("{message}");
            1
        }
    }
}

fn pub_run_checked(
    mapped: &[(std::path::PathBuf, String)],
    staging: &Path,
    package_name: &str,
    version: &str,
    registry_url: &str,
    secrets: Option<crate::run::Secrets>,
    api_check: &dyn Fn(&Path, &str) -> Result<(), String>,
) -> Result<String, String> {
    let (flutter, version) = pub_checked_stage(mapped, staging, package_name, version)?;
    if secrets.is_some() {
        if let Some(error) = placeholder_refusal("pub_deploy", &version) {
            return Err(error);
        }
    }
    api_check(staging, &version)?;
    let tool = tool_name(flutter);
    let Some(program) = which_on_path(tool) else {
        if secrets.is_some() {
            return Err(format!("pub_deploy: '{tool}' CLI not found on PATH"));
        }
        println!("pub_deploy: {tool} not found on PATH; skipped {tool} pub publish --dry-run");
        return Ok(version);
    };
    let Some(secrets) = secrets else {
        return pub_dry_run(&program, staging).map(|()| version);
    };
    pub_dry_run(&program, staging)?;
    let cache = tempfile_stage_dir("pub-cache-").map_err(|error| format!("pub_deploy: {error}"))?;
    let published = pub_publish(
        &program,
        staging,
        &cache,
        registry_url,
        secrets.get("PUB_API_TOKEN"),
    );
    let _ = std::fs::remove_dir_all(&cache);
    published.map(|()| version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    const PUBSPEC: &str =
        "name: pub_demo # the package\nversion: '1.2.3'\nenvironment:\n  sdk: ^3.0.0\n";

    #[test]
    fn pubspec_must_match_target() {
        assert_eq!(pubspec_refusal(PUBSPEC, "pub_demo"), None);
        let wrong = pubspec_refusal(PUBSPEC, "other").expect("name refused");
        assert!(wrong.contains("is pub_demo") && wrong.contains("target is other"));
        let missing = pubspec_refusal("version: 1.2.3\n", "pub_demo").expect("missing");
        assert!(missing.contains("(missing)"), "{missing}");
        assert!(pubspec_refusal("  name: pub_demo\n", "pub_demo").is_some());
    }

    #[test]
    fn flutter_packages_are_detected() {
        assert!(pubspec_is_flutter(
            "name: a\nflutter:\n  uses-material-design: true\n"
        ));
        assert!(!pubspec_is_flutter(
            "name: a\ndependencies:\n  flutter:\n    sdk: flutter\n"
        ));
    }

    #[test]
    fn changelog_needs_a_version_heading() {
        assert_eq!(
            changelog_refusal(Some("## 1.2.3\n\n- first\n"), "1.2.3"),
            None
        );
        assert_eq!(
            changelog_refusal(Some("# [v1.2.3] - 2026\n"), "1.2.3"),
            None
        );
        assert!(changelog_refusal(Some("## 1.2.30\n"), "1.2.3").is_some());
        assert!(changelog_refusal(Some("1.2.3\n"), "1.2.3").is_some());
        assert!(
            changelog_refusal(None, "1.2.3")
                .expect("missing")
                .contains("no CHANGELOG.md")
        );
    }

    fn write_package(root: &Path, pubspec: &str, changelog: Option<&str>) -> Vec<String> {
        let mut rlocs = Vec::new();
        let mut put = |rel: &str, body: &str| {
            let path = write_crate_file(root, rel, body);
            rlocs.push(path.display().to_string());
        };
        put("pkg/pubspec.yaml", pubspec);
        put("pkg/lib/pub_demo.dart", "int one() => 1;\n");
        if let Some(changelog) = changelog {
            put("pkg/CHANGELOG.md", changelog);
        }
        rlocs
    }

    fn deploy_env<R>(live: bool, f: impl FnOnce() -> R) -> R {
        let mut vars: Vec<(&str, Option<&str>)> = vec![
            ("RUNFILES_DIR", None),
            ("RUNFILES_MANIFEST_FILE", None),
            ("PUB_API_TOKEN", None),
            ("PUB_PUBLISH_APPROVED", None),
            ("DX_DEPLOY_LIVE", None),
            ("PATH", Some("")),
        ];
        if live {
            vars[2] = ("PUB_API_TOKEN", Some("secret-token"));
            vars[3] = ("PUB_PUBLISH_APPROVED", Some("1"));
            vars[4] = ("DX_DEPLOY_LIVE", Some("1"));
        }
        with_env(&vars, f)
    }

    #[test]
    fn pub_main_stages_the_checked_package() {
        let scratch = scratch_dir();
        let rlocs = write_package(scratch.path(), PUBSPEC, Some("## 1.2.3\n- first\n"));
        let outdir = scratch.path().join("out");
        let argv = vec!["launcher".to_owned(), outdir.display().to_string()];
        deploy_env(false, || {
            assert_eq!(
                pub_main(&rlocs, "pub_demo", "1.2.3", "https://pub.invalid", &argv),
                0
            );
            assert_eq!(
                pub_main(&rlocs, "pub_demo", "9.9.9", "https://pub.invalid", &argv),
                1
            );
            assert_eq!(
                pub_main(&rlocs, "pub_demo", "", "https://pub.invalid", &argv),
                0
            );
        });
        let house = outdir.join("pub_demo-package");
        assert!(house.join("pubspec.yaml").is_file());
        assert!(house.join("lib/pub_demo.dart").is_file());
    }

    #[test]
    fn pub_main_refuses_a_package_without_changelog() {
        let scratch = scratch_dir();
        let rlocs = write_package(scratch.path(), PUBSPEC, None);
        let argv = vec![
            "launcher".to_owned(),
            scratch.path().join("out").display().to_string(),
        ];
        deploy_env(false, || {
            assert_eq!(
                pub_main(&rlocs, "pub_demo", "1.2.3", "https://pub.invalid", &argv),
                1
            );
        });
    }

    #[test]
    fn live_publish_is_gated() {
        let scratch = scratch_dir();
        let rlocs = write_package(scratch.path(), PUBSPEC, Some("## 1.2.3\n"));
        let argv = vec![
            "launcher".to_owned(),
            scratch.path().join("out").display().to_string(),
        ];
        let placeholder = write_package(
            &scratch.path().join("zero"),
            "name: pub_demo\nversion: 0.0.0\n",
            Some("## 0.0.0\n"),
        );
        deploy_env(true, || {
            assert_eq!(
                pub_main(
                    &placeholder,
                    "pub_demo",
                    "0.0.0",
                    "https://pub.invalid",
                    &argv
                ),
                1
            );
            assert_eq!(
                pub_main(&rlocs, "pub_demo", "1.2.3", "https://pub.invalid", &argv),
                1,
                "a live run without dart on PATH fails"
            );
        });
        with_env(
            &[
                ("RUNFILES_DIR", None),
                ("PUB_API_TOKEN", None),
                ("PUB_PUBLISH_APPROVED", None),
                ("DX_DEPLOY_LIVE", Some("1")),
            ],
            || {
                assert_eq!(
                    pub_main(&rlocs, "pub_demo", "1.2.3", "https://pub.invalid", &argv),
                    1
                );
            },
        );
    }

    const APITOOL_REPORT: &str = include_str!("../testdata/pub/apitool_report.json");

    #[test]
    fn apitool_report_lists_each_breaking_change() {
        assert_eq!(
            apitool_changes(APITOOL_REPORT),
            Some(vec![
                "Class Greeter: Field \"count\" removed (CF01)".to_owned(),
                "Class Greeter > Method hello: Parameter \"times\" added (CE02)".to_owned(),
            ])
        );
        assert_eq!(apitool_changes(r#"{"report":{}}"#), Some(vec![]));
        assert_eq!(apitool_changes("not json"), None);
    }

    #[test]
    fn previous_version_skips_retracted_and_prerelease() {
        let body = br#"{"versions":[{"version":"1.0.0"},{"version":"1.2.0","retracted":true},{"version":"2.0.0-dev.1"}]}"#;
        assert_eq!(
            pub_previous_version(body, "3.0.0").as_deref(),
            Some("1.0.0")
        );
        assert_eq!(pub_previous_version(body, "1.0.0"), None);
        assert_eq!(pub_previous_version(b"<html>", "1.0.0"), None);
    }

    #[cfg(unix)]
    #[test]
    fn api_check_fails_a_breaking_minor_and_passes_a_major_against_the_registry() {
        let scratch = scratch_dir();
        let registry = static_registry(|_| {
            vec![(
                "/api/packages/pub_demo".to_owned(),
                br#"{"versions":[{"version":"0.9.0"},{"version":"1.0.0"},{"version":"1.0.5","retracted":true},{"version":"2.0.0-beta.1"}]}"#.to_vec(),
            )]
        });
        let report = scratch.path().join("captured.json");
        std::fs::write(&report, APITOOL_REPORT).expect("report");
        let (args_log, env_log) = (scratch.path().join("args"), scratch.path().join("env"));
        let fake = scratch.path().join("bin/dart");
        std::fs::create_dir_all(scratch.path().join("bin")).expect("bin");
        dx_testing::executable_script(
            &fake,
            &format!(
                "#!/bin/sh\necho \"$@\" > '{}'\nenv > '{}'\nwhile [ $# -gt 0 ]; do\n  [ \"$1\" = --report-file-path ] && cp '{}' \"$2\"\n  shift\ndone\nexit 0\n",
                args_log.display(),
                env_log.display(),
                report.display()
            ),
        );
        let staging = scratch.path().join("stage");
        write_crate_file(&staging, "pubspec.yaml", PUBSPEC);
        let agent = crate::http::publish_agent_with_proxy(None);
        let run_with =
            |name: &str, version: &str, staging: &Path, flutter_sdk: Option<(&Path, &Path)>| {
                with_env(&[("PUB_API_TOKEN", Some("sekret"))], || {
                    pub_api_check(
                        &agent,
                        (&fake, Path::new("dart_apitool.dill")),
                        flutter_sdk,
                        (name, version),
                        (staging, &registry),
                        ("registry", ""),
                        scratch.path(),
                    )
                    .expect("ran")
                })
            };
        let run =
            |name: &str, version: &str, staging: &Path| run_with(name, version, staging, None);
        let outcome = run("pub_demo", "1.1.0", &staging);
        assert_eq!(
            outcome,
            Outcome::Compared {
                previous: "1.0.0".to_owned(),
                changes: apitool_changes(APITOOL_REPORT).expect("report"),
            }
        );
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert!(
            logged.starts_with("dart_apitool.dill diff --old pub://pub_demo/1.0.0 --new ")
                && logged.contains("--version-check-mode none --report-format json"),
            "{logged}"
        );
        let env = std::fs::read_to_string(&env_log).expect("env");
        assert!(env.contains(&format!("PUB_HOSTED_URL={registry}")), "{env}");
        assert!(env.contains("PUB_CACHE="), "{env}");
        assert!(
            env.contains(&format!("PATH={}", scratch.path().join("bin").display())),
            "{env}"
        );
        assert!(!env.contains("sekret"), "{env}");
        assert!(conclude("pub_deploy", ApiCompatMode::Fail, "1.1.0", outcome).is_err());
        let major = run("pub_demo", "2.0.0", &staging);
        assert!(conclude("pub_deploy", ApiCompatMode::Fail, "2.0.0", major).is_ok());
        assert_eq!(run("missing", "1.0.0", &staging), Outcome::FirstRelease);
        let flutter = scratch.path().join("flutter");
        write_crate_file(
            &flutter,
            "pubspec.yaml",
            "name: pub_demo\ndependencies:\n  flutter:\n    sdk: flutter\n",
        );
        assert_eq!(
            run("pub_demo", "1.1.0", &flutter),
            Outcome::Skipped("flutter packages need the pinned flutter SDK".to_owned())
        );
        let sdk = scratch.path().join("flutter_sdk/dart-sdk");
        std::fs::create_dir_all(sdk.join("bin")).expect("sdk");
        std::fs::create_dir_all(sdk.join("lib")).expect("lib");
        let seen = scratch.path().join("seen");
        let pinned = sdk.join("bin/dart");
        dx_testing::executable_script(
            &pinned,
            &format!(
                "#!/bin/sh\n[ -x \"$(dirname \"$0\")/../../../flutter\" ] && echo \"shim $FLUTTER_ROOT\" > '{}'\nwhile [ $# -gt 0 ]; do\n  [ \"$1\" = --report-file-path ] && cp '{}' \"$2\"\n  shift\ndone\nexit 0\n",
                seen.display(),
                report.display()
            ),
        );
        let root = scratch.path().join("flutter_root");
        let pinned_sdk = Some((pinned.as_path(), root.as_path()));
        let breaking = run_with("pub_demo", "1.1.0", &flutter, pinned_sdk);
        assert!(conclude("pub_deploy", ApiCompatMode::Fail, "1.1.0", breaking).is_err());
        assert_eq!(
            std::fs::read_to_string(&seen).expect("seen").trim(),
            format!("shim {}", root.display())
        );
        let major = run_with("pub_demo", "2.0.0", &flutter, pinned_sdk);
        assert!(conclude("pub_deploy", ApiCompatMode::Fail, "2.0.0", major).is_ok());
    }

    #[cfg(unix)]
    fn fake_dart(dir: &Path, exit: i32) -> std::path::PathBuf {
        let path = dir.join("dart");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{d}/args.txt'\nenv >> '{d}/env.txt'\nexit {exit}\n"
        );
        dx_testing::executable_script(&path, &script);
        path
    }

    #[cfg(unix)]
    #[test]
    fn publish_keeps_the_token_out_of_argv() {
        let scratch = scratch_dir();
        let dart = fake_dart(scratch.path(), 0);
        let stage = scratch.path().join("stage");
        std::fs::create_dir_all(&stage).expect("stage");
        let cache = scratch.path().join("cache");
        with_env(&[("DX_LEAK_PROBE", Some("1"))], || {
            pub_publish(&dart, &stage, &cache, "https://pub.invalid", "sekret").expect("publish");
        });
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(args.contains("--force") && args.contains("--env-var"));
        assert!(!args.contains("sekret"));
        assert!(env.contains("PUB_TOKEN=sekret"));
        assert!(env.contains(&format!("PUB_CACHE={}", cache.display())));
        assert!(!env.contains("DX_LEAK_PROBE"));
    }

    #[cfg(unix)]
    #[test]
    fn tool_failure_is_reported_without_the_token() {
        let scratch = scratch_dir();
        let dart = fake_dart(scratch.path(), 65);
        let stage = scratch.path().join("stage");
        std::fs::create_dir_all(&stage).expect("stage");
        let error = pub_dry_run(&dart, &stage).expect_err("dry run fails");
        assert!(error.contains("pub publish --dry-run failed"), "{error}");
        let error = pub_publish(
            &dart,
            &stage,
            &scratch.path().join("c"),
            "https://p.invalid",
            "sekret",
        )
        .expect_err("token add fails");
        assert!(error.contains("pub token add failed") && !error.contains("sekret"));
    }
}
