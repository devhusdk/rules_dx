//! Homebrew tap publisher: renders a formula from a GitHub release and pushes it to a tap.

use crate::archive::*;
use crate::config::GithubConfig;
use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, env_or_empty, exit_text, pages_redact,
    placeholder_refusal, run_publish, scrubbed_command,
};
use crate::stage::*;
use base64::Engine as _;
use std::io;
use std::path::{Path, PathBuf};

/// Owner approval gate for homebrew_tap_deploy.
pub(crate) const HOMEBREW_TAP_GATE: OwnerGate = OwnerGate {
    tool: "homebrew_tap_deploy",
    action: "publish",
    credentials: &["HOMEBREW_TAP_API_TOKEN"],
    approved: "HOMEBREW_TAP_PUBLISH_APPROVED",
};

/// Env names `gh` keeps besides the token the gate read.
const GH_CHILD_ENV_EXTRA: &[&str] = &[
    "GH_HOST",
    "GH_CONFIG_DIR",
    "GH_PROMPT_DISABLED",
    "XDG_CONFIG_HOME",
];

/// Names what the formula installs and where it is published.
pub struct HomebrewLaunch<'a> {
    pub release_rloc: &'a str,
    pub deploy_name: &'a str,
    pub repo: &'a str,
    pub tap: &'a str,
    pub formula: &'a str,
    pub binary: &'a str,
    pub desc: &'a str,
    pub homepage: &'a str,
}

/// One release archive with the platform it is built for and its digest.
#[derive(Debug, PartialEq, Eq)]
pub struct FormulaAsset {
    pub os: &'static str,
    pub arch: &'static str,
    pub name: String,
    pub sha256: String,
}

/// Returns the os and arch a release archive name says it is built for.
pub fn homebrew_asset_platform(base: &str) -> Option<(&'static str, &'static str)> {
    let lower = base.to_ascii_lowercase();
    if !(lower.ends_with(".tar.gz") || lower.ends_with(".tgz") || lower.ends_with(".zip")) {
        return None;
    }
    let has = |words: &[&str]| words.iter().any(|word| lower.contains(word));
    let os = if has(&["darwin", "macos", "apple"]) {
        "macos"
    } else if has(&["linux"]) {
        "linux"
    } else {
        return None;
    };
    let arch = if has(&["arm64", "aarch64"]) {
        "arm"
    } else if has(&["x86_64", "amd64", "x64"]) {
        "intel"
    } else {
        return None;
    };
    Some((os, arch))
}

