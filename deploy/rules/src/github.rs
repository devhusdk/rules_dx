//! GitHub release publisher: asset staging and the gh release upload.

use crate::archive::*;
use crate::run::{
    env_or_empty, placeholder_refusal, scrubbed_command, DeployRun, OwnerGate,
    PUBLISH_CHILD_ENV_KEEP,
};
use crate::stage::*;
use std::io;
use std::path::Path;

/// Formats one GitHub release command.
pub fn github_release_command(tag: &str, asset_bases: &[String]) -> String {
    let mut cmd = format!("gh release create {tag}");
    for base in asset_bases {
        cmd.push_str(&format!(" {base}"));
    }
    cmd.push_str(" --draft --verify-tag");
    cmd
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
    lines.push(github_release_command(tag, &bases));
    std::fs::write(staging.join("would-run.txt"), lines.join("\n").as_bytes())?;
    Ok(staging)
}

/// Owner approval gate for github_deploy.
pub(crate) const GITHUB_GATE: OwnerGate = OwnerGate {
    tool: "github_deploy",
    action: "publish",
    credentials: &[],
    approved: "GH_RELEASE_APPROVED",
    live: "GH_RELEASE_LIVE",
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

/// Runs the GitHub deploy tool.
pub fn github_main(asset_rlocs: &[String], deploy_name: &str, tag: &str, argv: &[String]) -> i32 {
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
    if GITHUB_GATE.requested(env_or_empty) {
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
        if let Some(workspace) =
            std::env::var_os("BUILD_WORKSPACE_DIRECTORY").filter(|dir| !dir.is_empty())
        {
            cmd.current_dir(workspace);
        }
        match cmd.status() {
            Ok(status) if status.success() => {
                println!("github_deploy: published draft {tag}");
                for line in asset_lines {
                    println!("{line}");
                }
                return 0;
            }
            Ok(status) => {
                eprintln!("github_deploy: gh failed with {status}");
                return 1;
            }
            Err(error) => {
                eprintln!("github_deploy: {error}");
                return 1;
            }
        }
    }
    match github_build_staging(&assets, outdir, deploy_name, tag) {
        Ok(staging) => {
            println!("github_deploy: staged draft release, publishing nothing:");
            println!("  tag: {tag}");
            for (src, base) in sorted.iter().zip(&bases) {
                println!("  asset: {base} ({})", src.display());
            }
            println!("  command: {}", github_release_command(tag, &bases));
            println!("  staging: {}", staging.display());
            println!("  manifest: {}", staging.join("would-run.txt").display());
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
        use std::os::unix::fs::PermissionsExt as _;
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
        std::fs::write(
            &gh,
            format!(
                "#!/bin/sh\nfor a in \"$@\"; do echo \"$a\" >> {}; done\n",
                log.display()
            ),
        )
        .expect("write gh");
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let rlocs = ["b.tar.gz".to_owned(), "a.tar.gz".to_owned()];
        let code = with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("BUILD_WORKSPACE_DIRECTORY", None),
                ("PATH", Some(bin.to_string_lossy().as_ref())),
                ("GH_RELEASE_LIVE", Some("1")),
                ("GH_RELEASE_APPROVED", Some("1")),
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
}
