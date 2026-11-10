//! GitHub release publisher: asset staging and the gh release upload.

use crate::api_compat::{ApiCompatMode, Baseline, Outcome, conclude};
use crate::archive::*;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, env_or_empty, exit_text, placeholder_refusal, run_publish,
    scrubbed_command,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Where the release body comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notes {
    /// No body.
    None,
    /// The grouped conventional-commit list since the previous tag.
    Commits,
    /// A markdown file from the runfiles.
    File(String),
}

impl Notes {
    /// Reads the `notes` and `notes_rloc` config fields.
    pub fn from_config(mode: &str, rloc: &str) -> Notes {
        match mode {
            "commits" => Notes::Commits,
            "file" => Notes::File(rloc.to_owned()),
            _ => Notes::None,
        }
    }
}

/// Formats one GitHub release command.
pub fn github_release_command(tag: &str, asset_bases: &[String]) -> String {
    release_command(tag, asset_bases, false)
}

fn release_command(tag: &str, asset_bases: &[String], with_notes: bool) -> String {
    let mut cmd = format!("gh release create {tag}");
    for base in asset_bases {
        cmd.push_str(&format!(" {base}"));
    }
    cmd.push_str(" --draft --verify-tag");
    if with_notes {
        cmd.push_str(" --notes-file notes.md");
    }
    cmd
}

fn git_output(workspace: Option<&std::ffi::OsString>, args: &[&str]) -> Result<String, String> {
    let dir = workspace.map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
    let out = dx_process::capture::run_checked(
        dx_process::capture::git(&dir).args(args),
        dx_process::capture::DEFAULT_TIMEOUT,
    )
    .map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Resolves `current` to the tag that points at HEAD; other tags pass through.
pub(crate) fn resolve_tag(
    tag: &str,
    workspace: Option<&std::ffi::OsString>,
) -> Result<String, String> {
    if tag != "current" {
        return Ok(tag.to_owned());
    }
    let out = git_output(
        workspace,
        &["tag", "--points-at", "HEAD", "--sort=-v:refname"],
    )?;
    out.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "tag = \"current\" but no tag points at HEAD".to_owned())
}

/// Renders the grouped commit list between the tag before `tag` and `tag`.
pub(crate) fn commit_notes(
    tag: &str,
    workspace: Option<&std::ffi::OsString>,
) -> Result<String, String> {
    let previous = git_output(
        workspace,
        &["describe", "--tags", "--abbrev=0", &format!("{tag}^")],
    )
    .ok()
    .map(|out| out.trim().to_owned())
    .filter(|out| !out.is_empty());
    let range = match previous {
        Some(previous) => format!("{previous}..{tag}"),
        None => tag.to_owned(),
    };
    let log = git_output(workspace, &["log", "--format=%H%x00%s%x00%b%x1e", &range])?;
    let commits = dx_update::conventional::parse(&log);
    Ok(dx_update::conventional::notes(
        &commits,
        dx_update::conventional::DEFAULT_GROUPS,
    ))
}

/// Returns the asset names, rejecting two assets that share one.
fn github_asset_bases(srcs: &[&std::path::PathBuf]) -> io::Result<Vec<String>> {
    let mut seen: std::collections::HashMap<String, &Path> = std::collections::HashMap::new();
    let mut bases = Vec::new();
    for src in srcs {
        let base = basename(src)?;
        if let Some(first) = seen.get(&base) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "assets {} and {} share the asset name '{base}'",
                    first.display(),
                    src.display()
                ),
            ));
        }
        seen.insert(base.clone(), src.as_path());
        bases.push(base);
    }
    Ok(bases)
}

/// Formats one `basename sha256` line per asset.
fn github_asset_lines(sorted: &[&std::path::PathBuf], bases: &[String]) -> io::Result<Vec<String>> {
    let mut lines = Vec::new();
    for (src, base) in sorted.iter().zip(bases) {
        lines.push(format!(
            "  asset: {base} sha256:{}",
            dx_digest::sha256_file_hex(src)?
        ));
    }
    Ok(lines)
}

