//! Helm chart publisher: chart staging, lint, package and the OCI push.

use crate::run::{
    DeployRun, OwnerGate, PUBLISH_CHILD_ENV_KEEP, deploy_build_mode, env_or_empty, pages_redact,
    placeholder_refusal, scrubbed_command,
};
use crate::stage::copy_verified;
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// One chart file: its path inside the chart and its runfiles location.
#[derive(Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelmFile {
    pub path: String,
    pub rloc: String,
}

/// Everything a helm_chart_deploy target pins.
pub struct HelmLaunch<'a> {
    pub deploy_name: &'a str,
    pub version: &'a str,
    pub registry: &'a str,
    pub repository: &'a str,
    pub helm_rloc: &'a str,
    pub files: &'a [HelmFile],
}

/// Owner approval gate for helm_chart_deploy; it uses the OCI credentials.
pub(crate) const HELM_GATE: OwnerGate = OwnerGate {
    tool: "helm_chart_deploy",
    action: "push",
    credentials: &["OCI_REGISTRY_USER", "OCI_REGISTRY_TOKEN"],
    approved: "OCI_PUBLISH_APPROVED",
};

fn chart_yaml_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let value = line.strip_prefix(key)?.strip_prefix(':')?;
        let value = value.split(" #").next().unwrap_or("").trim();
        Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned())
    })
}

/// Reads the chart name and version from a Chart.yaml document.
pub fn helm_chart_identity(text: &str) -> Result<(String, String), String> {
    let name = chart_yaml_value(text, "name").unwrap_or_default();
    let version = chart_yaml_value(text, "version").unwrap_or_default();
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.'));
    if !valid {
        return Err(format!(
            "helm_chart_deploy: Chart.yaml name {name:?} must be lowercase letters, digits, '-' or '.'"
        ));
    }
    if version.is_empty() {
        return Err("helm_chart_deploy: Chart.yaml has no version".to_owned());
    }
    Ok((name, version))
}

fn stage_chart(
    files: &[HelmFile],
    resolve: &dyn Fn(&str) -> Option<PathBuf>,
    dir: &Path,
) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dir);
    for file in files {
        let relative = Path::new(&file.path);
        let plain = relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_)));
        if !plain {
            return Err(format!(
                "helm_chart_deploy: chart file path {:?} must stay inside the chart",
                file.path
            ));
        }
        let src = resolve(&file.rloc).ok_or_else(|| format!("cannot resolve {}", file.rloc))?;
        let dest = dir.join(relative);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("helm_chart_deploy: {error}"))?;
        }
        copy_verified(&src, &dest).map_err(|error| format!("helm_chart_deploy: {error}"))?;
    }
    Ok(())
}

struct Helm {
    program: PathBuf,
    home: PathBuf,
    secret: String,
}

impl Helm {
    fn command(&self) -> std::process::Command {
        let home = |name: &str| self.home.join(name).to_string_lossy().into_owned();
        let env = [
            ("HELM_CACHE_HOME", home("cache")),
            ("HELM_CONFIG_HOME", home("config")),
            ("HELM_DATA_HOME", home("data")),
            ("HELM_REGISTRY_CONFIG", home("registry.json")),
        ];
        let extra: Vec<(&str, &str)> = env.iter().map(|(k, v)| (*k, v.as_str())).collect();
        scrubbed_command(&self.program, &[PUBLISH_CHILD_ENV_KEEP], &extra)
    }

    /// Runs helm with the given args and optional stdin; prints redacted output.
    fn run(&self, args: &[&std::ffi::OsStr], stdin: Option<&str>) -> bool {
        let mut command = self.command();
        command.args(args);
        if stdin.is_some() {
            command.stdin(std::process::Stdio::piped());
        }
        command
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let child = command.spawn().and_then(|mut child| {
            if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
                pipe.write_all(text.as_bytes())?;
            }
            child.wait_with_output()
        });
        match child {
            Ok(out) => {
                let text =
                    |bytes: &[u8]| pages_redact(&String::from_utf8_lossy(bytes), &self.secret);
                print!("{}", text(&out.stdout));
                eprint!("{}", text(&out.stderr));
                if !out.status.success() {
                    eprintln!("helm_chart_deploy: helm failed with {}", out.status);
                }
                out.status.success()
            }
            Err(error) => {
                eprintln!("helm_chart_deploy: cannot run helm: {error}");
                false
            }
        }
    }
}

