//! Hex publisher: package staging and the mix or rebar3 build and publish.

use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty,
    placeholder_refusal, scrubbed_command,
};
use crate::stage::*;
use std::io;
use std::path::{Path, PathBuf};

/// Owner approval gate for hex_deploy.
pub(crate) const HEX_GATE: OwnerGate = OwnerGate {
    tool: "hex_deploy",
    action: "publish",
    credentials: &["HEX_API_KEY"],
    approved: "HEX_PUBLISH_APPROVED",
};

/// Extra env names mix and rebar3 keep beyond the publish base list.
const HEX_CHILD_ENV_EXTRA: &[&str] = &["ERL_LIBS", "ERL_ROOTDIR", "HEX_HOME", "MIX_HOME"];

/// Most tool output lines shown when a build or publish fails.
const HEX_FAILURE_LINES: usize = 40;

/// Returns the manifest file name that marks the package root for one tool.
fn hex_manifest(tool: &str) -> Result<&'static str, String> {
    match tool {
        "mix" => Ok("mix.exs"),
        "rebar3" => Ok("rebar.config"),
        _ => Err(format!(
            "hex_deploy: unknown tool {tool:?}: want mix or rebar3"
        )),
    }
}

/// Maps package input files to package-relative paths under the manifest directory.
pub fn hex_package_paths(
    package_files: &[PathBuf],
    tool: &str,
) -> io::Result<Vec<(PathBuf, String)>> {
    let manifest = hex_manifest(tool)
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    package_relative_paths(package_files, manifest, "hex_deploy")
}

/// Returns the first double-quoted string in `text`.
fn first_quoted(text: &str) -> Option<&str> {
    let rest = &text[text.find('"')? + 1..];
    Some(&rest[..rest.find('"')?])
}

/// Returns the version declared in a mix.exs, `@version "x"` first, then `version: "x"`.
pub fn mix_version(text: &str) -> Option<String> {
    let line_with = |prefix: &str| {
        text.lines()
            .filter_map(|line| line.trim().strip_prefix(prefix))
            .find_map(|rest| first_quoted(rest.split('#').next().unwrap_or("")))
    };
    line_with("@version")
        .or_else(|| line_with("version:"))
        .map(str::to_owned)
}

/// Returns the version declared by `{vsn, "x"}` in an .app.src.
pub fn app_src_version(text: &str) -> Option<String> {
    let start = text.find("{vsn")?;
    let rest = &text[start + 4..];
    let rest = rest[..rest.find('}')?].trim_start().strip_prefix(',')?;
    first_quoted(rest).map(str::to_owned)
}

/// Returns a refusal when the staged package does not declare the target version.
fn hex_version_refusal(
    staging: &Path,
    mapped: &[(PathBuf, String)],
    tool: &str,
    version: &str,
) -> Option<String> {
    let (file, found) = if tool == "mix" {
        let text = std::fs::read_to_string(staging.join("mix.exs")).unwrap_or_default();
        ("mix.exs".to_owned(), mix_version(&text))
    } else {
        let Some((_, rel)) = mapped.iter().find(|(_, rel)| rel.ends_with(".app.src")) else {
            return Some("hex_deploy: package has no .app.src file; refusing".to_owned());
        };
        let text = std::fs::read_to_string(staging.join(rel)).unwrap_or_default();
        (rel.clone(), app_src_version(&text))
    };
    if found.as_deref() == Some(version) {
        return None;
    }
    Some(format!(
        "hex_deploy: {file} declares version {}, target is {version}; refusing",
        found.as_deref().unwrap_or("(not a plain value)")
    ))
}