/// Returns the Ruby class name Homebrew expects for a formula name.
pub fn homebrew_class_name(formula: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in formula.chars() {
        if c == '-' || c == '_' {
            upper = true;
        } else if upper {
            out.push(c.to_ascii_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Escapes a value for a double-quoted Ruby string.
fn ruby_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\\' | '"' | '#' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Renders the formula for the given release archives.
pub fn homebrew_render_formula(
    launch: &HomebrewLaunch<'_>,
    tag: &str,
    assets: &[FormulaAsset],
) -> String {
    let version = tag.strip_prefix('v').unwrap_or(tag);
    let mut out = format!(
        "class {} < Formula\n  desc \"{}\"\n  homepage \"{}\"\n  version \"{version}\"\n",
        homebrew_class_name(launch.formula),
        ruby_escape(launch.desc),
        ruby_escape(launch.homepage),
    );
    for os in ["macos", "linux"] {
        let in_os: Vec<&FormulaAsset> = assets.iter().filter(|asset| asset.os == os).collect();
        if in_os.is_empty() {
            continue;
        }
        out.push_str(&format!("\n  on_{os} do\n"));
        for arch in ["arm", "intel"] {
            for asset in in_os.iter().filter(|asset| asset.arch == arch) {
                out.push_str(&format!(
                    "    on_{arch} do\n      url \"https://github.com/{}/releases/download/{tag}/{}\"\n      sha256 \"{}\"\n    end\n",
                    launch.repo, asset.name, asset.sha256
                ));
            }
        }
        out.push_str("  end\n");
    }
    out.push_str(&format!(
        "\n  def install\n    bin.install \"{}\"\n  end\nend\n",
        ruby_escape(launch.binary)
    ));
    out
}

/// Picks the platform archives from the release assets and checks them against any `.sha256` file.
pub fn homebrew_collect_assets(assets: &[PathBuf]) -> io::Result<Vec<FormulaAsset>> {
    let invalid = |message: String| io::Error::new(io::ErrorKind::InvalidInput, message);
    let mut found: Vec<FormulaAsset> = Vec::new();
    for path in assets {
        let base = basename(path)?;
        let Some((os, arch)) = homebrew_asset_platform(&base) else {
            continue;
        };
        if let Some(first) = found.iter().find(|a| a.os == os && a.arch == arch) {
            return Err(invalid(format!(
                "assets {} and {base} are both {os} {arch}",
                first.name
            )));
        }
        let sha256 = dx_digest::sha256_file_hex(path)?;
        let listed = path.with_file_name(format!("{base}.sha256"));
        if listed.is_file() {
            let text = std::fs::read_to_string(&listed)?;
            if text.split_whitespace().next() != Some(sha256.as_str()) {
                return Err(invalid(format!(
                    "{base} does not match its checksum file {}",
                    listed.display()
                )));
            }
        }
        found.push(FormulaAsset {
            os,
            arch,
            name: base,
            sha256,
        });
    }
    if found.is_empty() {
        return Err(invalid(
            "the release has no .tar.gz or .zip asset named for a platform (darwin or linux, arm64 or x86_64)"
                .to_owned(),
        ));
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

/// Writes the formula under `<outdir>/<name>-tap/Formula` and the would-run manifest.
pub fn homebrew_build_staging(
    launch: &HomebrewLaunch<'_>,
    formula_text: &str,
    outdir: &Path,
) -> io::Result<PathBuf> {
    let staging = outdir.join(format!("{}-tap", launch.deploy_name));
    let formula_dir = staging.join("Formula");
    std::fs::create_dir_all(&formula_dir)?;
    std::fs::write(
        formula_dir.join(format!("{}.rb", launch.formula)),
        formula_text.as_bytes(),
    )?;
    let manifest = format!(
        "# would-run manifest for homebrew_tap_deploy\n{}\n",
        homebrew_api_command(launch)
    );
    std::fs::write(staging.join("would-run.txt"), manifest.as_bytes())?;
    Ok(staging)
}

fn homebrew_formula_path(launch: &HomebrewLaunch<'_>) -> String {
    format!(
        "repos/{}/contents/Formula/{}.rb",
        launch.tap, launch.formula
    )
}

/// Formats the GitHub API call that writes the formula.
pub fn homebrew_api_command(launch: &HomebrewLaunch<'_>) -> String {
    format!(
        "gh api -X PUT {} -f message=... -f content=<base64 formula>",
        homebrew_formula_path(launch)
    )
}

fn homebrew_commit_message(launch: &HomebrewLaunch<'_>, tag: &str) -> String {
    format!("{} {tag}", launch.formula)
}

/// Pushes the formula to the tap through `gh api`, replacing the file when it exists.
fn homebrew_push(launch: &HomebrewLaunch<'_>, tag: &str, formula: &str, token: &str) -> i32 {
    let gh = |args: &[&str]| {
        let mut cmd = scrubbed_command(
            Path::new("gh"),
            &[PUBLISH_CHILD_ENV_KEEP, GH_CHILD_ENV_EXTRA],
            &[("GH_TOKEN", token)],
        );
        cmd.arg("api").args(args);
        run_publish(&mut cmd)
    };
    let path = homebrew_formula_path(launch);
    let existing = match gh(&[&path, "--jq", ".sha"]) {
        Ok(out) if out.success() => {
            Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        }
        Ok(out) if String::from_utf8_lossy(&out.stderr).contains("(HTTP 404)") => None,
        Ok(out) => {
            eprintln!(
                "homebrew_tap_deploy: gh failed with {}: {}",
                exit_text(&out),
                pages_redact(&String::from_utf8_lossy(&out.stderr), token).trim_end()
            );
            return 1;
        }
        Err(error) => {
            eprintln!("homebrew_tap_deploy: {error}");
            return 1;
        }
    };
    let content = base64::engine::general_purpose::STANDARD.encode(formula);
    let message = format!("message={}", homebrew_commit_message(launch, tag));
    let content_field = format!("content={content}");
    let mut args = vec![
        "-X",
        "PUT",
        path.as_str(),
        "-f",
        message.as_str(),
        "-f",
        content_field.as_str(),
    ];
    let sha_field = existing.map(|sha| format!("sha={sha}"));
    if let Some(field) = &sha_field {
        args.extend(["-f", field.as_str()]);
    }
    match gh(&args) {
        Ok(out) if out.success() => {
            println!(
                "homebrew_tap_deploy: pushed Formula/{}.rb to {}",
                launch.formula, launch.tap
            );
            0
        }
        Ok(out) => {
            eprintln!(
                "homebrew_tap_deploy: gh failed with {}: {}",
                exit_text(&out),
                pages_redact(&String::from_utf8_lossy(&out.stderr), token).trim_end()
            );
            1
        }
        Err(error) => {
            eprintln!("homebrew_tap_deploy: {error}");
            1
        }
    }
}

/// Runs `ruby -c` on the staged formula when ruby is on PATH.
fn homebrew_syntax_check(formula: &Path) -> bool {
    if which_on_path("ruby").is_none() {
        println!("  syntax: skipped, ruby not on PATH");
        return true;
    }
    let mut cmd = scrubbed_command(Path::new("ruby"), &[PUBLISH_CHILD_ENV_KEEP], &[]);
    match run_publish(cmd.arg("-c").arg(formula)) {
        Ok(out) if out.success() => {
            println!("  syntax: ruby -c ok");
            true
        }
        Ok(out) => {
            eprint!("{}", String::from_utf8_lossy(&out.stderr));
            eprintln!("homebrew_tap_deploy: ruby -c failed with {}", exit_text(&out));
            false
        }
        Err(error) => {
            eprintln!("homebrew_tap_deploy: {error}");
            false
        }
    }
}

/// Runs the Homebrew tap deploy tool.
pub fn homebrew_tap_main(launch: &HomebrewLaunch<'_>, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "homebrew_tap_deploy",
        "the formula is rendered from the pinned release",
        argv,
    ) else {
        return 1;
    };
    let Some(release_path) = run.resolve(launch.release_rloc) else {
        return 1;
    };
    let release: GithubConfig = match std::fs::read_to_string(&release_path)
        .map_err(|error| error.to_string())
        .and_then(|text| {
            serde_json::from_str::<serde_json::Value>(&text).map_err(|error| error.to_string())
        })
        .and_then(|value| {
            serde_json::from_value(value["config"].clone()).map_err(|error| error.to_string())
        }) {
        Ok(release) => release,
        Err(error) => {
            eprintln!(
                "homebrew_tap_deploy: cannot read release config {}: {error}",
                release_path.display()
            );
            return 1;
        }
    };
    let mut assets = Vec::new();
    for rloc in &release.asset_rlocs {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        assets.push(path);
    }
    let formula_assets = match homebrew_collect_assets(&assets) {
        Ok(found) => found,
        Err(error) => {
            eprintln!("homebrew_tap_deploy: {error}");
            return 1;
        }
    };
    let formula = homebrew_render_formula(launch, &release.tag, &formula_assets);
    if HOMEBREW_TAP_GATE.requested(env_or_empty) {
        if let Some(error) = placeholder_refusal("homebrew_tap_deploy", &release.tag) {
            eprintln!("{error}");
            return 1;
        }
        let secrets = match HOMEBREW_TAP_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        if which_on_path("gh").is_none() {
            eprintln!("homebrew_tap_deploy: 'gh' CLI not found on PATH");
            return 1;
        }
        return homebrew_push(
            launch,
            &release.tag,
            &formula,
            secrets.get("HOMEBREW_TAP_API_TOKEN"),
        );
    }
    match homebrew_build_staging(launch, &formula, run.outdir()) {
        Ok(staging) => {
            let file = staging
                .join("Formula")
                .join(format!("{}.rb", launch.formula));
            println!("homebrew_tap_deploy: staged formula, pushing nothing:");
            println!("  tap: {}", launch.tap);
            println!("  tag: {}", release.tag);
            for asset in &formula_assets {
                println!("  asset: {} sha256:{}", asset.name, asset.sha256);
            }
            println!("  command: {}", homebrew_api_command(launch));
            println!("  formula: {}", file.display());
            if homebrew_syntax_check(&file) { 0 } else { 1 }
        }
        Err(error) => {
            eprintln!("homebrew_tap_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn launch() -> HomebrewLaunch<'static> {
        HomebrewLaunch {
            release_rloc: "release.json",
            deploy_name: "brew_demo",
            repo: "org/tool",
            tap: "org/homebrew-tap",
            formula: "my-tool",
            binary: "my-tool",
            desc: "Does things",
            homepage: "https://github.com/org/tool",
        }
    }

    #[test]
    fn render_escapes_ruby_metacharacters() {
        let mut l = launch();
        l.desc = "A \"fast\" #{x} tool\\";
        l.binary = "b\"#{y}";
        let out = homebrew_render_formula(&l, "v1", &[]);
        assert!(out.contains("desc \"A \\\"fast\\\" \\#{x} tool\\\\\""), "{out}");
        assert!(out.contains("bin.install \"b\\\"\\#{y}\""), "{out}");
    }

    #[test]
    fn platforms_come_from_archive_names() {
        assert_eq!(
            homebrew_asset_platform("t-aarch64-apple-darwin.tar.gz"),
            Some(("macos", "arm"))
        );
        assert_eq!(
            homebrew_asset_platform("t-linux-x86_64.tar.gz"),
            Some(("linux", "intel"))
        );
        assert_eq!(
            homebrew_asset_platform("t-linux-x86_64.tar.gz.sha256"),
            None
        );
        assert_eq!(homebrew_asset_platform("t-windows-x86_64.zip"), None);
    }

    #[test]
    fn class_names_follow_homebrew() {
        assert_eq!(homebrew_class_name("my-tool"), "MyTool");
        assert_eq!(homebrew_class_name("tool_2"), "Tool2");
    }

    #[test]
    fn formula_lists_each_platform_with_its_digest() {
        let assets = [
            FormulaAsset {
                os: "linux",
                arch: "intel",
                name: "t-linux-x86_64.tar.gz".to_owned(),
                sha256: "bb".to_owned(),
            },
            FormulaAsset {
                os: "macos",
                arch: "arm",
                name: "t-darwin-arm64.tar.gz".to_owned(),
                sha256: "aa".to_owned(),
            },
        ];
        let text = homebrew_render_formula(&launch(), "v1.2.3", &assets);
        let want = "class MyTool < Formula
  desc \"Does things\"
  homepage \"https://github.com/org/tool\"
  version \"1.2.3\"

  on_macos do
    on_arm do
      url \"https://github.com/org/tool/releases/download/v1.2.3/t-darwin-arm64.tar.gz\"
      sha256 \"aa\"
    end
  end

  on_linux do
    on_intel do
      url \"https://github.com/org/tool/releases/download/v1.2.3/t-linux-x86_64.tar.gz\"
      sha256 \"bb\"
    end
  end

  def install
    bin.install \"my-tool\"
  end
end
";
        assert_eq!(text, want);
    }

    #[test]
    fn collect_checks_the_listed_checksum() {
        let scratch = scratch_dir();
        let archive = scratch.path().join("t-darwin-arm64.tar.gz");
        std::fs::write(&archive, b"bytes").expect("write archive");
        let digest = dx_digest::sha256_file_hex(&archive).expect("digest");
        let sums = scratch.path().join("t-darwin-arm64.tar.gz.sha256");
        std::fs::write(&sums, format!("{digest}  t-darwin-arm64.tar.gz\n")).expect("write sums");
        let found = homebrew_collect_assets(&[archive.clone(), sums.clone()]).expect("collect");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].sha256, digest);
        std::fs::write(&sums, "00  t-darwin-arm64.tar.gz\n").expect("write bad sums");
        let error = homebrew_collect_assets(&[archive, sums]).expect_err("mismatch");
        assert!(error.to_string().contains("does not match"), "{error}");
    }

    #[test]
    fn collect_refuses_a_release_without_platform_archives() {
        let error = homebrew_collect_assets(&[]).expect_err("empty");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn live_run_pushes_the_formula_through_gh_api() {
        if env_test_in_child("homebrew::tests::live_run_pushes_the_formula_through_gh_api") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        std::fs::write(runfiles.join("t-darwin-arm64.tar.gz"), b"mac").expect("write asset");
        std::fs::write(
            runfiles.join("release.json"),
            r#"{"kind":"github","config":{"deploy_name":"r","tag":"v1.2.3","asset_rlocs":["t-darwin-arm64.tar.gz"],"api_compat":"off","api_baseline":"registry"}}"#,
        )
        .expect("write release");
        let bin = scratch.path().join("bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let log = scratch.path().join("gh.log");
        let gh = bin.join("gh");
        dx_testing::executable_script(
            &gh,
            &format!(
                "#!/bin/sh\necho \"$GH_TOKEN $*\" >> {}\n[ \"$3\" = PUT ] || {{ echo 'gh: Not Found (HTTP 404)' >&2; exit 1; }}\n",
                log.display()
            ),
        );
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let code = with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("PATH", Some(bin.to_string_lossy().as_ref())),
                ("DX_DEPLOY_LIVE", Some("1")),
                ("HOMEBREW_TAP_PUBLISH_APPROVED", Some("1")),
                ("HOMEBREW_TAP_API_TOKEN", Some("tok")),
            ],
            || homebrew_tap_main(&launch(), &argv),
        );
        assert_eq!(code, 0);
        let logged = std::fs::read_to_string(&log).expect("gh log");
        let lines: Vec<&str> = logged.lines().collect();
        assert_eq!(lines.len(), 2, "{logged}");
        assert_eq!(
            lines[0],
            "tok api repos/org/homebrew-tap/contents/Formula/my-tool.rb --jq .sha"
        );
        assert!(
            lines[1].starts_with(
                "tok api -X PUT repos/org/homebrew-tap/contents/Formula/my-tool.rb -f message=my-tool v1.2.3 -f content="
            ),
            "{logged}"
        );
        assert!(!lines[1].contains("sha="), "{logged}");
    }

    #[test]
    fn dry_run_writes_the_formula_into_the_output_tree() {
        if env_test_in_child("homebrew::tests::dry_run_writes_the_formula_into_the_output_tree") {
            return;
        }
        let scratch = scratch_dir();
        let runfiles = scratch.path().join("runfiles");
        std::fs::create_dir_all(&runfiles).expect("runfiles");
        std::fs::write(runfiles.join("t-linux-amd64.tar.gz"), b"lin").expect("write asset");
        std::fs::write(
            runfiles.join("release.json"),
            r#"{"kind":"github","config":{"deploy_name":"r","tag":"v1.2.3","asset_rlocs":["t-linux-amd64.tar.gz"],"api_compat":"off","api_baseline":"registry"}}"#,
        )
        .expect("write release");
        let outdir = scratch.path().join("out");
        let argv = vec!["prog".to_owned(), outdir.to_string_lossy().into_owned()];
        let code = with_env(
            &[
                ("RUNFILES_DIR", Some(runfiles.to_string_lossy().as_ref())),
                ("RUNFILES_MANIFEST_FILE", None),
                ("DX_DEPLOY_LIVE", None),
            ],
            || homebrew_tap_main(&launch(), &argv),
        );
        assert_eq!(code, 0);
        let text = std::fs::read_to_string(outdir.join("brew_demo-tap/Formula/my-tool.rb"))
            .expect("formula");
        assert!(text.contains("on_linux do"), "{text}");
        assert!(outdir.join("brew_demo-tap/would-run.txt").is_file());
    }
}