/// Builds one GitHub release staging dir.
pub fn github_build_staging(
    asset_srcs: &[std::path::PathBuf],
    outdir: &Path,
    deploy_name: &str,
    tag: &str,
) -> io::Result<std::path::PathBuf> {
    github_build_staging_with_notes(asset_srcs, outdir, deploy_name, tag, None)
}

/// Builds one GitHub release staging dir that also holds the release body.
pub fn github_build_staging_with_notes(
    asset_srcs: &[std::path::PathBuf],
    outdir: &Path,
    deploy_name: &str,
    tag: &str,
    notes: Option<&str>,
) -> io::Result<std::path::PathBuf> {
    let mut sorted: Vec<&std::path::PathBuf> = asset_srcs.iter().collect();
    sorted.sort();
    let bases = github_asset_bases(&sorted)?;
    let staging = outdir.join(deploy_name.to_owned() + "-release");
    std::fs::create_dir_all(&staging)?;
    let mut lines = vec![String::from("# would-run manifest for github_deploy")];
    for (src, base) in sorted.iter().zip(&bases) {
        let dest = staging.join(base);
        let digest = copy_verified(src, &dest)?;
        lines.push(format!("asset-sha256: {digest}  {base}"));
    }
    if let Some(notes) = notes {
        std::fs::write(staging.join("notes.md"), notes.as_bytes())?;
    }
    lines.push(release_command(tag, &bases, notes.is_some()));
    std::fs::write(staging.join("would-run.txt"), lines.join("\n").as_bytes())?;
    Ok(staging)
}

/// Owner approval gate for github_deploy.
pub(crate) const GITHUB_GATE: OwnerGate = OwnerGate {
    tool: "github_deploy",
    action: "publish",
    credentials: &[],
    approved: "GITHUB_PUBLISH_APPROVED",
};

/// Extra env names `gh` keeps: its auth, host, config and session bus.
const GH_CHILD_ENV_EXTRA: &[&str] = &[
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
    "GH_HOST",
    "GH_REPO",
    "GH_CONFIG_DIR",
    "GH_PROMPT_DISABLED",
    "XDG_CONFIG_HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// The jq program for `gh release view`: the tag, then each asset's name and size, tab-separated.
const PREVIOUS_RELEASE_JQ: &str =
    r#""tag\t\(.tagName)", (.assets[] | "asset\t\(.name)\t\(.size)")"#;

/// Reads the latest published release with `gh release view`: its tag and each asset's size.
fn previous_release(
    workspace: Option<std::ffi::OsString>,
) -> Result<(String, Vec<(String, u64)>), String> {
    let mut cmd = scrubbed_command(
        Path::new("gh"),
        &[PUBLISH_CHILD_ENV_KEEP, GH_CHILD_ENV_EXTRA],
        &[],
    );
    cmd.args([
        "release",
        "view",
        "--json",
        "tagName,assets",
        "--jq",
        PREVIOUS_RELEASE_JQ,
    ]);
    if let Some(dir) = workspace {
        cmd.current_dir(dir);
    }
    let out = run_publish(&mut cmd).map_err(|error| error.to_string())?;
    if !out.success() {
        return Err(format!("gh release view failed with {}", exit_text(&out)));
    }
    let mut tag = None;
    let mut assets = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        match line.split('\t').collect::<Vec<_>>().as_slice() {
            ["tag", value] => tag = Some((*value).to_owned()),
            ["asset", name, size] => {
                let size = size
                    .parse()
                    .map_err(|error| format!("asset {name} size: {error}"))?;
                assets.push(((*name).to_owned(), size));
            }
            _ => return Err(format!("unexpected gh output: {line}")),
        }
    }
    let tag = tag.ok_or_else(|| "gh returned no release".to_owned())?;
    Ok((tag, assets))
}

/// Describes each asset's size in the latest published release, for a dry run.
///
/// Says why the release is unavailable instead of failing, so a dry run works offline.
fn previous_release_lines(bases: &[String]) -> Vec<String> {
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty());
    let (tag, assets) = match previous_release(workspace) {
        Ok(found) => found,
        Err(reason) => return vec![format!("  previous release: unavailable ({reason})")],
    };
    let mut lines = vec![format!("  previous release: {tag}")];
    for base in bases {
        match assets.iter().find(|(name, _)| name == base) {
            Some((_, size)) => {
                lines.push(format!("  previous asset: {base} {size} bytes in {tag}"))
            }
            None => lines.push(format!("  previous asset: {base} not in {tag}")),
        }
    }
    lines
}