fn helm_push(
    helm: &Helm,
    launch: &HelmLaunch<'_>,
    package: &Path,
    user: &str,
    token: &str,
) -> bool {
    let host = launch.registry.split('/').next().unwrap_or(launch.registry);
    let target = if launch.repository.is_empty() {
        format!("oci://{}", launch.registry)
    } else {
        format!("oci://{}/{}", launch.registry, launch.repository)
    };
    let os = |text: &str| std::ffi::OsString::from(text);
    let login = [
        os("registry"),
        os("login"),
        os(host),
        os("--username"),
        os(user),
        os("--password-stdin"),
    ];
    let login: Vec<&std::ffi::OsStr> = login.iter().map(|a| a.as_os_str()).collect();
    if !helm.run(&login, Some(token)) {
        return false;
    }
    let ok = helm.run(
        &[
            "push".as_ref(),
            package.as_os_str(),
            os(&target).as_os_str(),
        ],
        None,
    );
    let _ = std::fs::remove_file(helm.home.join("registry.json"));
    if ok {
        println!(
            "helm_chart_deploy: pushed {} to {target}",
            package.display()
        );
    }
    ok
}

fn helm_main_with(
    launch: &HelmLaunch<'_>,
    outdir: &Path,
    resolve: &dyn Fn(&str) -> Option<PathBuf>,
    var: &dyn Fn(&str) -> String,
) -> i32 {
    let Some(program) = resolve(launch.helm_rloc) else {
        eprintln!(
            "helm_chart_deploy: cannot resolve helm {}",
            launch.helm_rloc
        );
        return 1;
    };
    let name = launch.deploy_name;
    let stage = outdir.join(format!("{name}-chart"));
    if let Err(error) = stage_chart(launch.files, resolve, &stage) {
        eprintln!("{error}");
        return 1;
    }
    let identity = std::fs::read_to_string(stage.join("Chart.yaml"))
        .map_err(|error| format!("helm_chart_deploy: cannot read Chart.yaml: {error}"))
        .and_then(|text| helm_chart_identity(&text));
    let (chart, version) = match identity {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("{error}");
            return 1;
        }
    };
    if version != launch.version {
        eprintln!(
            "helm_chart_deploy: Chart.yaml version {version} must equal the release version {}",
            launch.version
        );
        return 1;
    }
    let creds = if HELM_GATE.requested(var) {
        if let Some(error) = placeholder_refusal("helm_chart_deploy", launch.version) {
            eprintln!("{error}");
            return 1;
        }
        match HELM_GATE.authorize(var) {
            Ok(secrets) => Some((
                secrets.get("OCI_REGISTRY_USER").to_owned(),
                secrets.get("OCI_REGISTRY_TOKEN").to_owned(),
            )),
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    } else {
        None
    };
    let helm = Helm {
        program,
        home: outdir.join(format!("{name}-helm-home")),
        secret: creds
            .as_ref()
            .map(|(_, token)| token.clone())
            .unwrap_or_default(),
    };
    let packages = outdir.join(format!("{name}-package"));
    let _ = std::fs::remove_dir_all(&packages);
    if let Err(error) = std::fs::create_dir_all(&packages) {
        eprintln!("helm_chart_deploy: {error}");
        return 1;
    }
    if !helm.run(&["lint".as_ref(), stage.as_os_str()], None) {
        return 1;
    }
    let packed = helm.run(
        &[
            "package".as_ref(),
            stage.as_os_str(),
            "--destination".as_ref(),
            packages.as_os_str(),
        ],
        None,
    );
    if !packed {
        return 1;
    }
    let package = packages.join(format!("{chart}-{version}.tgz"));
    if !package.is_file() {
        eprintln!(
            "helm_chart_deploy: helm did not write {}",
            package.display()
        );
        return 1;
    }
    let Some((user, token)) = creds else {
        println!(
            "helm_chart_deploy: staged {chart}-{version}.tgz (build_mode: {})",
            deploy_build_mode()
        );
        println!("  package: {}", package.display());
        return 0;
    };
    let pushed = helm_push(&helm, launch, &package, &user, &token);
    let _ = std::fs::remove_dir_all(&helm.home);
    i32::from(!pushed)
}