fn hex_stage(mapped: &[(PathBuf, String)], staging: &Path) -> Result<(), String> {
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

/// Returns the last output lines of a failed child with the key removed.
fn hex_failure_lines(out: &std::process::Output, key: &str) -> String {
    let text = crate::run::pages_redact(
        &format!(
            "{}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ),
        key,
    );
    let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    lines[lines.len().saturating_sub(HEX_FAILURE_LINES)..]
        .iter()
        .map(|line| format!("\nhex: {line}"))
        .collect()
}

/// Runs one mix or rebar3 command in the staged package with an empty HOME.
fn hex_run(program: &Path, args: &[&str], dir: &Path, key: &str) -> Result<(), String> {
    let home = tempfile_stage_dir("hex-home-").map_err(|error| format!("hex_deploy: {error}"))?;
    let home_text = home.to_string_lossy().into_owned();
    let mut cmd = scrubbed_command(
        program,
        &[PUBLISH_CHILD_ENV_KEEP, HEX_CHILD_ENV_EXTRA],
        &[
            ("HOME", home_text.as_str()),
            ("USERPROFILE", home_text.as_str()),
            ("HEX_API_KEY", key),
        ],
    );
    cmd.args(args).current_dir(dir);
    let output = cmd.output();
    let _ = std::fs::remove_dir_all(&home);
    let output = output.map_err(|error| format!("hex_deploy: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "hex_deploy: {} {} failed with {}{}",
        program.display(),
        args.join(" "),
        output.status,
        hex_failure_lines(&output, key)
    ))
}

/// Returns the tool arguments that build the package, writing a mix tarball to `tarball`.
fn hex_build_args<'a>(tool: &str, tarball: &'a str) -> Vec<&'a str> {
    if tool == "mix" {
        vec!["hex.build", "-o", tarball]
    } else {
        vec!["hex", "build"]
    }
}

/// Returns the tool arguments that publish the package.
fn hex_publish_args(tool: &str) -> Vec<&'static str> {
    if tool == "mix" {
        vec!["hex.publish", "--yes"]
    } else {
        vec!["hex", "publish", "--yes"]
    }
}

