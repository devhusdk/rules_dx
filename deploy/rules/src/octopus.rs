//! Octopus Deploy publisher: drop staging and the octo push and release.

use crate::archive::*;
use crate::run::{deploy_profile, env_or_empty, scrubbed_command, DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP};
use crate::stage::*;
use std::ffi::OsString;
use std::io;
use std::path::Path;

/// Octopus drop inputs.
pub struct OctopusDrop<'a> {
    pub deploy_name: &'a str,
    pub project: &'a str,
    pub channel: &'a str,
    pub version: &'a str,
    pub deploy_to: &'a [String],
    pub space: &'a str,
    pub server: &'a str,
}

/// Builds the argv for `octo push`.
pub fn octopus_push_args(package: &Path, spec: &OctopusDrop<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "push".into(),
        "--package".into(),
        package.as_os_str().to_owned(),
        "--server".into(),
        spec.server.into(),
    ];
    if !spec.space.is_empty() {
        args.push("--space".into());
        args.push(spec.space.into());
    }
    args
}

/// Builds the argv for `octo create-release`.
pub fn octopus_release_args(spec: &OctopusDrop<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "create-release".into(),
        "--project".into(),
        spec.project.into(),
        "--channel".into(),
        spec.channel.into(),
        "--version".into(),
        spec.version.into(),
    ];
    for env in spec.deploy_to {
        args.push("--deployTo".into());
        args.push(env.into());
    }
    args.push("--server".into());
    args.push(spec.server.into());
    if !spec.space.is_empty() {
        args.push("--space".into());
        args.push(spec.space.into());
    }
    args
}

/// Renders an octo argv as one display line.
fn octo_line(args: &[OsString]) -> String {
    let words: Vec<String> = args
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    format!("octo {}", words.join(" "))
}

/// Octopus launcher inputs.
pub struct OctopusLaunch<'a> {
    pub package_rloc: &'a str,
    pub deploy_name: &'a str,
    pub project: &'a str,
    pub channel: &'a str,
    pub version: &'a str,
    pub deploy_to_raw: &'a str,
    pub space_default: &'a str,
    pub url_default: &'a str,
}

/// Builds one Octopus package drop.
pub fn octopus_build_drop(
    package_src: &Path,
    outdir: &Path,
    spec: &OctopusDrop<'_>,
) -> io::Result<std::path::PathBuf> {
    let src_str = package_src.to_string_lossy();
    if !src_str.ends_with(".tar.gz") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "octopus drop: want exactly one .tar.gz source (archive_deploy output), got '{src_str}'"
            ),
        ));
    }
    if spec.deploy_name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty deploy name",
        ));
    }
    if spec.project.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty project",
        ));
    }
    if spec.channel.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty channel",
        ));
    }
    if spec.version.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "octopus drop: need a non-empty version",
        ));
    }
    let drop = outdir.join(spec.deploy_name.to_owned() + "-drop");
    std::fs::create_dir_all(&drop)?;
    let base = basename(package_src)?;
    let dest = drop.join(&base);
    let digest = copy_verified(package_src, &dest)?;
    let manifest = format!(
        "# would-run manifest for octopus_deploy (local default publishes nothing)\n{}\n{}\npackage-sha256: {digest}  {base}\n",
        octo_line(&octopus_push_args(Path::new(&base), spec)),
        octo_line(&octopus_release_args(spec)),
    );
    std::fs::write(drop.join("would-run.txt"), manifest.as_bytes())?;
    Ok(drop)
}

/// Owner approval gate for octopus_deploy.
pub(crate) const OCTOPUS_GATE: OwnerGate = OwnerGate {
    tool: "octopus_deploy",
    action: "push",
    credentials: &["OCTOPUS_API_KEY"],
    approved: "OCTOPUS_PUBLISH_APPROVED",
    live: "OCTOPUS_PUBLISH_LIVE",
};

