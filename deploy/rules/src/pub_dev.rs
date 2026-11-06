//! pub.dev publisher: package staging, pubspec checks and the dart or flutter publish.

use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_profile, env_or_empty,
    placeholder_refusal, scrubbed_command,
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
    credentials: &["PUB_TOKEN"],
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
fn pubspec_refusal(text: &str, package_name: &str, version: &str) -> Option<String> {
    let name = pubspec_field(text, "name");
    let found = pubspec_field(text, "version");
    if name.as_deref() == Some(package_name) && found.as_deref() == Some(version) {
        return None;
    }
    let show = |value: &Option<String>| value.clone().unwrap_or_else(|| "(missing)".to_owned());
    Some(format!(
        "pub_deploy: pubspec.yaml is {} {}, target is {package_name} {version}; refusing",
        show(&name),
        show(&found)
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

/// Stages the package and checks pubspec.yaml and CHANGELOG.md against the target.
fn pub_checked_stage(
    mapped: &[(std::path::PathBuf, String)],
    staging: &Path,
    package_name: &str,
    version: &str,
) -> Result<bool, String> {
    pub_stage(mapped, staging).map_err(|error| format!("pub_deploy: {error}"))?;
    let pubspec = std::fs::read_to_string(staging.join("pubspec.yaml"))
        .map_err(|error| format!("pub_deploy: cannot read pubspec.yaml: {error}"))?;
    if let Some(error) = pubspec_refusal(&pubspec, package_name, version) {
        return Err(error);
    }
    let changelog = std::fs::read_to_string(staging.join("CHANGELOG.md")).ok();
    if let Some(error) = changelog_refusal(changelog.as_deref(), version) {
        return Err(error);
    }
    Ok(pubspec_is_flutter(&pubspec))
}

fn tool_name(flutter: bool) -> &'static str {
    if flutter { "flutter" } else { "dart" }
}

fn failure_message(
    step: &str,
    status: std::process::ExitStatus,
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
    let output = cmd
        .output()
        .map_err(|error| format!("pub_deploy: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let mut both = output.stdout;
    both.extend_from_slice(&output.stderr);
    Err(failure_message(
        "pub publish --dry-run",
        output.status,
        &both,
        "",
    ))
}

/// Publishes the staged package with a token held only in a temporary pub cache.
fn pub_publish(
    program: &Path,
    staging: &Path,
    cache: &Path,
    hosted_url: &str,
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
    add.args(["pub", "token", "add", hosted_url, "--env-var", "PUB_TOKEN"]);
    let added = add
        .output()
        .map_err(|error| format!("pub_deploy: {error}"))?;
    if !added.status.success() {
        let mut both = added.stdout;
        both.extend_from_slice(&added.stderr);
        return Err(failure_message("pub token add", added.status, &both, token));
    }
    let mut publish = command();
    publish.args(["pub", "publish", "--force"]);
    let output = publish
        .output()
        .map_err(|error| format!("pub_deploy: {error}"))?;
    let mut both = output.stdout;
    both.extend_from_slice(&output.stderr);
    if output.status.success() {
        return Ok(());
    }
    Err(failure_message("pub publish", output.status, &both, token))
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

/// Runs the pub deploy tool.
pub fn pub_main(
    package_rlocs: &[String],
    package_name: &str,
    version: &str,
    hosted_url: &str,
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
    let live = PUB_GATE.requested(env_or_empty);
    let secrets = if live {
        if let Some(error) = placeholder_refusal("pub_deploy", version) {
            eprintln!("{error}");
            return 1;
        }
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
        hosted_url,
        secrets,
    );
    let _ = std::fs::remove_dir_all(&staging);
    if let Err(message) = result {
        eprintln!("{message}");
        return 1;
    }
    if live {
        println!("pub_deploy: published {package_name} {version}");
        return 0;
    }
    match pub_build_package(&mapped, run.outdir(), package_name) {
        Ok(house) => {
            println!(
                "pub_deploy: staged {package_name} {version} (profile: {})",
                deploy_profile()
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
    hosted_url: &str,
    secrets: Option<crate::run::Secrets>,
) -> Result<(), String> {
    let flutter = pub_checked_stage(mapped, staging, package_name, version)?;
    let tool = tool_name(flutter);
    let Some(program) = which_on_path(tool) else {
        if secrets.is_some() {
            return Err(format!("pub_deploy: '{tool}' CLI not found on PATH"));
        }
        println!("pub_deploy: {tool} not found on PATH; skipped {tool} pub publish --dry-run");
        return Ok(());
    };
    let Some(secrets) = secrets else {
        return pub_dry_run(&program, staging);
    };
    pub_dry_run(&program, staging)?;
    let cache = tempfile_stage_dir("pub-cache-").map_err(|error| format!("pub_deploy: {error}"))?;
    let published = pub_publish(
        &program,
        staging,
        &cache,
        hosted_url,
        secrets.get("PUB_TOKEN"),
    );
    let _ = std::fs::remove_dir_all(&cache);
    published
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    const PUBSPEC: &str =
        "name: pub_demo # the package\nversion: '1.2.3'\nenvironment:\n  sdk: ^3.0.0\n";

    #[test]
    fn pubspec_must_match_target() {
        assert_eq!(pubspec_refusal(PUBSPEC, "pub_demo", "1.2.3"), None);
        let wrong = pubspec_refusal(PUBSPEC, "pub_demo", "2.0.0").expect("version refused");
        assert!(wrong.contains("pub_demo 1.2.3") && wrong.contains("target is pub_demo 2.0.0"));
        let missing = pubspec_refusal("name: pub_demo\n", "pub_demo", "1.2.3").expect("missing");
        assert!(missing.contains("(missing)"), "{missing}");
        assert!(
            pubspec_refusal("  name: pub_demo\n  version: 1.2.3\n", "pub_demo", "1.2.3").is_some()
        );
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
            ("PUB_TOKEN", None),
            ("PUB_PUBLISH_APPROVED", None),
            ("DX_DEPLOY_LIVE", None),
            ("PATH", Some("")),
        ];
        if live {
            vars[2] = ("PUB_TOKEN", Some("secret-token"));
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
                ("PUB_TOKEN", None),
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

    #[cfg(unix)]
    fn fake_dart(dir: &Path, exit: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("dart");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{d}/args.txt'\nenv >> '{d}/env.txt'\nexit {exit}\n"
        );
        std::fs::write(&path, script).expect("write dart");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
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