/// Describes each `.spdx.json` asset with its package count, for a dry run.
fn sbom_lines(sorted: &[&std::path::PathBuf], bases: &[String]) -> Vec<String> {
    let mut lines = Vec::new();
    for (src, base) in sorted.iter().zip(bases) {
        if !base.ends_with(".spdx.json") {
            continue;
        }
        let count = dx_sbom::spdx_package_count(src);
        match count {
            Some(count) => lines.push(format!("  sbom: {base} {count} packages")),
            None => lines.push(format!("  sbom: {base} is not a readable SPDX document")),
        }
    }
    lines
}

/// Runs the GitHub deploy tool.
pub fn github_main(asset_rlocs: &[String], deploy_name: &str, tag: &str, argv: &[String]) -> i32 {
    github_main_with_notes(asset_rlocs, deploy_name, tag, &Notes::None, argv)
}

/// Runs the GitHub deploy tool with a release body.
pub fn github_main_with_notes(
    asset_rlocs: &[String],
    deploy_name: &str,
    tag: &str,
    notes: &Notes,
    argv: &[String],
) -> i32 {
    let go_api = GoApi {
        mode: "off",
        baseline: "registry",
        apidiff_rloc: "",
        go_rloc: "",
        modules: &[],
    };
    github_main_checked(asset_rlocs, deploy_name, tag, notes, &go_api, argv)
}

/// The Go API compatibility settings of one github_deploy.
pub struct GoApi<'a> {
    pub mode: &'a str,
    pub baseline: &'a str,
    pub apidiff_rloc: &'a str,
    pub go_rloc: &'a str,
    pub modules: &'a [String],
}