/// Runs one octo step with the key in env only; true on success.
fn octo_run(octo: &Path, args: &[OsString], api_key: &str, step: &str) -> bool {
    let mut cmd = scrubbed_command(
        octo,
        &[PUBLISH_CHILD_ENV_KEEP],
        &[("OCTOPUS_CLI_API_KEY", api_key)],
    );
    cmd.args(args);
    match cmd.status() {
        Ok(status) if status.success() => true,
        Ok(status) => {
            eprintln!("octopus_deploy: octo {step} failed with {status}");
            false
        }
        Err(error) => {
            eprintln!("octopus_deploy: octo {step}: {error}");
            false
        }
    }
}

/// Pushes the package, then creates the release and queues each deployment.
pub fn octopus_live_publish(
    octo: &Path,
    package: &Path,
    spec: &OctopusDrop<'_>,
    api_key: &str,
) -> i32 {
    let name = package
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !octo_run(octo, &octopus_push_args(package, spec), api_key, "push") {
        return 1;
    }
    if !octo_run(octo, &octopus_release_args(spec), api_key, "create-release") {
        eprintln!(
            "octopus_deploy: package {name} is already pushed; a rerun will fail on push (FailIfExists)"
        );
        return 1;
    }
    println!("octopus_deploy: pushed package {name}");
    println!(
        "octopus_deploy: created release {} {} on channel {}",
        spec.project, spec.version, spec.channel
    );
    for env in spec.deploy_to {
        println!(
            "octopus_deploy: queued deploy of {} {} to {env}",
            spec.project, spec.version
        );
    }
    0
}

