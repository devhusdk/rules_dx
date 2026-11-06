//! Go module API compatibility check for github_deploy, using apidiff.

use crate::api_compat::{ApiCompatMode, breaking_refusal, previous_tag, required_bump};
use crate::run::{PUBLISH_CHILD_ENV_KEEP, scrubbed_command};
use crate::stage::tempfile_stage_dir;
use std::path::Path;

/// Counts the lines under `Incompatible changes:` in apidiff output.
pub(crate) fn count_incompatible(output: &str) -> usize {
    let mut inside = false;
    let mut count = 0;
    for line in output.lines() {
        let line = line.trim();
        if line == "Incompatible changes:" {
            inside = true;
        } else if line.is_empty() || line.ends_with("changes:") {
            inside = false;
        } else if inside && line.starts_with("- ") {
            count += 1;
        }
    }
    count
}

/// Returns the module path of a go.mod file.
fn module_path(go_mod: &Path) -> Option<String> {
    let text = std::fs::read_to_string(go_mod).ok()?;
    text.lines().find_map(|line| {
        let path = line.trim().strip_prefix("module")?.trim();
        (!path.is_empty()).then(|| path.trim_matches('"').to_owned())
    })
}

/// Runs apidiff inside a module directory; apidiff shells out to `go`.
fn run_apidiff(program: &Path, dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = scrubbed_command(program, &[PUBLISH_CHILD_ENV_KEEP, GO_CHILD_ENV_EXTRA], &[]);
    cmd.args(args).current_dir(dir);
    let output = cmd
        .output()
        .map_err(|error| format!("github_deploy: apidiff: {error}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.status.success() {
        return Err(format!(
            "github_deploy: apidiff failed in {} with {}\n{}",
            dir.display(),
            output.status,
            text.trim()
        ));
    }
    Ok(text)
}

const GO_CHILD_ENV_EXTRA: &[&str] = &[
    "GOPATH",
    "GOCACHE",
    "GOMODCACHE",
    "GOFLAGS",
    "GOPROXY",
    "GONOSUMDB",
    "GOROOT",
];

fn git(workspace: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs apidiff for each module against the highest `v*` tag below `tag`.
///
/// A missing tool, a missing earlier tag or a module absent at that tag skips with a note.
pub(crate) fn go_api_check(
    apidiff: Option<&Path>,
    workspace: &Path,
    modules: &[String],
    tag: &str,
    mode: ApiCompatMode,
) -> Result<(), String> {
    if mode == ApiCompatMode::Off {
        return Ok(());
    }
    let detected = [String::new()];
    let modules = if modules.is_empty() {
        if !workspace.join("go.mod").is_file() {
            return Ok(());
        }
        &detected[..]
    } else {
        modules
    };
    let Some(program) = apidiff else {
        println!("api compat: apidiff or go not found; skipped");
        return Ok(());
    };
    let version = tag.trim_start_matches('v');
    let tags = git(workspace, &["tag", "--list", "v*"])
        .map_err(|error| format!("github_deploy: api compat: {error}"))?;
    let Some(previous) = previous_tag(&tags, version) else {
        println!("api compat: no earlier v<version> tag; skipped (first release)");
        return Ok(());
    };
    let old_tree = tempfile_stage_dir("go-api-base-")
        .map_err(|error| format!("github_deploy: api compat: {error}"))?;
    let added = git(
        workspace,
        &[
            "worktree",
            "add",
            "--detach",
            &old_tree.to_string_lossy(),
            &previous,
        ],
    );
    let checked = added
        .map_err(|error| format!("github_deploy: api compat: {error}"))
        .and_then(|_| {
            let mut breaking = 0;
            for module in modules {
                let rel = module.trim_matches('/');
                let old = old_tree.join(rel);
                if !old.join("go.mod").is_file() {
                    println!("api compat: no go.mod at {module} in {previous}; skipped");
                    continue;
                }
                let new = workspace.join(rel);
                let Some(path) = module_path(&new.join("go.mod")) else {
                    return Err(format!("github_deploy: no module line in {module}/go.mod"));
                };
                let export = old_tree.join("apidiff.export");
                run_apidiff(
                    program,
                    &old,
                    &["-m", "-w", &export.to_string_lossy(), &path],
                )?;
                let text = run_apidiff(program, &new, &["-m", &export.to_string_lossy(), &path])?;
                let found = count_incompatible(&text);
                for line in text.lines().filter(|line| line.starts_with("- ")) {
                    println!("apidiff: {module}: {line}");
                }
                breaking += found;
            }
            println!("api compat: {breaking} breaking changes against {previous}");
            match breaking_refusal(mode, required_bump(&previous[1..], version), breaking) {
                Some(reason) => Err(format!(
                    "github_deploy: api compat: {reason} ({previous} -> {tag})"
                )),
                None => Ok(()),
            }
        });
    let _ = git(
        workspace,
        &["worktree", "remove", "--force", &old_tree.to_string_lossy()],
    );
    let _ = std::fs::remove_dir_all(&old_tree);
    checked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn counts_only_incompatible_lines() {
        let text = "Incompatible changes:\n- Gone: removed\n- Other: changed\n\nCompatible changes:\n- New: added\n";
        assert_eq!(count_incompatible(text), 2);
        assert_eq!(count_incompatible("Compatible changes:\n- New: added\n"), 0);
    }

    #[cfg(unix)]
    fn git_fixture(scratch: &Path) -> std::path::PathBuf {
        let repo = scratch.join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        write_crate_file(&repo, "lib/go.mod", "module example.com/lib\n\ngo 1.22\n");
        write_crate_file(&repo, "lib/lib.go", "package lib\n\nfunc Hello() {}\n");
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.com",
                "commit",
                "-q",
                "-m",
                "x",
            ],
            vec!["tag", "v1.2.0"],
        ] {
            let status = std::process::Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .status()
                .expect("git");
            assert!(status.success(), "git {args:?}");
        }
        repo
    }

    #[cfg(unix)]
    #[test]
    fn removed_function_needs_a_major_bump() {
        let scratch = scratch_dir();
        let repo = git_fixture(scratch.path());
        let args_log = scratch.path().join("args");
        let fake = scratch.path().join("apidiff");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\necho \"$@\" >> {}\ncase \"$*\" in *-w*) exit 0;; esac\necho 'Incompatible changes:'\necho '- Hello: removed'\n",
                args_log.display()
            ),
        )
        .expect("fake tool");
        set_mode(&fake, 0o755);
        let modules = vec!["lib".to_owned()];
        let run =
            |tag: &str, mode: ApiCompatMode| go_api_check(Some(&fake), &repo, &modules, tag, mode);
        let refused = run("v1.3.0", ApiCompatMode::Fail).expect_err("minor bump refused");
        assert!(refused.contains("v1.2.0 -> v1.3.0"), "{refused}");
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert!(logged.starts_with("-m -w "), "{logged}");
        assert!(logged.contains("example.com/lib"), "{logged}");
        assert!(run("v2.0.0", ApiCompatMode::Fail).is_ok());
        assert!(go_api_check(Some(&fake), &repo, &[], "v1.3.0", ApiCompatMode::Fail).is_ok());
        assert!(run("v1.3.0", ApiCompatMode::Warn).is_ok());
        std::fs::remove_file(&args_log).expect("clear log");
        assert!(run("v1.3.0", ApiCompatMode::Off).is_ok());
        assert!(!args_log.exists(), "off must not run the tool");
        let listed = git(&repo, &["worktree", "list"]).expect("list");
        assert_eq!(listed.lines().count(), 1, "{listed}");
    }

    #[cfg(unix)]
    #[test]
    fn real_apidiff_flags_a_removed_function() {
        let (Some(apidiff), Some(go)) = (
            std::env::var("APIDIFF_BIN").ok(),
            std::env::var("DX_TEST_GO").ok(),
        ) else {
            println!("skipped: run under Bazel for the apidiff pin and the Go toolchain");
            return;
        };
        let apidiff = crate::stage::resolve_runfile(&apidiff).expect("apidiff");
        let go = crate::stage::resolve_runfile(&go).expect("go");
        let go_dir = go.parent().expect("go dir").to_string_lossy().into_owned();
        let scratch = scratch_dir();
        let repo = git_fixture(scratch.path());
        write_crate_file(&repo, "lib/lib.go", "package lib\n");
        let cache = scratch.path().join("gocache");
        let path = format!("{go_dir}:/usr/bin:/bin");
        with_env(
            &[
                ("PATH", Some(path.as_str())),
                ("GOCACHE", cache.to_str()),
                ("GOFLAGS", Some("-mod=mod")),
                ("GOPROXY", Some("off")),
            ],
            || {
                let modules = vec!["lib".to_owned()];
                let run = |tag: &str| {
                    go_api_check(Some(&apidiff), &repo, &modules, tag, ApiCompatMode::Fail)
                };
                let refused = run("v1.3.0").expect_err("minor bump refused");
                assert!(refused.contains("v1.2.0 -> v1.3.0"), "{refused}");
                run("v2.0.0").expect("a major bump allows the removal");
            },
        );
    }
}