/// Runs the GitHub deploy tool with a release body and a Go API compatibility check.
pub fn github_main_checked(
    asset_rlocs: &[String],
    deploy_name: &str,
    tag: &str,
    notes: &Notes,
    go_api: &GoApi,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "github_deploy",
        "the release is exactly the artifacts pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let mut assets = Vec::new();
    for rloc in asset_rlocs {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        assets.push(path);
    }
    let mut sorted: Vec<&std::path::PathBuf> = assets.iter().collect();
    sorted.sort();
    let bases = match github_asset_bases(&sorted) {
        Ok(bases) => bases,
        Err(error) => {
            eprintln!("github_deploy: {error}");
            return 1;
        }
    };
    let workspace = std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty());
    let live = GITHUB_GATE.requested(env_or_empty);
    let resolved = resolve_tag(tag, workspace.as_ref());
    let tag_text = match &resolved {
        Ok(tag) => tag.as_str(),
        Err(_) if !live => tag,
        Err(error) => {
            eprintln!("github_deploy: {error}");
            return 1;
        }
    };
    let notes_text: Result<Option<String>, String> = match notes {
        Notes::None => Ok(None),
        Notes::Commits => resolved
            .clone()
            .and_then(|tag| commit_notes(&tag, workspace.as_ref()))
            .map(Some),
        Notes::File(rloc) => match run.resolve(rloc) {
            Some(path) => std::fs::read_to_string(&path)
                .map(Some)
                .map_err(|error| format!("cannot read notes {}: {error}", path.display())),
            None => return 1,
        },
    };
    let mode = ApiCompatMode::parse(go_api.mode);
    let (Some(mode), Some(baseline)) = (mode, Baseline::parse(go_api.baseline)) else {
        eprintln!(
            "github_deploy: invalid api_compat '{}' or api_baseline '{}'",
            go_api.mode, go_api.baseline
        );
        return 1;
    };
    if mode != ApiCompatMode::Off {
        let root = workspace
            .clone()
            .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from);
        let outcomes = if go_api.apidiff_rloc.is_empty() || go_api.go_rloc.is_empty() {
            Ok(vec![Outcome::Skipped(
                "apidiff or go is not available".to_owned(),
            )])
        } else {
            let (Some(apidiff), Some(go)) = (
                run.resolve(go_api.apidiff_rloc),
                run.resolve(go_api.go_rloc),
            ) else {
                return 1;
            };
            crate::go_api::go_api_check(
                (&apidiff, Some(&go)),
                &root,
                go_api.modules,
                tag_text,
                baseline,
            )
        };
        let concluded = outcomes.and_then(|outcomes| {
            let results: Vec<_> = outcomes
                .into_iter()
                .map(|outcome| conclude("github_deploy", mode, tag_text, outcome))
                .collect();
            results.into_iter().collect::<Result<(), String>>()
        });
        if let Err(message) = concluded {
            eprintln!("{message}");
            return 1;
        }
    }
    if live {
        let tag = tag_text;
        let notes_text = match notes_text {
            Ok(text) => text,
            Err(error) => {
                eprintln!("github_deploy: {error}");
                return 1;
            }
        };
        if let Some(error) = placeholder_refusal("github_deploy", tag) {
            eprintln!("{error}");
            return 1;
        }
        if let Some(error) = GITHUB_GATE.refusal(env_or_empty) {
            eprintln!("{error}");
            return 1;
        }
        if which_on_path("gh").is_none() {
            eprintln!("github_deploy: 'gh' CLI not found on PATH");
            return 1;
        }
        let asset_lines = match github_asset_lines(&sorted, &bases) {
            Ok(lines) => lines,
            Err(error) => {
                eprintln!("github_deploy: {error}");
                return 1;
            }
        };
        let mut cmd = scrubbed_command(
            Path::new("gh"),
            &[PUBLISH_CHILD_ENV_KEEP, GH_CHILD_ENV_EXTRA],
            &[],
        );
        cmd.arg("release").arg("create").arg(tag);
        for src in &assets {
            match std::path::absolute(src) {
                Ok(path) => cmd.arg(path),
                Err(error) => {
                    eprintln!("github_deploy: cannot resolve {}: {error}", src.display());
                    return 1;
                }
            };
        }
        cmd.arg("--draft").arg("--verify-tag");
        if let Some(text) = &notes_text {
            let file = outdir.join("notes.md");
            if let Err(error) =
                std::fs::create_dir_all(outdir).and_then(|()| std::fs::write(&file, text))
            {
                eprintln!("github_deploy: cannot write {}: {error}", file.display());
                return 1;
            }
            cmd.arg("--notes-file")
                .arg(std::path::absolute(&file).unwrap_or(file));
        }
        if let Some(workspace) = workspace {
            cmd.current_dir(workspace);
        }
        match run_publish(&mut cmd) {
            Ok(out) if out.success() => {
                print!("{}", out.stdout_text());
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
                println!("github_deploy: published draft {tag}");
                for line in asset_lines {
                    println!("{line}");
                }
                return 0;
            }
            Ok(out) => {
                print!("{}", out.stdout_text());
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
                eprintln!("github_deploy: gh failed with {}", exit_text(&out));
                return 1;
            }
            Err(error) => {
                eprintln!("github_deploy: {error}");
                return 1;
            }
        }
    }
    let attached = notes_text.as_ref().ok().and_then(|text| text.as_deref());
    match github_build_staging_with_notes(&assets, outdir, deploy_name, tag_text, attached) {
        Ok(staging) => {
            println!("github_deploy: staged draft release, publishing nothing:");
            println!("  tag: {tag_text}");
            for (src, base) in sorted.iter().zip(&bases) {
                println!("  asset: {base} ({})", src.display());
            }
            for line in sbom_lines(&sorted, &bases) {
                println!("{line}");
            }
            println!(
                "  command: {}",
                release_command(tag_text, &bases, attached.is_some())
            );
            match &notes_text {
                Ok(Some(text)) => {
                    println!("  notes:");
                    for line in text.lines() {
                        println!("    {line}");
                    }
                }
                Ok(None) => {}
                Err(reason) => println!("  notes: unavailable ({reason})"),
            }
            println!("  staging: {}", staging.display());
            println!("  manifest: {}", staging.join("would-run.txt").display());
            for line in previous_release_lines(&bases) {
                println!("{line}");
            }
            0
        }
        Err(error) => {
            eprintln!("github_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn github_staging_writes_manifest() {
        let scratch = scratch_dir();
        let asset = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&asset, b"asset-bytes-v1").expect("write asset");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let staging = github_build_staging(
            std::slice::from_ref(&asset),
            &outdir,
            "github_demo",
            "v0.0.0-dryrun",
        )
        .expect("staging");
        assert!(staging.join("release_demo.tar.gz").is_file());
        assert!(staging.join("would-run.txt").is_file());
        assert!(github_release_command("v1", &["a".to_owned()]).contains("gh release create v1 a"));
    }

    #[cfg(unix)]
    #[test]
    fn breaking_go_api_change_stops_a_live_release_before_gh_runs() {
        if env_test_in_child("github::tests::breaking_go_api_change_stops_a_live_release_before_gh_runs")
        {
            return;
        }
        let scratch = scratch_dir();
        let repo = scratch.path().join("repo");
        let runfiles = scratch.path().join("runfiles");
        let bin = scratch.path().join("bin");
        write_crate_file(&repo, "go.mod", "module example.com/lib\n\ngo 1.22\n");
        write_crate_file(&repo, "lib.go", "package lib\n");
        for dir in [&runfiles, &bin] {
            std::fs::create_dir_all(dir).expect("dir");
        }
        for args in [
            vec!["init", "-q"],
            vec!["add", "."],
            vec![
                "-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "x",
            ],
            vec!["tag", "v1.2.0"],
        ] {
            dx_process::capture::run_checked(
                dx_process::capture::git(&repo).args(&args),
                dx_process::capture::DEFAULT_TIMEOUT,
            )
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        }
        std::fs::write(runfiles.join("asset.tar.gz"), b"asset").expect("asset");
        let marker = scratch.path().join("apidiff-ran");
        let apidiff = runfiles.join("apidiff");
        std::fs::write(
            &apidiff,
            format!(
                "#!/bin/sh\ntouch {}\ncase \"$*\" in *-w*) exit 0;; esac\necho 'Incompatible changes:'\necho '- Hello: removed'\n",
                marker.display()
            ),
        )
        .expect("fake apidiff");
        set_mode(&apidiff, 0o755);
        std::fs::write(runfiles.join("go"), "#!/bin/sh\nexit 0\n").expect("fake go");
        set_mode(&runfiles.join("go"), 0o755);
        let gh_log = bin.join("gh.log");
        std::fs::write(
            bin.join("gh"),
            format!("#!/bin/sh\ntouch {}\n", gh_log.display()),
        )
        .expect("fake gh");
        set_mode(&bin.join("gh"), 0o755);
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let outdir = tempfile_stage_dir("dx-deploy-gh-").expect("outdir");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let repo_dir = repo.display().to_string();
        let go_api = GoApi {
            mode: "fail",
            baseline: "git",
            apidiff_rloc: "apidiff",
            go_rloc: "go",
            modules: &[],
        };
        let code = with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("BUILD_WORKSPACE_DIRECTORY", Some(repo_dir.as_str())),
                ("PATH", Some(path.as_str())),
                ("DX_DEPLOY_LIVE", Some("1")),
                ("GITHUB_TOKEN", Some("fake")),
                ("GH_TOKEN", Some("fake")),
                ("GITHUB_PUBLISH_APPROVED", Some("1")),
            ],
            || {
                github_main_checked(
                    &["asset.tar.gz".to_owned()],
                    "demo",
                    "v1.3.0",
                    &Notes::None,
                    &go_api,
                    &argv,
                )
            },
        );
        assert_eq!(code, 1);
        assert!(marker.exists(), "the live run skipped the check");
        assert!(!gh_log.exists(), "gh ran after a breaking change");
    }

    #[test]
    fn github_staging_without_artifacts_holds_only_the_manifest() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let staging = github_build_staging(&[], &outdir, "github_draft", "v1").expect("staging");
        let names: Vec<_> = std::fs::read_dir(&staging)
            .expect("staging dir")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, ["would-run.txt"]);
        let manifest = std::fs::read_to_string(staging.join("would-run.txt")).expect("manifest");
        assert!(manifest.ends_with("gh release create v1 --draft --verify-tag"));
    }

    #[test]
    fn github_staging_rejects_duplicate_basename() {
        let scratch = scratch_dir();
        let first = scratch.path().join("a").join("app.tar.gz");
        let second = scratch.path().join("b").join("app.tar.gz");
        for (path, bytes) in [(&first, b"linux-bytes"), (&second, b"macos-bytes")] {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("asset dir");
            std::fs::write(path, bytes).expect("write asset");
        }
        let outdir = scratch.path().join("out");
        let error = github_build_staging(
            &[second.clone(), first.clone()],
            &outdir,
            "github_dup",
            "v0.0.0-dryrun",
        )
        .expect_err("duplicate basename must fail");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        let message = error.to_string();
        assert!(message.contains(&first.display().to_string()), "{message}");
        assert!(message.contains(&second.display().to_string()), "{message}");
        assert!(
            message.contains("share the asset name 'app.tar.gz'"),
            "{message}"
        );
        assert!(!outdir.join("github_dup-release").exists());
    }

    #[cfg(unix)]
    #[test]
    fn github_live_run_calls_gh_with_tag_and_absolute_assets() {
        if env_test_in_child("github::tests::github_live_run_calls_gh_with_tag_and_absolute_assets")
        {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        std::fs::write(runfiles.join("b.tar.gz"), b"bee").expect("write b");
        std::fs::write(runfiles.join("a.tar.gz"), b"ay").expect("write a");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let log = scratch.path().join("gh.log");
        let gh = bin.join("gh");
        dx_testing::executable_script(
            &gh,
            &format!(
                "#!/bin/sh\nfor a in \"$@\"; do echo \"$a\" >> {}; done\n",
                log.display()
            ),
        );
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let rlocs = ["b.tar.gz".to_owned(), "a.tar.gz".to_owned()];
        let code = with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("PATH", Some(bin.to_string_lossy().as_ref())),
                ("DX_DEPLOY_LIVE", Some("1")),
                ("GITHUB_PUBLISH_APPROVED", Some("1")),
            ],
            || github_main(&rlocs, "demo", "v1.2.3", &argv),
        );
        assert_eq!(code, 0);
        let logged = std::fs::read_to_string(&log).expect("gh log");
        let args: Vec<&str> = logged.lines().collect();
        let a = runfiles.join("a.tar.gz").display().to_string();
        let b = runfiles.join("b.tar.gz").display().to_string();
        let want = [
            "release",
            "create",
            "v1.2.3",
            &b,
            &a,
            "--draft",
            "--verify-tag",
        ];
        assert_eq!(args, want);
    }

    #[test]
    fn github_asset_lines_name_each_asset_with_its_sha256() {
        let scratch = scratch_dir();
        let a = scratch.path().join("a.tar.gz");
        std::fs::write(&a, b"ay").expect("write a");
        let lines = github_asset_lines(&[&a], &["a.tar.gz".to_owned()]).expect("lines");
        let digest = dx_digest::sha256_file_hex(&a).expect("digest");
        assert_eq!(lines, [format!("  asset: a.tar.gz sha256:{digest}")]);
    }

    #[test]
    fn sbom_lines_count_packages_of_spdx_assets_only() {
        let scratch = scratch_dir();
        let sbom = scratch.path().join("a.spdx.json");
        let bad = scratch.path().join("b.spdx.json");
        let other = scratch.path().join("c.tar.gz");
        std::fs::write(&sbom, br#"{"packages":[{"name":"a"},{"name":"b"}]}"#).expect("write");
        std::fs::write(&bad, b"nope").expect("write");
        std::fs::write(&other, b"x").expect("write");
        let bases = ["a.spdx.json", "b.spdx.json", "c.tar.gz"].map(str::to_owned);
        let lines = sbom_lines(&[&sbom, &bad, &other], &bases);
        assert_eq!(
            lines,
            [
                "  sbom: a.spdx.json 2 packages",
                "  sbom: b.spdx.json is not a readable SPDX document",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn github_dry_run_reports_previous_release_asset_sizes() {
        if env_test_in_child("github::tests::github_dry_run_reports_previous_release_asset_sizes") {
            return;
        }
        let scratch = scratch_dir();
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let gh = bin.join("gh");
        dx_testing::executable_script(
            &gh,
            "#!/bin/sh\nprintf 'tag\\tv1.2.2\\nasset\\tapp.tar.gz\\t1234\\n'\n",
        );
        let bases = ["app.tar.gz".to_owned(), "new.tar.gz".to_owned()];
        let lines = with_env(
            &[
                ("PATH", Some(bin.to_string_lossy().as_ref())),
                ("BUILD_WORKSPACE_DIRECTORY", None),
            ],
            || previous_release_lines(&bases),
        );
        assert_eq!(
            lines,
            [
                "  previous release: v1.2.2",
                "  previous asset: app.tar.gz 1234 bytes in v1.2.2",
                "  previous asset: new.tar.gz not in v1.2.2",
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn github_dry_run_says_when_previous_release_is_unavailable() {
        if env_test_in_child(
            "github::tests::github_dry_run_says_when_previous_release_is_unavailable",
        ) {
            return;
        }
        let scratch = scratch_dir();
        let empty = scratch.path().join("empty");
        std::fs::create_dir_all(&empty).expect("empty");
        let lines = with_env(
            &[
                ("PATH", Some(empty.to_string_lossy().as_ref())),
                ("BUILD_WORKSPACE_DIRECTORY", None),
            ],
            || previous_release_lines(&["app.tar.gz".to_owned()]),
        );
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].starts_with("  previous release: unavailable ("),
            "{lines:?}"
        );
    }

    #[cfg(unix)]
    fn fake_git(bin: &Path) {
        std::fs::create_dir_all(bin).expect("bin");
        let git = bin.join("git");
        let script = r#"#!/bin/sh
case "$1" in
  tag) echo v1.1.0 ;;
  describe) echo v1.0.0 ;;
  log) printf 'aaaaaaa1\0feat(cli): add init\0\036bbbbbbb2\0fix: stop crash\0\036ccccccc3\0fix: second\0\036' ;;
esac
"#;
        dx_testing::executable_script(&git, &script);
    }

    #[cfg(unix)]
    #[test]
    fn current_tag_and_commit_notes_come_from_git() {
        if env_test_in_child("github::tests::current_tag_and_commit_notes_come_from_git") {
            return;
        }
        let scratch = scratch_dir();
        let bin = scratch.path().join("bin");
        fake_git(&bin);
        let (tag, notes) = with_env(&[("PATH", Some(bin.to_string_lossy().as_ref()))], || {
            let tag = resolve_tag("current", None).expect("tag");
            (tag.clone(), commit_notes(&tag, None).expect("notes"))
        });
        assert_eq!(tag, "v1.1.0");
        assert!(
            notes.contains("## Features\n\n- **cli**: add init (aaaaaaa)"),
            "{notes}"
        );
        assert!(
            notes.contains("## Fixes\n\n- stop crash (bbbbbbb)\n- second (ccccccc)"),
            "{notes}"
        );
        assert_eq!(resolve_tag("v2", None), Ok("v2".to_owned()));
    }

    #[cfg(unix)]
    #[test]
    fn github_dry_run_stages_the_commit_notes() {
        if env_test_in_child("github::tests::github_dry_run_stages_the_commit_notes") {
            return;
        }
        let scratch = scratch_dir();
        let bin = scratch.path().join("bin");
        fake_git(&bin);
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let code = with_env(
            &[
                ("PATH", Some(bin.to_string_lossy().as_ref())),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("DX_DEPLOY_LIVE", None),
                ("GITHUB_PUBLISH_APPROVED", None),
            ],
            || github_main_with_notes(&[], "demo", "current", &Notes::Commits, &argv),
        );
        assert_eq!(code, 0);
        let staging = outdir.join("demo-release");
        let notes = std::fs::read_to_string(staging.join("notes.md")).expect("notes");
        assert!(notes.contains("## Features"), "{notes}");
        assert!(notes.contains("## Fixes"), "{notes}");
        let manifest = std::fs::read_to_string(staging.join("would-run.txt")).expect("manifest");
        assert!(
            manifest
                .ends_with("gh release create v1.1.0 --draft --verify-tag --notes-file notes.md"),
            "{manifest}"
        );
    }
}
