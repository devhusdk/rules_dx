use std::path::{Path, PathBuf};

use dx_test_scratch::{scratch, TempDir};
use dx_testing::http::{requests, serve, Reply, RequestLog};
use dx_testing::{resolve_runfiles, run, Run};

const DX_TOML: &str = r#"schema_version = 1

[[dependency_set]]
name = "frontend"
ecosystem = "uv"
manifests = ["apps/frontend/pyproject.toml"]
locks = ["apps/frontend/uv.lock"]
scopes = ["apps/frontend"]

[[dependency_set]]
name = "worker"
ecosystem = "uv"
manifests = ["services/worker/pyproject.toml"]
locks = ["services/worker/uv.lock"]
scopes = ["services/worker"]
"#;

const HOSTILE_CONFIG: &str = "exclude-newer = \"2020-01-01T00:00:00Z\"\n";

const FRONTEND: &str = "apps/frontend";

const WORKER: &str = "services/worker";

const WHEEL: &str = "dxprobe-1.0-py3-none-any.whl";

const EXTRA_WHEEL: &str = "dxextra-1.0-py3-none-any.whl";

const WHEEL_SCRIPT: &str = r#"
import sys, zipfile
path, name = sys.argv[1], sys.argv[2]
with zipfile.ZipFile(path, "w") as z:
    z.writestr(name + "/__init__.py", "")
    z.writestr(name + "-1.0.dist-info/METADATA", "Metadata-Version: 2.1\nName: " + name + "\nVersion: 1.0\n")
    z.writestr(name + "-1.0.dist-info/WHEEL", "Wheel-Version: 1.0\nGenerator: t\nRoot-Is-Purelib: true\nTag: py3-none-any\n")
    z.writestr(name + "-1.0.dist-info/RECORD", "")
"#;

fn pyproject(name: &str, requires: &str, url: Option<&str>) -> String {
    pyproject_with(name, requires, url, &[WHEEL])
}

fn pyproject_with(name: &str, requires: &str, url: Option<&str>, wheels: &[&str]) -> String {
    let head = format!(
        "[project]\nname = \"{name}\"\nversion = \"0.1.0\"\nrequires-python = \"{requires}\"\n"
    );
    match url {
        None => format!("{head}dependencies = []\n"),
        Some(url) => {
            let names: Vec<&str> = wheels
                .iter()
                .filter_map(|wheel| wheel.split('-').next())
                .collect();
            let deps: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
            let sources: Vec<String> = wheels
                .iter()
                .zip(&names)
                .map(|(wheel, name)| format!("{name} = {{ url = \"{url}/{wheel}\" }}"))
                .collect();
            format!(
                "{head}dependencies = [{}]\n\n[tool.uv.sources]\n{}\n",
                deps.join(", "),
                sources.join("\n")
            )
        }
    }
}

struct Index {
    url: String,
    seen: RequestLog,
}

impl Index {
    fn start(wheels: Vec<(String, Vec<u8>)>) -> Index {
        let (url, seen) = serve(move |path| {
            wheels
                .iter()
                .find(|(name, _)| path.ends_with(name.as_str()))
                .map(|(_, bytes)| Reply {
                    content_type: "application/octet-stream",
                    body: bytes.clone(),
                })
        })
        .unwrap_or_else(|error| panic!("serve: {error}"));
        Index { url, seen }
    }

    fn requests(&self) -> Vec<String> {
        requests(&self.seen)
    }
}

