//! Go module API compatibility check for github_deploy, using apidiff.

use crate::api_compat::{Baseline, Outcome, fetch, highest_below, previous_git_tag};
use crate::archive::archive_extract;
use crate::run::{PUBLISH_CHILD_ENV_KEEP, exit_text, run_publish, scrubbed_command};
use crate::stage::tempfile_stage_dir;
use std::path::Path;

/// Returns the lines under `Incompatible changes:` in apidiff output.
pub(crate) fn incompatible_changes(output: &str) -> Vec<String> {
    let mut inside = false;
    let mut changes = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line == "Incompatible changes:" {
            inside = true;
        } else if line.is_empty() || line.ends_with("changes:") {
            inside = false;
        } else if let Some(change) = line.strip_prefix("- ").filter(|_| inside) {
            changes.push(change.to_owned());
        }
    }
    changes
}

/// Where module versions and zips are read from.
const GO_PROXY: &str = "https://proxy.golang.org";

/// Escapes a module path for the module proxy: each capital letter becomes `!` and its lower case.
fn proxy_escape(path: &str) -> String {
    let mut out = String::new();
    for c in path.chars() {
        if c.is_ascii_uppercase() {
            out.push('!');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Returns the highest `v*` version below `tag` in a proxy `@v/list` body.
fn proxy_previous(list: &str, tag: &str) -> Option<String> {
    highest_below(list.lines().filter_map(|v| v.trim().strip_prefix('v')), tag)
        .map(|version| format!("v{version}"))
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
fn run_apidiff(
    program: &Path,
    extra: &[(&str, &str)],
    dir: &Path,
    args: &[&str],
) -> Result<String, String> {
    let mut cmd = scrubbed_command(
        program,
        &[PUBLISH_CHILD_ENV_KEEP, GO_CHILD_ENV_EXTRA],
        extra,
    );
    cmd.args(args).current_dir(dir);
    let output =
        run_publish(&mut cmd).map_err(|error| format!("github_deploy: apidiff: {error}"))?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if !output.success() {
        return Err(format!(
            "github_deploy: apidiff failed in {} with {}\n{}",
            dir.display(),
            exit_text(&output),
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
    dx_process::capture::run_checked(
        dx_process::capture::git(workspace).args(args),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .map(|output| output.stdout_text())
    .map_err(|error| error.to_string())
}

/// Returns the breaking changes of the module in `new` against the module in `old`.
fn compare_module(
    apidiff: &Path,
    extra: &[(&str, &str)],
    (old, new): (&Path, &Path),
    path: &str,
    export: &Path,
) -> Result<Vec<String>, String> {
    let export = export.to_string_lossy();
    run_apidiff(apidiff, extra, old, &["-m", "-w", &export, path])?;
    let text = run_apidiff(apidiff, extra, new, &["-m", &export, path])?;
    Ok(incompatible_changes(&text)
        .into_iter()
        .map(|change| format!("{path}: {change}"))
        .collect())
}

/// Returns the outcome of each module against the earlier version `baseline` picks.
/// `go` is the pinned go binary apidiff runs; a module absent at the baseline is skipped.
pub(crate) fn go_api_check(
    (apidiff, go): (&Path, Option<&Path>),
    workspace: &Path,
    modules: &[String],
    tag: &str,
    baseline: Baseline,
) -> Result<Vec<Outcome>, String> {
    let detected = [String::new()];
    let modules = if modules.is_empty() {
        if !workspace.join("go.mod").is_file() {
            return Ok(Vec::new());
        }
        &detected[..]
    } else {
        modules
    };
    let path_env = go.and_then(Path::parent).map(|dir| {
        let mut dirs = vec![dir.to_path_buf()];
        dirs.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(dirs)
            .map_or_else(|_| String::new(), |p| p.to_string_lossy().into_owned())
    });
    let extra: Vec<(&str, &str)> = path_env
        .iter()
        .map(|path| ("PATH", path.as_str()))
        .collect();
    let work = tempfile_stage_dir("go-api-")
        .map_err(|error| format!("github_deploy: api compat: {error}"))?;
    let mut outcomes = Vec::new();
    let mut checked = Ok(());
    let mut worktree = None;
    for (index, module) in modules.iter().enumerate() {
        let rel = module.trim_matches('/');
        let new = workspace.join(rel);
        let Some(path) = module_path(&new.join("go.mod")) else {
            checked = Err(format!("github_deploy: no module line in {module}/go.mod"));
            break;
        };
        let old = if baseline == Baseline::Git {
            go_git_baseline(workspace, &work, &mut worktree, tag)
                .map(|found| found.map(|(previous, tree)| (previous, tree.join(rel))))
        } else {
            go_proxy_baseline(&path, tag, &work.join(index.to_string()))
        };
        let (previous, old) = match old {
            Ok(Some(found)) => found,
            Ok(None) => {
                outcomes.push(Outcome::FirstRelease);
                continue;
            }
            Err(why) => {
                outcomes.push(Outcome::Skipped(why));
                continue;
            }
        };
        if !old.join("go.mod").is_file() {
            outcomes.push(Outcome::Skipped(format!(
                "no go.mod for {path} at {previous}"
            )));
            continue;
        }
        let export = work.join(format!("{index}.export"));
        match compare_module(apidiff, &extra, (&old, &new), &path, &export) {
            Ok(changes) => outcomes.push(Outcome::Compared { previous, changes }),
            Err(error) => {
                checked = Err(error);
                break;
            }
        }
    }
    if worktree.is_some() {
        let tree = work.join("tree");
        let _ = git(
            workspace,
            &["worktree", "remove", "--force", &tree.to_string_lossy()],
        );
    }
    let _ = std::fs::remove_dir_all(&work);
    checked.map(|()| outcomes)
}

/// Checks out the highest `v*` tag below `tag` once under `work/tree`.
fn go_git_baseline(
    workspace: &Path,
    work: &Path,
    worktree: &mut Option<String>,
    tag: &str,
) -> Result<Option<(String, std::path::PathBuf)>, String> {
    let tree = work.join("tree");
    if let Some(previous) = worktree {
        return Ok(Some((previous.clone(), tree)));
    }
    let Some(previous) = previous_git_tag(workspace, tag)? else {
        return Ok(None);
    };
    git(
        workspace,
        &[
            "worktree",
            "add",
            "--detach",
            &tree.to_string_lossy(),
            &previous,
        ],
    )?;
    *worktree = Some(previous.clone());
    Ok(Some((previous, tree)))
}

/// Downloads the highest proxy version of `path` below `tag` and returns its module directory.
fn go_proxy_baseline(
    path: &str,
    tag: &str,
    dir: &Path,
) -> Result<Option<(String, std::path::PathBuf)>, String> {
    let base = format!("{GO_PROXY}/{}/@v", proxy_escape(path));
    let Some(list) = fetch(&format!("{base}/list"), 1 << 20)? else {
        return Ok(None);
    };
    let Some(previous) = proxy_previous(&String::from_utf8_lossy(&list), tag) else {
        return Ok(None);
    };
    let url = format!("{base}/{previous}.zip");
    let Some(zip) = fetch(&url, 512 << 20)? else {
        return Err(format!("{url} not found"));
    };
    let archive = dir.with_extension("zip");
    std::fs::write(&archive, zip).map_err(|error| format!("{}: {error}", archive.display()))?;
    archive_extract(&archive, dir).map_err(|error| format!("{url}: {error}"))?;
    Ok(Some((
        previous.clone(),
        dir.join(format!("{path}@{previous}")),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn counts_only_incompatible_lines() {
        let text = "Incompatible changes:\n- Gone: removed\n- Other: changed\n\nCompatible changes:\n- New: added\n";
        assert_eq!(
            incompatible_changes(text),
            vec!["Gone: removed", "Other: changed"]
        );
        assert!(incompatible_changes("Compatible changes:\n- New: added\n").is_empty());
    }

    #[test]
    fn proxy_paths_escape_capitals_and_pick_the_version_below() {
        assert_eq!(
            proxy_escape("github.com/Azure/SDK"),
            "github.com/!azure/!s!d!k"
        );
        let list = "v1.0.0\nv1.2.0\nv1.3.0-rc.1\nv2.0.0\n";
        assert_eq!(proxy_previous(list, "v1.3.0").as_deref(), Some("v1.2.0"));
        assert_eq!(proxy_previous(list, "v1.0.0"), None);
    }

    #[test]
    fn proxy_zip_extracts_under_the_module_prefix() {
        let scratch = scratch_dir();
        let zip = crate::archive::zip_archive(&[
            (
                "example.com/Lib@v1.2.0/go.mod",
                b"module example.com/Lib\n",
                0o644,
            ),
            ("example.com/Lib@v1.2.0/lib.go", b"package lib\n", 0o644),
        ])
        .expect("zip");
        let archive = scratch.path().join("0.zip");
        std::fs::write(&archive, zip).expect("write");
        let dir = scratch.path().join("0");
        archive_extract(&archive, &dir).expect("extract");
        assert!(dir.join("example.com/Lib@v1.2.0/go.mod").is_file());
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
            dx_process::capture::run_checked(
                dx_process::capture::git(&repo).args(&args),
                dx_process::capture::DEFAULT_TIMEOUT,
            )
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        }
        repo
    }

    #[cfg(unix)]
    #[test]
    fn removed_function_is_reported_against_the_tag() {
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
        let run = |tag: &str| go_api_check((&fake, None), &repo, &modules, tag, Baseline::Git);
        assert_eq!(
            run("v1.3.0"),
            Ok(vec![Outcome::Compared {
                previous: "v1.2.0".to_owned(),
                changes: vec!["example.com/lib: Hello: removed".to_owned()],
            }])
        );
        let logged = std::fs::read_to_string(&args_log).expect("args");
        assert!(logged.starts_with("-m -w "), "{logged}");
        assert!(logged.contains("example.com/lib"), "{logged}");
        assert_eq!(run("v1.0.0"), Ok(vec![Outcome::FirstRelease]));
        assert_eq!(
            go_api_check((&fake, None), &repo, &[], "v1.3.0", Baseline::Git),
            Ok(Vec::new())
        );
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
                let outcomes = go_api_check(
                    (&apidiff, Some(&go)),
                    &repo,
                    &modules,
                    "v1.3.0",
                    Baseline::Git,
                )
                .expect("apidiff ran");
                assert!(
                    matches!(&outcomes[..], [Outcome::Compared { previous, changes }] if previous == "v1.2.0" && changes.len() == 1),
                    "{outcomes:?}"
                );
            },
        );
    }
}