/// Runs the Helm chart deploy tool.
pub fn helm_main(launch: &HelmLaunch<'_>, argv: &[String]) -> i32 {
    let Some(run) = DeployRun::start(
        "helm_chart_deploy",
        "the chart is exactly the files pinned at analysis time",
        argv,
    ) else {
        return 1;
    };
    helm_main_with(
        launch,
        run.outdir(),
        &|rloc| run.resolve(rloc),
        &env_or_empty,
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::test_support::scratch_dir;

    const CHART: &str = "apiVersion: v2\nname: demo\nversion: \"1.2.3\" # release\n";

    #[test]
    fn identity_reads_name_and_version() {
        assert_eq!(
            helm_chart_identity(CHART),
            Ok(("demo".to_owned(), "1.2.3".to_owned()))
        );
        assert!(helm_chart_identity("name: Demo\nversion: 1\n").is_err());
        assert!(helm_chart_identity("name: demo\n").is_err());
    }

    /// Writes a fake helm that records its args and writes the package.
    fn fake_helm(dir: &Path, exit: i32) -> PathBuf {
        let path = dir.join("helm");
        let log = dir.join("calls.txt");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{log}'\nif [ \"$1\" = package ]; then : > \"$4/demo-1.2.3.tgz\"; fi\nif [ \"$2\" = login ]; then cat >> '{log}'; fi\n[ \"$1\" = lint ] && exit {exit}\nexit 0\n",
            log = log.display()
        );
        dx_testing::executable_script(&path, &script);
        path
    }

    struct Fixture {
        dir: tempfile::TempDir,
        files: Vec<HelmFile>,
    }

    fn fixture(chart: &str) -> Fixture {
        let dir = scratch_dir();
        std::fs::write(dir.path().join("Chart.yaml"), chart).expect("chart");
        let files = vec![HelmFile {
            path: "Chart.yaml".to_owned(),
            rloc: "Chart.yaml".to_owned(),
        }];
        Fixture { dir, files }
    }

    fn run(fx: &Fixture, helm: &Path, version: &str, env: &[(&str, &str)]) -> i32 {
        let launch = HelmLaunch {
            deploy_name: "demo",
            version,
            registry: "registry.invalid/charts",
            repository: "team",
            helm_rloc: "helm",
            files: &fx.files,
        };
        let root = fx.dir.path().to_path_buf();
        let helm = helm.to_path_buf();
        let resolve = move |rloc: &str| {
            Some(if rloc == "helm" {
                helm.clone()
            } else {
                root.join(rloc)
            })
        };
        let var = |key: &str| {
            env.iter()
                .find(|(k, _)| *k == key)
                .map_or(String::new(), |(_, v)| (*v).to_owned())
        };
        helm_main_with(&launch, &fx.dir.path().join("out"), &resolve, &var)
    }

    fn calls(fx: &Fixture) -> String {
        std::fs::read_to_string(fx.dir.path().join("calls.txt")).unwrap_or_default()
    }

    #[test]
    fn dry_run_lints_and_packages_without_pushing() {
        let fx = fixture(CHART);
        let helm = fake_helm(fx.dir.path(), 0);
        assert_eq!(run(&fx, &helm, "1.2.3", &[]), 0);
        let log = calls(&fx);
        assert!(log.contains("lint "), "{log}");
        assert!(log.contains("package "), "{log}");
        assert!(!log.contains("push"), "{log}");
    }

    #[test]
    fn version_mismatch_and_lint_failure_stop_before_packaging() {
        let fx = fixture(CHART);
        let helm = fake_helm(fx.dir.path(), 1);
        assert_eq!(run(&fx, &helm, "9.9.9", &[]), 1);
        assert_eq!(calls(&fx), "");
        assert_eq!(run(&fx, &helm, "1.2.3", &[]), 1);
        assert!(!calls(&fx).contains("package"));
    }

    #[test]
    fn live_push_needs_the_oci_gate() {
        let fx = fixture(CHART);
        let helm = fake_helm(fx.dir.path(), 0);
        assert_eq!(run(&fx, &helm, "1.2.3", &[("DX_DEPLOY_LIVE", "1")]), 1);
        assert_eq!(calls(&fx), "");
        let live = [
            ("DX_DEPLOY_LIVE", "1"),
            ("OCI_PUBLISH_APPROVED", "1"),
            ("OCI_REGISTRY_USER", "bot"),
            ("OCI_REGISTRY_TOKEN", "s3cret"),
        ];
        assert_eq!(run(&fx, &helm, "1.2.3", &live), 0);
        let log = calls(&fx);
        assert!(
            log.contains("registry login registry.invalid --username bot --password-stdin"),
            "{log}"
        );
        assert!(log.contains("s3cret"), "{log}");
        assert!(log.contains("oci://registry.invalid/charts/team"), "{log}");
        assert!(!fx.dir.path().join("out/demo-helm-home").exists());
    }

    #[test]
    fn live_push_refuses_a_placeholder_version() {
        let fx = fixture("name: demo\nversion: 0.0.0\n");
        let helm = fake_helm(fx.dir.path(), 0);
        let live = [
            ("DX_DEPLOY_LIVE", "1"),
            ("OCI_PUBLISH_APPROVED", "1"),
            ("OCI_REGISTRY_USER", "bot"),
            ("OCI_REGISTRY_TOKEN", "t"),
        ];
        assert_eq!(run(&fx, &helm, "0.0.0", &live), 1);
        assert_eq!(calls(&fx), "");
    }

    #[test]
    fn chart_paths_cannot_escape() {
        let mut fx = fixture(CHART);
        fx.files[0].path = "../Chart.yaml".to_owned();
        let helm = fake_helm(fx.dir.path(), 0);
        assert_eq!(run(&fx, &helm, "1.2.3", &[]), 1);
    }
}