/// Stages the package under `dir`, checks its version and returns the mapped paths.
fn hex_stage_checked(
    files: &[PathBuf],
    dir: &Path,
    tool: &str,
    version: &str,
) -> Result<(), String> {
    let mapped = hex_package_paths(files, tool).map_err(|error| format!("hex_deploy: {error}"))?;
    hex_stage(&mapped, dir).map_err(|error| format!("hex_deploy: {error}"))?;
    match hex_version_refusal(dir, &mapped, tool, version) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// Runs the Hex deploy tool.
pub fn hex_main(
    package_rlocs: &[String],
    package_name: &str,
    version: &str,
    tool: &str,
    argv: &[String],
) -> i32 {
    let Some(run) = DeployRun::start(
        "hex_deploy",
        "the package is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    if let Err(error) = hex_manifest(tool) {
        eprintln!("{error}");
        return 1;
    }
    let mut files = Vec::new();
    for rloc in package_rlocs.iter().filter(|s| !s.is_empty()) {
        let Some(path) = run.resolve(rloc) else {
            return 1;
        };
        files.push(path);
    }
    if HEX_GATE.requested(env_or_empty) {
        return hex_publish(&files, package_name, version, tool);
    }
    let house = run.outdir().join(format!("{package_name}-hex"));
    let package_dir = house.join("package");
    let _ = std::fs::remove_dir_all(&house);
    let staged = std::fs::create_dir_all(&package_dir)
        .map_err(|error| format!("hex_deploy: {error}"))
        .and_then(|()| hex_stage_checked(&files, &package_dir, tool, version));
    if let Err(error) = staged {
        eprintln!("{error}");
        return 1;
    }
    let tarball = house.join(format!("{package_name}-{version}.tar"));
    match which_on_path(tool) {
        Some(program) => {
            let tarball_text = tarball.to_string_lossy().into_owned();
            let args = hex_build_args(tool, &tarball_text);
            if let Err(error) = hex_run(&program, &args, &package_dir, "") {
                eprintln!("{error}");
                return 1;
            }
        }
        None => println!("hex_deploy: {tool} not found on PATH; skipped the hex build"),
    }
    println!(
        "hex_deploy: staged {package_name} {version} (build_mode: {})",
        deploy_build_mode()
    );
    println!("  package: {}", package_dir.display());
    if tarball.is_file() {
        println!("  tarball: {}", tarball.display());
    }
    0
}

fn hex_publish(files: &[PathBuf], package_name: &str, version: &str, tool: &str) -> i32 {
    if let Some(error) = placeholder_refusal("hex_deploy", version) {
        eprintln!("{error}");
        return 1;
    }
    let secrets = match HEX_GATE.authorize(env_or_empty) {
        Ok(secrets) => secrets,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    let Some(program) = which_on_path(tool) else {
        eprintln!("hex_deploy: {tool} not found on PATH; refusing");
        return 1;
    };
    let staging = match tempfile_stage_dir("hex-publish-") {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("hex_deploy: {error}");
            return 1;
        }
    };
    let published = hex_stage_checked(files, &staging, tool, version).and_then(|()| {
        hex_run(
            &program,
            &hex_publish_args(tool),
            &staging,
            secrets.get("HEX_API_KEY"),
        )
    });
    let _ = std::fs::remove_dir_all(&staging);
    match published {
        Ok(()) => {
            println!("hex_deploy: published {package_name} {version}");
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    fn write(root: &Path, rel: &str, text: &str) -> PathBuf {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&path, text).expect("write");
        path
    }

    #[test]
    fn mix_version_reads_attribute_or_project_field() {
        assert_eq!(
            mix_version("  @version \"1.2.3\"\n  version: @version,\n"),
            Some("1.2.3".to_owned())
        );
        assert_eq!(
            mix_version("[app: :demo, version: \"0.4.0\", deps: []]\n  version: \"0.4.0\",\n"),
            Some("0.4.0".to_owned())
        );
        assert_eq!(mix_version("version: @other,\n"), None);
        assert_eq!(mix_version(""), None);
    }

    #[test]
    fn app_src_version_reads_vsn() {
        let text = "{application, demo,\n [{vsn, \"1.0.1\"},\n  {modules, []}]}.\n";
        assert_eq!(app_src_version(text), Some("1.0.1".to_owned()));
        assert_eq!(app_src_version("{vsn, git}"), None);
        assert_eq!(app_src_version("{application, demo, []}."), None);
    }

    #[test]
    fn version_must_match_target() {
        let scratch = scratch_dir();
        let root = scratch.path().join("in");
        let files = vec![
            write(&root, "mix.exs", "version: \"1.0.0\",\n"),
            write(&root, "lib/demo.ex", ""),
        ];
        let staging = scratch.path().join("staging");
        assert_eq!(hex_stage_checked(&files, &staging, "mix", "1.0.0"), Ok(()));
        let error = hex_stage_checked(&files, &staging, "mix", "2.0.0").expect_err("mismatch");
        assert!(
            error.contains("mix.exs declares version 1.0.0, target is 2.0.0"),
            "{error}"
        );

        let erl = scratch.path().join("erl");
        let files = vec![
            write(&erl, "rebar.config", "{erl_opts, []}.\n"),
            write(
                &erl,
                "src/demo.app.src",
                "{application, demo, [{vsn, \"0.2.0\"}]}.\n",
            ),
        ];
        let staging = scratch.path().join("erl_staging");
        assert_eq!(
            hex_stage_checked(&files, &staging, "rebar3", "0.2.0"),
            Ok(())
        );
        assert!(hex_stage_checked(&files, &staging, "rebar3", "0.3.0").is_err());
        let error =
            hex_stage_checked(&files, &staging, "gleam", "0.2.0").expect_err("unknown tool");
        assert!(error.contains("unknown tool"), "{error}");
    }

    #[test]
    fn package_root_needs_the_tool_manifest() {
        let scratch = scratch_dir();
        let files = vec![write(scratch.path(), "src/demo.app.src", "")];
        let error = hex_package_paths(&files, "rebar3").expect_err("no rebar.config");
        assert!(error.to_string().contains("no rebar.config"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn hex_run_hides_key_and_uses_empty_home() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = scratch_dir();
        let fake = scratch.path().join("mix");
        std::fs::write(
            &fake,
            "#!/bin/sh\necho \"key $HEX_API_KEY\" >&2\nls -A \"$HOME\" | wc -l >&2\nexit 1\n",
        )
        .expect("write fake mix");
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let message = hex_run(&fake, &["hex.publish", "--yes"], scratch.path(), "s3cret")
            .expect_err("publish fails");
        assert!(
            message.contains("hex.publish --yes failed with"),
            "{message}"
        );
        assert!(message.contains("\nhex: key [redacted]"), "{message}");
        assert!(message.contains("\nhex: 0"), "{message}");
        assert!(!message.contains("s3cret"), "{message}");
        std::fs::write(&fake, "#!/bin/sh\nexit 0\n").expect("rewrite");
        assert_eq!(hex_run(&fake, &[], scratch.path(), ""), Ok(()));
    }
}