fn runfile(var: &str) -> PathBuf {
    let rel = std::env::var(var).unwrap_or_else(|_| panic!("{var} must name a runfile"));
    let path = resolve_runfiles(&rel);
    std::fs::canonicalize(&path).unwrap_or_else(|error| panic!("{var} {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn read(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

struct Consumer {
    root: PathBuf,
    ambient: PathBuf,
    index: Index,
    /// Declared last so it drops after the index server stops.
    _guard: TempDir,
}

impl Consumer {
    fn new(name: &str) -> Consumer {
        let guard = scratch(name);
        let base = guard.path().to_path_buf();
        let root = base.join("ws");
        let ambient = base.join("ambient");
        let python = runfile("UV_SETS_PYTHON");
        let wheels = [WHEEL, EXTRA_WHEEL]
            .into_iter()
            .map(|file| {
                let path = base.join(file);
                let name = file.split('-').next().unwrap_or_default();
                let built = std::process::Command::new(&python)
                    .arg("-c")
                    .arg(WHEEL_SCRIPT)
                    .arg(&path)
                    .arg(name)
                    .status()
                    .unwrap_or_else(|error| panic!("build wheel with {python:?}: {error}"));
                assert!(built.success(), "wheel build failed");
                let bytes =
                    std::fs::read(&path).unwrap_or_else(|error| panic!("wheel {path:?}: {error}"));
                (file.to_owned(), bytes)
            })
            .collect();
        let index = Index::start(wheels);
        write(&root.join("MODULE.bazel"), "module(name = \"consumer\")\n");
        write(&root.join("dx.toml"), DX_TOML);
        write(
            &root.join(FRONTEND).join("pyproject.toml"),
            &pyproject("frontend", ">=3.9", Some(&index.url)),
        );
        write(
            &root.join(WORKER).join("pyproject.toml"),
            &pyproject("worker", ">=3.9", None),
        );
        write(&ambient.join("home/.config/uv/uv.toml"), HOSTILE_CONFIG);
        write(&ambient.join("xdg/uv/uv.toml"), HOSTILE_CONFIG);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let fake = ambient.join("bin/uv");
            write(
                &fake,
                &format!(
                    "#!/bin/sh\ntouch '{}'\nexit 0\n",
                    ambient.join("ambient-uv-ran").display()
                ),
            );
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|error| panic!("chmod: {error}"));
            // An unset DX_UV_BIN makes dx ask Bazel; this one fails fast without a server.
            let bazel = ambient.join("bin/bazel");
            write(
                &bazel,
                "#!/bin/sh\necho \"ERROR: no such repository '@dx_tools'\" >&2\nexit 2\n",
            );
            std::fs::set_permissions(&bazel, std::fs::Permissions::from_mode(0o755))
                .unwrap_or_else(|error| panic!("chmod: {error}"));
        }
        Consumer {
            root,
            ambient,
            index,
            _guard: guard,
        }
    }

    fn lock(&self, dir: &str) -> Option<Vec<u8>> {
        read(&self.root.join(dir).join("uv.lock"))
    }

    fn dx(&self, args: &[&str], uv: Option<&str>) -> Run {
        let workspace = self.root.to_string_lossy().into_owned();
        let mut argv: Vec<&str> = args.to_vec();
        argv.push("--workspace");
        argv.push(workspace.as_str());
        let pinned_uv = runfile("UV_SETS_UV").to_string_lossy().into_owned();
        let python = runfile("UV_SETS_PYTHON").to_string_lossy().into_owned();
        let home = self.ambient.join("home").to_string_lossy().into_owned();
        let xdg = self.ambient.join("xdg").to_string_lossy().into_owned();
        let mut entries = vec![self.ambient.join("bin")];
        entries.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let path = std::env::join_paths(entries)
            .unwrap_or_else(|error| panic!("PATH: {error}"))
            .to_string_lossy()
            .into_owned();
        let envs = [
            ("DX_UV_BIN", uv.unwrap_or(pinned_uv.as_str())),
            ("DX_UV_PYTHON", python.as_str()),
            ("HOME", home.as_str()),
            ("USERPROFILE", home.as_str()),
            ("XDG_CONFIG_HOME", xdg.as_str()),
            ("UV_EXCLUDE_NEWER", "2020-01-01T00:00:00Z"),
            ("UV_INDEX_URL", "https://example.invalid/simple"),
            ("UV_DEFAULT_INDEX", "https://example.invalid/simple"),
            ("UV_OFFLINE", "1"),
            ("PATH", path.as_str()),
        ];
        run(&dx(), &argv, &envs).unwrap_or_else(|error| panic!("run dx: {error}"))
    }

    fn ambient_uv_ran(&self) -> bool {
        self.ambient.join("ambient-uv-ran").exists()
    }
}

fn dx() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

fn assert_code(run: &Run, code: i32) {
    assert_eq!(run.status.code(), Some(code), "{}", run.combined());
}

#[test]
fn pinned_uv_checks_and_applies_only_the_selected_set() {
    let consumer = Consumer::new("uv-sets-flow-");

    let missing = consumer.dx(&["update", "--check"], None);
    assert_code(&missing, 1);
    assert!(
        missing.stderr.contains("is missing"),
        "{}",
        missing.combined()
    );
    assert_eq!(consumer.lock(FRONTEND), None);

    let applied = consumer.dx(&["update", "--apply", "frontend"], None);
    assert_code(&applied, 0);
    assert!(
        applied
            .stdout
            .contains("updated frontend (apps/frontend/uv.lock)"),
        "{}",
        applied.combined()
    );
    let frontend = consumer.lock(FRONTEND).expect("frontend lock");
    assert_eq!(consumer.lock(WORKER), None, "worker was not selected");
    let text = String::from_utf8_lossy(&frontend);
    assert!(text.contains("name = \"frontend\""), "{text}");
    assert!(
        text.contains(&format!("{}/{WHEEL}", consumer.index.url)),
        "{text}"
    );
    assert!(!text.contains("example.invalid"), "{text}");
    let seen = consumer.index.requests();
    assert!(seen.iter().any(|path| path.ends_with(WHEEL)), "{seen:?}");
    assert!(text.contains("requires-python = \">=3.9\""), "{text}");
    assert!(
        !text.contains("exclude-newer"),
        "ambient config leaked: {text}"
    );

    let applied = consumer.dx(&["update", "--apply", "worker"], None);
    assert_code(&applied, 0);
    let worker = consumer.lock(WORKER).expect("worker lock");
    assert_eq!(consumer.lock(FRONTEND), Some(frontend.clone()));

    for args in [
        &["update", "--check"][..],
        &["update", "--check", "--frozen"][..],
    ] {
        let checked = consumer.dx(args, None);
        assert_code(&checked, 0);
        assert!(
            checked
                .stdout
                .contains("frontend lockfile current (apps/frontend/uv.lock)"),
            "{}",
            checked.combined()
        );
        assert!(
            checked
                .stdout
                .contains("worker lockfile current (services/worker/uv.lock)"),
            "{}",
            checked.combined()
        );
        assert_eq!(consumer.lock(FRONTEND), Some(frontend.clone()));
        assert_eq!(consumer.lock(WORKER), Some(worker.clone()));
    }

    write(
        &consumer.root.join(FRONTEND).join("pyproject.toml"),
        &pyproject("frontend", ">=3.10", Some(&consumer.index.url)),
    );
    let stale = consumer.dx(&["update", "--check"], None);
    assert_code(&stale, 1);
    assert!(
        stale
            .stderr
            .contains("failed to check frontend: updater exited 1"),
        "{}",
        stale.combined()
    );
    assert!(
        stale
            .stdout
            .contains("worker lockfile current (services/worker/uv.lock)"),
        "{}",
        stale.combined()
    );
    assert_eq!(consumer.lock(FRONTEND), Some(frontend.clone()));

    for (args, reason) in [
        (
            &["update", "--apply", "--frozen", "frontend"][..],
            "frozen_locked",
        ),
        (
            &["update", "--apply", "--offline", "frontend"][..],
            "offline_required",
        ),
    ] {
        let refused = consumer.dx(args, None);
        assert_code(&refused, 1);
        assert!(refused.stderr.contains(reason), "{}", refused.combined());
        assert_eq!(consumer.lock(FRONTEND), Some(frontend.clone()));
    }

    let applied = consumer.dx(&["update", "--apply", "frontend"], None);
    assert_code(&applied, 0);
    let refreshed = consumer.lock(FRONTEND).expect("frontend lock");
    assert_ne!(refreshed, frontend);
    let text = String::from_utf8_lossy(&refreshed);
    assert!(text.contains("requires-python = \">=3.10\""), "{text}");
    assert_eq!(consumer.lock(WORKER), Some(worker));
    assert!(!consumer.ambient_uv_ran(), "ambient uv on PATH ran");
}

#[test]
fn missing_or_relative_pinned_uv_is_never_clean() {
    let consumer = Consumer::new("uv-sets-unavailable-");
    let applied = consumer.dx(&["update", "--apply"], None);
    assert_code(&applied, 0);
    let before = consumer.lock(FRONTEND).expect("frontend lock");
    for uv in ["", "uv"] {
        let checked = consumer.dx(&["update", "--check"], Some(uv));
        assert_code(&checked, 1);
        assert!(
            checked.stderr.contains("backend_unavailable"),
            "{}",
            checked.combined()
        );
        assert!(
            checked.stderr.contains("DX_UV_BIN"),
            "{}",
            checked.combined()
        );
        assert!(
            !checked.stdout.contains("lockfile current"),
            "{}",
            checked.combined()
        );
    }
    assert_eq!(consumer.lock(FRONTEND), Some(before));
    assert!(!consumer.ambient_uv_ran(), "ambient uv on PATH ran");
}

#[test]
fn uv_config_in_the_set_is_unsupported() {
    let consumer = Consumer::new("uv-sets-config-");
    let applied = consumer.dx(&["update", "--apply"], None);
    assert_code(&applied, 0);
    let before = consumer.lock(WORKER).expect("worker lock");
    write(&consumer.root.join(WORKER).join("uv.toml"), HOSTILE_CONFIG);
    let refused = consumer.dx(&["update", "--apply", "worker"], None);
    assert_code(&refused, 1);
    assert!(
        refused.stderr.contains("update_set_unsupported"),
        "{}",
        refused.combined()
    );
    assert!(
        refused.stderr.contains("services/worker/uv.toml exists"),
        "{}",
        refused.combined()
    );
    assert_eq!(consumer.lock(WORKER), Some(before));
}

#[test]
fn offline_check_without_the_needed_inputs_is_never_clean() {
    let consumer = Consumer::new("uv-sets-offline-");
    let applied = consumer.dx(&["update", "--apply"], None);
    assert_code(&applied, 0);
    let before = consumer.lock(FRONTEND).expect("frontend lock");
    let online = consumer.dx(&["update", "--check"], None);
    assert_code(&online, 0);
    assert!(
        online
            .stdout
            .contains("frontend lockfile current (apps/frontend/uv.lock)"),
        "{}",
        online.combined()
    );
    write(
        &consumer.root.join(FRONTEND).join("pyproject.toml"),
        &pyproject_with(
            "frontend",
            ">=3.9",
            Some(&consumer.index.url),
            &[WHEEL, EXTRA_WHEEL],
        ),
    );
    let stale = consumer.dx(&["update", "--check"], None);
    assert_code(&stale, 1);
    assert!(
        !stale
            .combined()
            .contains("Network connectivity is disabled"),
        "{}",
        stale.combined()
    );
    let checked = consumer.dx(&["update", "--check", "--offline"], None);
    assert_code(&checked, 1);
    assert!(
        checked
            .combined()
            .contains("Network connectivity is disabled"),
        "{}",
        checked.combined()
    );
    assert!(
        !checked.stdout.contains("frontend lockfile current"),
        "{}",
        checked.combined()
    );
    assert!(
        checked
            .stdout
            .contains("worker lockfile current (services/worker/uv.lock)"),
        "{}",
        checked.combined()
    );
    assert_eq!(consumer.lock(FRONTEND), Some(before));
}