/// Runs the Octopus deploy launcher.
pub fn octopus_main(spec: &OctopusLaunch<'_>, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "octopus_deploy",
        "the drop is exactly the file pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    let outdir = run.outdir();
    let Some(package) = run.resolve(spec.package_rloc) else {
        return 1;
    };
    let space = std::env::var("OCTOPUS_SPACE").unwrap_or_else(|_| spec.space_default.to_owned());
    let server = std::env::var("OCTOPUS_URL").unwrap_or_else(|_| spec.url_default.to_owned());
    let deploy_to: Vec<String> = spec
        .deploy_to_raw
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_owned())
        .collect();
    let drop_spec = OctopusDrop {
        deploy_name: spec.deploy_name,
        project: spec.project,
        channel: spec.channel,
        version: spec.version,
        deploy_to: &deploy_to,
        space: &space,
        server: &server,
    };
    if OCTOPUS_GATE.requested(env_or_empty) {
        if spec.version == "0.0.0" {
            eprintln!("octopus_deploy: live push refuses version 0.0.0; set a real version");
            return 1;
        }
        if server.is_empty() || server == "https://octopus.example.invalid" {
            eprintln!(
                "octopus_deploy: live push needs OCTOPUS_URL plus explicit owner approval (OCTOPUS_PUBLISH_APPROVED=1); refusing"
            );
            return 1;
        }
        let secrets = match OCTOPUS_GATE.authorize(env_or_empty) {
            Ok(secrets) => secrets,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        };
        let Some(octo) = which_on_path("octo") else {
            eprintln!("octopus_deploy: 'octo' CLI not found on PATH");
            return 1;
        };
        return octopus_live_publish(&octo, &package, &drop_spec, secrets.get("OCTOPUS_API_KEY"));
    }
    match octopus_build_drop(&package, outdir, &drop_spec) {
        Ok(drop) => {
            let profile = deploy_profile();
            println!(
                "octopus_deploy: staged {} {} (profile: {profile})",
                spec.project, spec.version
            );
            println!("  drop: {}", drop.display());
            println!("  manifest: {}", drop.join("would-run.txt").display());
            0
        }
        Err(error) => {
            eprintln!("octopus_deploy: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::*;

    #[test]
    fn octopus_drop_writes_manifest() {
        let scratch = scratch_dir();
        let package = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&package, b"package-bytes-v1").expect("write package");
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let drop = octopus_build_drop(
            &package,
            &outdir,
            &OctopusDrop {
                deploy_name: "octopus_demo",
                project: "octopus_demo",
                channel: "Default",
                version: "0.0.0",
                deploy_to: &["Production".to_owned()],
                space: "",
                server: "https://octopus.example.invalid",
            },
        )
        .expect("drop");
        assert!(drop.join("release_demo.tar.gz").is_file());
        let manifest = std::fs::read_to_string(drop.join("would-run.txt")).expect("manifest");
        let spec = OctopusDrop {
            deploy_name: "octopus_demo",
            project: "octopus_demo",
            channel: "Default",
            version: "0.0.0",
            deploy_to: &["Production".to_owned()],
            space: "",
            server: "https://octopus.example.invalid",
        };
        let lines: Vec<&str> = manifest.lines().collect();
        assert_eq!(
            lines[1],
            "octo push --package release_demo.tar.gz --server https://octopus.example.invalid"
        );
        assert_eq!(
            lines[2],
            "octo create-release --project octopus_demo --channel Default --version 0.0.0 --deployTo Production --server https://octopus.example.invalid"
        );
        assert_eq!(
            lines[1],
            octo_line(&octopus_push_args(Path::new("release_demo.tar.gz"), &spec))
        );
        assert_eq!(lines[2], octo_line(&octopus_release_args(&spec)));
    }

    #[test]
    fn octopus_args_builders_cover_optional_segments() {
        let envs = ["Production".to_owned(), "Staging".to_owned()];
        let spec = |deploy_to: &'static [String], space: &'static str| OctopusDrop {
            deploy_name: "demo",
            project: "demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to,
            space,
            server: "https://octo.invalid",
        };
        let envs: &'static [String] = Box::leak(Box::new(envs));
        let lines = |args: Vec<OsString>| octo_line(&args);
        let pkg = Path::new("demo-1.0.0.tar.gz");
        assert_eq!(
            lines(octopus_push_args(pkg, &spec(&[], ""))),
            "octo push --package demo-1.0.0.tar.gz --server https://octo.invalid"
        );
        assert_eq!(
            lines(octopus_push_args(pkg, &spec(&[], "Spaces"))),
            "octo push --package demo-1.0.0.tar.gz --server https://octo.invalid --space Spaces"
        );
        assert_eq!(
            lines(octopus_release_args(&spec(&[], ""))),
            "octo create-release --project demo --channel Default --version 1.0.0 --server https://octo.invalid"
        );
        let release = lines(octopus_release_args(&spec(envs, "Spaces")));
        assert_eq!(
            release,
            "octo create-release --project demo --channel Default --version 1.0.0 --deployTo Production --deployTo Staging --server https://octo.invalid --space Spaces"
        );
        let push = lines(octopus_push_args(pkg, &spec(envs, "Spaces")));
        for line in [&push, &release] {
            assert!(!line.contains("--deploy-to") && !line.contains("--apiKey"));
            assert!(!line.to_lowercase().contains("key"));
        }
        assert!(!release.contains("--package"));
    }

    #[test]
    fn octopus_drop_rejects_empty_spec_fields() {
        let scratch = scratch_dir();
        let outdir = scratch.path().join("out");
        std::fs::create_dir_all(&outdir).expect("outdir");
        let package = scratch.path().join("release_demo.tar.gz");
        std::fs::write(&package, b"package").expect("write package");
        let base = OctopusDrop {
            deploy_name: "octopus_demo",
            project: "octopus_demo",
            channel: "Default",
            version: "1.0.0",
            deploy_to: &[],
            space: "",
            server: "https://octopus.example.invalid",
        };
        let empties = [
            (
                "deploy_name",
                OctopusDrop {
                    deploy_name: "",
                    ..base
                },
            ),
            (
                "project",
                OctopusDrop {
                    project: "",
                    ..base
                },
            ),
            (
                "channel",
                OctopusDrop {
                    channel: "",
                    ..base
                },
            ),
            (
                "version",
                OctopusDrop {
                    version: "",
                    ..base
                },
            ),
        ];
        for (field, spec) in empties {
            let error = octopus_build_drop(&package, &outdir, &spec).expect_err("empty spec field");
            assert_eq!(
                error.kind(),
                io::ErrorKind::InvalidInput,
                "{field}: {error}"
            );
        }
    }

    #[cfg(unix)]
    fn fake_octo(dir: &Path, fail_on: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("octo");
        let d = dir.display();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" '---' >> '{d}/args.txt'\nenv >> '{d}/env.txt'\nif [ \"$1\" = '{fail_on}' ]; then exit 1; fi\nexit 0\n"
        );
        std::fs::write(&path, script).expect("write octo");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[cfg(unix)]
    fn octo_spec<'a>(envs: &'a [String], space: &'a str) -> OctopusDrop<'a> {
        OctopusDrop {
            deploy_name: "p",
            project: "p",
            channel: "Default",
            version: "1.0.0",
            deploy_to: envs,
            space,
            server: "https://o.invalid",
        }
    }

    #[cfg(unix)]
    fn octo_calls(dir: &Path) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(dir.join("args.txt")).unwrap_or_default();
        let mut calls = vec![];
        let mut cur = vec![];
        for line in text.lines() {
            if line == "---" {
                calls.push(std::mem::take(&mut cur));
            } else {
                cur.push(line.to_owned());
            }
        }
        calls
    }

    #[cfg(unix)]
    fn strings(args: Vec<OsString>) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_runs_push_then_release() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "none");
        let package = Path::new("/stage/p-1.0.0.tar.gz");
        let envs = ["Production".to_owned()];
        with_env(
            &[
                ("DX_LEAK_PROBE", Some("1")),
                ("OCTOPUS_API_KEY", Some("leaky")),
            ],
            || {
                assert_eq!(
                    octopus_live_publish(&octo, package, &octo_spec(&envs, "S"), "sekret"),
                    0
                );
            },
        );
        let calls = octo_calls(scratch.path());
        let spec = octo_spec(&envs, "S");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], strings(octopus_push_args(package, &spec)));
        assert_eq!(calls[1], strings(octopus_release_args(&spec)));
        assert!(calls[0].contains(&"S".to_owned()));
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        let env = std::fs::read_to_string(scratch.path().join("env.txt")).expect("env");
        assert!(!args.contains("sekret") && !args.contains("--apiKey"));
        assert!(env.contains("OCTOPUS_CLI_API_KEY=sekret"));
        assert!(!env.contains("OCTOPUS_API_KEY") && !env.contains("DX_LEAK_PROBE"));
        std::fs::remove_file(scratch.path().join("args.txt")).expect("rm");
        assert_eq!(
            octopus_live_publish(&octo, package, &octo_spec(&envs, ""), "sekret"),
            0
        );
        let args = std::fs::read_to_string(scratch.path().join("args.txt")).expect("args");
        assert!(!args.contains("--space"));
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_stops_after_failed_push() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "push");
        let code = octopus_live_publish(
            &octo,
            Path::new("p.tar.gz"),
            &octo_spec(&["Production".to_owned()], ""),
            "sekret",
        );
        assert_eq!(code, 1);
        assert_eq!(octo_calls(scratch.path()).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn octopus_live_publish_surfaces_release_failure() {
        let scratch = scratch_dir();
        let octo = fake_octo(scratch.path(), "create-release");
        let code = octopus_live_publish(
            &octo,
            Path::new("p.tar.gz"),
            &octo_spec(&["Production".to_owned()], ""),
            "sekret",
        );
        assert_eq!(code, 1);
        assert_eq!(octo_calls(scratch.path()).len(), 2);
    }
}
