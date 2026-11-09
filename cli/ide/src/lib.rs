#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

//! Editor support files under `.dx/local/ide/`, refreshed from `dx sync` and `dx upgrade`.

mod cc;
mod go;
mod jdk;
mod link;
mod python;
mod rust_analyzer;
mod typescript;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub use link::link;

pub const IDE_DIR: &str = ".dx/local/ide";
pub const STATE_FILE: &str = ".dx/local/ide/state.json";
/// Paths under `IDE_DIR` that the plug-ins write and editor settings point at.
pub const OUTPUTS: [&str; 8] = [
    "cc",
    "cc/clangd",
    "jdk/home",
    "python/python",
    "python/site-packages",
    "rust_analyzer/flycheck.exe",
    "rust_analyzer/discover_bazel_rust_project.exe",
    "typescript/lib",
];

/// Everything one plug-in sees when it writes.
pub struct Ctx<'a> {
    pub workspace: &'a Path,
    pub dir: &'a Path,
    pub outputs: &'a [PathBuf],
    pub runner: &'a dyn Runner,
}

/// One editor integration: what turns it on, what it needs built, and how it writes.
pub struct Plugin {
    pub name: &'static str,
    pub extensions: &'static [&'static str],
    pub labels: &'static [&'static str],
    /// Paths the plug-in writes, relative to `.dx/local/ide/<name>`.
    pub outputs: &'static [&'static str],
    pub write: fn(&Ctx<'_>) -> Result<(), String>,
}

/// Builds rules_dx labels and returns the absolute paths of their files.
pub trait Runner {
    fn build(&self, labels: &[String]) -> Result<Vec<PathBuf>, String>;
    /// Builds labels as they are written and returns the files of one output group.
    fn build_group(&self, labels: &[String], group: &str) -> Result<Vec<PathBuf>, String>;
    /// Runs `bazel aquery --output=jsonproto` on an expression and returns the output.
    fn aquery(&self, expr: &str) -> Result<String, String>;
    /// The absolute execution root of the workspace.
    fn execution_root(&self) -> Result<PathBuf, String>;
}

/// What one refresh did.
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Report {
    pub ran: bool,
    pub failed: Vec<(String, String)>,
}

impl Report {
    /// One line naming every failed plug-in, or `None` when all are fine.
    pub fn warning(&self) -> Option<String> {
        if self.failed.is_empty() {
            return None;
        }
        let list: Vec<String> = self
            .failed
            .iter()
            .map(|(name, error)| format!("{name}: {error}"))
            .collect();
        Some(format!("ide refresh failed: {}", list.join("; ")))
    }
}

const PLUGINS: &[&Plugin] = &[
    &python::PLUGIN,
    &typescript::PLUGIN,
    &cc::PLUGIN,
    &jdk::PLUGIN,
    &go::PLUGIN,
    &rust_analyzer::PLUGIN,
];

/// Refreshes `.dx/local/ide/` for the plug-ins that apply; a second run with the same inputs does nothing.
pub fn refresh(workspace: &Path, runner: &dyn Runner) -> Result<Report, String> {
    refresh_with(workspace, runner, PLUGINS)
}

fn refresh_with(
    workspace: &Path,
    runner: &dyn Runner,
    plugins: &[&Plugin],
) -> Result<Report, String> {
    let mut extensions = Vec::new();
    collect_extensions(workspace, &mut extensions);
    let active: Vec<&Plugin> = plugins
        .iter()
        .copied()
        .filter(|plugin| {
            plugin
                .extensions
                .iter()
                .any(|e| extensions.iter().any(|x| x == e))
        })
        .collect();
    let digest = inputs_digest(workspace, &active);
    let state_path = workspace.join(STATE_FILE);
    if let Some(old) = read_state(&state_path) {
        let complete = active
            .iter()
            .all(|plugin| missing_outputs_of(workspace, plugin).is_empty());
        if old.digest == digest && old.errors.is_empty() && complete {
            return Ok(Report::default());
        }
    }
    remove_stale(workspace, plugins, &active);
    let mut labels: Vec<String> = Vec::new();
    for plugin in &active {
        for label in plugin.labels {
            if !labels.iter().any(|l| l == label) {
                labels.push((*label).to_owned());
            }
        }
    }
    let built = if labels.is_empty() {
        Ok(Vec::new())
    } else {
        runner.build(&labels)
    };
    let mut report = Report {
        ran: true,
        failed: Vec::new(),
    };
    let mut results = BTreeMap::new();
    for plugin in &active {
        let result = match &built {
            Ok(outputs) => {
                let dir = workspace.join(IDE_DIR).join(plugin.name);
                (plugin.write)(&Ctx {
                    workspace,
                    dir: &dir,
                    outputs,
                    runner,
                })
            }
            Err(error) => Err(error.clone()),
        };
        match result {
            Ok(()) => results.insert(plugin.name.to_owned(), "ok".to_owned()),
            Err(error) => {
                report.failed.push((plugin.name.to_owned(), error.clone()));
                results.insert(plugin.name.to_owned(), error)
            }
        };
    }
    write_state(&state_path, &digest, &results)?;
    Ok(report)
}

fn collect_extensions(dir: &Path, found: &mut Vec<String>) {
    let ignored = bazelignore(dir);
    walk(dir, &ignored, found);
}

fn bazelignore(root: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(root.join(".bazelignore"))
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .map(|line| root.join(line.trim_end_matches('/')))
                .collect()
        })
        .unwrap_or_default()
}

fn walk(dir: &Path, ignored: &[PathBuf], found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let skip = name == ".git"
                || name == dx_path::state::DX_DIR
                || name.starts_with("bazel-")
                || ignored.contains(&path);
            if !skip {
                walk(&path, ignored, found);
            }
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && !found.iter().any(|f| f == ext)
        {
            found.push(ext.to_owned());
        }
    }
}

fn inputs_digest(workspace: &Path, active: &[&Plugin]) -> String {
    let mut bytes = Vec::new();
    for file in [".dx/pins", "MODULE.bazel", "MODULE.bazel.lock"] {
        bytes.extend_from_slice(file.as_bytes());
        bytes.push(0);
        bytes.extend(std::fs::read(workspace.join(file)).unwrap_or_default());
        bytes.push(0);
    }
    for plugin in active {
        bytes.extend_from_slice(plugin.name.as_bytes());
        bytes.push(0);
    }
    dx_digest::sha256_hex(&bytes)
}

struct State {
    digest: String,
    errors: Vec<String>,
    ok: Vec<String>,
}

fn read_state(path: &Path) -> Option<State> {
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    let digest = value.get("digest")?.as_str()?.to_owned();
    let plugins = value.get("plugins")?.as_object()?;
    let errors = plugins
        .values()
        .filter_map(|v| v.as_str())
        .filter(|v| *v != "ok")
        .map(str::to_owned)
        .collect();
    let ok = plugins
        .iter()
        .filter(|(_, v)| v.as_str() == Some("ok"))
        .map(|(name, _)| name.clone())
        .collect();
    Some(State { digest, errors, ok })
}

fn write_state(
    path: &Path,
    digest: &str,
    results: &BTreeMap<String, String>,
) -> Result<(), String> {
    let state = serde_json::json!({ "digest": digest, "plugins": results });
    let mut text = serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?;
    text.push('\n');
    dx_atomic_fs::write_atomic(path, text.as_bytes())
        .map_err(|error| format!("writing {STATE_FILE}: {error}"))
}

fn remove_stale(workspace: &Path, all: &[&Plugin], active: &[&Plugin]) {
    for plugin in all {
        if !active.iter().any(|a| a.name == plugin.name) {
            let _ = std::fs::remove_dir_all(workspace.join(IDE_DIR).join(plugin.name));
        }
    }
}

/// Reads the plug-in results from `state.json`: `None` when it has never run.
pub fn failed_plugins(workspace: &Path) -> Option<Vec<(String, String)>> {
    let value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(workspace.join(STATE_FILE)).ok()?).ok()?;
    let plugins = value.get("plugins")?.as_object()?;
    Some(
        plugins
            .iter()
            .filter_map(|(name, v)| Some((name.clone(), v.as_str()?.to_owned())))
            .filter(|(_, v)| v != "ok")
            .collect(),
    )
}

fn missing_outputs_of(workspace: &Path, plugin: &Plugin) -> Vec<String> {
    plugin
        .outputs
        .iter()
        .map(|output| format!("{IDE_DIR}/{}/{output}", plugin.name))
        .filter(|path| std::fs::symlink_metadata(workspace.join(path)).is_err())
        .collect()
}

/// Output files of plug-ins that last ran fine but are gone now, as workspace-relative paths.
pub fn missing_outputs(workspace: &Path) -> Vec<String> {
    let Some(state) = read_state(&workspace.join(STATE_FILE)) else {
        return Vec::new();
    };
    PLUGINS
        .iter()
        .filter(|plugin| state.ok.iter().any(|name| name == plugin.name))
        .flat_map(|plugin| missing_outputs_of(workspace, plugin))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn ide_dir_is_the_local_ide_dir() {
        use dx_path::state::{IDE, LOCAL_DIR};
        assert_eq!(IDE_DIR, format!("{LOCAL_DIR}/{IDE}"));
        assert_eq!(STATE_FILE, format!("{IDE_DIR}/state.json"));
    }

    struct Fake {
        calls: Cell<usize>,
        lib: PathBuf,
        aquery: String,
        groups: Vec<PathBuf>,
    }

    impl Runner for Fake {
        fn build(&self, _labels: &[String]) -> Result<Vec<PathBuf>, String> {
            self.calls.set(self.calls.get() + 1);
            Ok(vec![
                self.lib.clone(),
                self.lib.join("clangd"),
                self.lib.join("identity.cc_identity.json"),
                self.lib.join("identity_exec.cc_identity.json"),
            ])
        }

        fn build_group(&self, _labels: &[String], _group: &str) -> Result<Vec<PathBuf>, String> {
            Ok(self.groups.clone())
        }

        fn aquery(&self, _expr: &str) -> Result<String, String> {
            Ok(self.aquery.clone())
        }

        fn execution_root(&self) -> Result<PathBuf, String> {
            Ok(self.lib.clone())
        }
    }

    fn fake(root: &Path) -> Fake {
        let lib = root.join("store").join("typescript");
        std::fs::create_dir_all(lib.join("lib")).expect("lib");
        Fake {
            calls: Cell::new(0),
            lib,
            aquery: String::new(),
            groups: Vec::new(),
        }
    }

    fn fail(_: &Ctx<'_>) -> Result<(), String> {
        Err("boom".to_owned())
    }

    const AQUERY: &str = r#"{"targets":[{"id":1,"label":"//:a"}],"actions":[{"mnemonic":"CppCompile","arguments":["clang","-c","a.cc"],"targetId":1}]}"#;

    fn cc_runner(root: &Path) -> Fake {
        let mut runner = fake(root);
        std::fs::write(runner.lib.join("clangd"), "").expect("clangd");
        std::fs::write(
            runner.lib.join("identity.cc_identity.json"),
            r#"{"configuration":"k8-fastbuild","compiler":"clang","family":"clang","driver":"gnu","abi":"itanium","target":"x86_64-linux-gnu"}"#,
        )
        .expect("identity");
        std::fs::write(
            runner.lib.join("identity_exec.cc_identity.json"),
            r#"{"configuration":"k8-opt-exec-ST-0","compiler":"clang","family":"clang","driver":"gnu","abi":"itanium","target":"x86_64-linux-gnu"}"#,
        )
        .expect("exec identity");
        runner.aquery = AQUERY.to_owned();
        runner
    }

    #[test]
    #[cfg(unix)]
    fn cc_writes_the_database_and_links_clangd_once() {
        let scratch = dx_test_scratch::scratch("dx-ide-cc-");
        let root = scratch.path();
        let runner = cc_runner(root);
        refresh(root, &runner).expect("no cc");
        assert!(!root.join(".dx/local/ide/cc").exists());
        std::fs::write(root.join("a.cc"), "").expect("cc");
        let first = refresh(root, &runner).expect("first");
        assert_eq!(first.failed, []);
        assert!(first.ran);
        let text = std::fs::read_to_string(root.join(".dx/local/ide/cc/compile_commands.json"))
            .expect("database");
        assert!(text.contains("a.cc"), "{text}");
        assert!(root.join(".dx/local/ide/cc/clangd").exists());
        assert!(!refresh(root, &runner).expect("second").ran);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn an_aquery_failure_fails_only_cc() {
        let scratch = dx_test_scratch::scratch("dx-ide-cc-fail-");
        let root = scratch.path();
        let runner = cc_runner(root);
        std::fs::write(root.join("a.cc"), "").expect("cc");
        std::fs::write(root.join("b.ts"), "").expect("ts");
        let mut runner = runner;
        runner.aquery = "not json".to_owned();
        let report = refresh(root, &runner).expect("refresh");
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, "cc");
        scratch.close().expect("cleanup");
    }

    #[test]
    fn second_run_skips_bazel_and_a_new_pin_runs_it_again() {
        let scratch = dx_test_scratch::scratch("dx-ide-skip-");
        let root = scratch.path();
        std::fs::write(root.join("a.ts"), "").expect("ts");
        std::fs::create_dir_all(root.join(".dx")).expect("dx");
        std::fs::write(root.join(".dx/pins"), "1").expect("pin");
        let runner = fake(root);
        let first = refresh(root, &runner).expect("first");
        assert_eq!(first.failed, []);
        assert!(first.ran);
        assert!(!refresh(root, &runner).expect("second").ran);
        assert_eq!(runner.calls.get(), 1);
        std::fs::write(root.join(".dx/pins"), "2").expect("pin");
        refresh(root, &runner).expect("third");
        assert_eq!(runner.calls.get(), 2);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn a_failing_plugin_is_recorded_and_the_others_still_write() {
        let scratch = dx_test_scratch::scratch("dx-ide-fail-");
        let root = scratch.path();
        std::fs::write(root.join("a.ts"), "").expect("ts");
        std::fs::write(root.join("b.bad"), "").expect("bad");
        let runner = fake(root);
        let bad = Plugin {
            name: "bad",
            extensions: &["bad"],
            labels: &[],
            outputs: &[],
            write: fail,
        };
        let report = refresh_with(root, &runner, &[&bad, &typescript::PLUGIN]).expect("ok");
        assert_eq!(report.failed, [("bad".to_owned(), "boom".to_owned())]);
        assert_eq!(
            failed_plugins(root),
            Some(vec![("bad".to_owned(), "boom".to_owned())])
        );
        #[cfg(unix)]
        assert!(root.join(".dx/local/ide/typescript/lib").exists());
        assert!(
            refresh_with(root, &runner, &[&bad, &typescript::PLUGIN])
                .expect("retry")
                .ran
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn a_deleted_output_reruns_the_refresh_and_is_reported() {
        let scratch = dx_test_scratch::scratch("dx-ide-missing-");
        let root = scratch.path();
        std::fs::write(root.join("a.ts"), "").expect("ts");
        let runner = fake(root);
        refresh(root, &runner).expect("first");
        assert_eq!(missing_outputs(root), Vec::<String>::new());
        std::fs::remove_file(root.join(".dx/local/ide/typescript/lib")).expect("delete");
        assert_eq!(missing_outputs(root), [".dx/local/ide/typescript/lib"]);
        assert!(refresh(root, &runner).expect("second").ran);
        assert_eq!(missing_outputs(root), Vec::<String>::new());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn typescript_needs_a_ts_file_and_stale_dirs_go() {
        let scratch = dx_test_scratch::scratch("dx-ide-ts-");
        let root = scratch.path();
        let runner = fake(root);
        std::fs::create_dir_all(root.join(".dx/local/ide/typescript")).expect("stale");
        std::fs::create_dir_all(root.join(".dx/local/ide/rust")).expect("rust");
        refresh(root, &runner).expect("empty tree");
        assert!(!root.join(".dx/local/ide/typescript").exists());
        assert!(root.join(".dx/local/ide/rust").exists());
        assert_eq!(runner.calls.get(), 0);
        std::fs::create_dir_all(root.join("src")).expect("src");
        std::fs::write(root.join("src/app.tsx"), "").expect("tsx");
        refresh(root, &runner).expect("with ts");
        assert_eq!(runner.calls.get(), 1);
        #[cfg(unix)]
        assert_eq!(
            std::fs::read_link(root.join(".dx/local/ide/typescript/lib")).expect("link"),
            runner.lib.join("lib")
        );
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn python_links_the_interpreter_and_a_rerun_is_a_no_op() {
        let scratch = dx_test_scratch::scratch("dx-ide-py-");
        let root = scratch.path();
        std::fs::write(root.join("main.py"), "").expect("py");
        let interpreter = root.join("store").join("python3");
        std::fs::create_dir_all(root.join("store")).expect("store");
        std::fs::write(&interpreter, "").expect("interpreter");
        let runner = Fake {
            calls: Cell::new(0),
            lib: interpreter.clone(),
            aquery: String::new(),
            groups: Vec::new(),
        };
        assert!(refresh(root, &runner).expect("first").ran);
        assert_eq!(
            std::fs::read_link(root.join(".dx/local/ide/python/python")).expect("link"),
            interpreter
        );
        assert!(!refresh(root, &runner).expect("second").ran);
        assert_eq!(runner.calls.get(), 1);
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn python_links_the_locked_packages_so_they_import() {
        let scratch = dx_test_scratch::scratch("dx-ide-py-site-");
        let root = scratch.path();
        std::fs::write(root.join("main.py"), "").expect("py");
        std::fs::write(
            root.join("uv.lock"),
            "[[package]]\nname = \"requests\"\nsource = { registry = \"x\" }\n",
        )
        .expect("lock");
        let install = root.join("store").join("requests.install");
        let site = install.join("lib/python3.12/site-packages");
        std::fs::create_dir_all(site.join("requests")).expect("site");
        std::fs::write(site.join("requests/__init__.py"), "").expect("module");
        let interpreter = root.join("store").join("python3");
        std::fs::write(&interpreter, "").expect("interpreter");
        let runner = Fake {
            calls: Cell::new(0),
            lib: interpreter,
            aquery: String::new(),
            groups: vec![install],
        };
        let report = refresh(root, &runner).expect("refresh");
        assert_eq!(report.failed, []);
        let linked = root.join(".dx/local/ide/python/site-packages");
        assert!(linked.join("requests/__init__.py").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn jdk_links_the_home_for_java_and_kotlin_and_reruns_as_a_no_op() {
        let scratch = dx_test_scratch::scratch("dx-ide-jdk-");
        let root = scratch.path();
        let mut runner = fake(root);
        let home = root.join("store").join("jdk");
        std::fs::create_dir_all(home.join("bin")).expect("bin");
        std::fs::write(home.join("bin/java"), "").expect("java");
        runner.lib = home.join("bin/java");
        refresh(root, &runner).expect("no java");
        assert!(!root.join(".dx/local/ide/jdk").exists());
        std::fs::write(root.join("App.kt"), "").expect("kt");
        let first = refresh(root, &runner).expect("first");
        assert_eq!(first.failed, []);
        assert_eq!(
            std::fs::read_link(root.join(".dx/local/ide/jdk/home")).expect("link"),
            home
        );
        assert!(!refresh(root, &runner).expect("second").ran);
        std::fs::remove_file(root.join("App.kt")).expect("rm");
        std::fs::remove_dir_all(root.join(".dx/local/ide")).expect("rm ide");
        std::fs::write(root.join("App.scala"), "").expect("scala");
        assert_eq!(refresh(root, &runner).expect("scala").failed, []);
        assert!(root.join(".dx/local/ide/jdk/home").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(unix)]
    fn go_writes_an_executable_driver_wrapper_and_a_rerun_is_a_no_op() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = dx_test_scratch::scratch("dx-ide-go-");
        let root = scratch.path();
        std::fs::write(root.join("main.go"), "").expect("go");
        let mut runner = fake(root);
        runner.lib = root.join("store").join("gopackagesdriver");
        let first = refresh(root, &runner).expect("first");
        assert_eq!(first.failed, []);
        let path = root.join(".dx/local/ide/go/gopackagesdriver");
        let text = std::fs::read_to_string(&path).expect("wrapper");
        assert!(text.contains("BUILD_WORKSPACE_DIRECTORY="), "{text}");
        assert!(text.contains("store/gopackagesdriver"), "{text}");
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        assert!(!refresh(root, &runner).expect("second").ran);
        assert_eq!(runner.calls.get(), 1);
        assert_eq!(missing_outputs(root), Vec::<String>::new());
        scratch.close().expect("cleanup");
    }

    #[test]
    #[cfg(windows)]
    fn go_writes_a_cmd_driver_wrapper_that_runs() {
        let scratch = dx_test_scratch::scratch("dx-ide-go-");
        let root = scratch.path().join("a%b");
        std::fs::create_dir_all(root.join("store")).expect("store");
        std::fs::write(root.join("main.go"), "").expect("go");
        let driver = root.join("store").join("gopackagesdriver.exe");
        std::fs::write(&driver, "").expect("driver");
        let mut runner = fake(&root);
        runner.lib = driver;
        let first = refresh(&root, &runner).expect("first");
        assert_eq!(first.failed, []);
        let dir = root.join(".dx/local/ide/go");
        let wrapper = dir.join("gopackagesdriver.cmd");
        let text = std::fs::read_to_string(&wrapper).expect("wrapper");
        assert!(text.contains("BUILD_WORKSPACE_DIRECTORY="), "{text}");
        assert!(text.contains("a%%b"), "{text}");
        assert!(!dir.join("gopackagesdriver").exists());
        assert_eq!(missing_outputs(&root), Vec::<String>::new());
        assert!(!refresh(&root, &runner).expect("second").ran);
        scratch.close().expect("cleanup");
    }

    #[cfg(unix)]
    struct RustRunner {
        files: Vec<PathBuf>,
    }

    #[cfg(unix)]
    impl Runner for RustRunner {
        fn build(&self, _labels: &[String]) -> Result<Vec<PathBuf>, String> {
            Ok(self.files.clone())
        }

        fn build_group(&self, _labels: &[String], _group: &str) -> Result<Vec<PathBuf>, String> {
            Ok(Vec::new())
        }

        fn aquery(&self, _expr: &str) -> Result<String, String> {
            Ok(String::new())
        }

        fn execution_root(&self) -> Result<PathBuf, String> {
            Ok(PathBuf::new())
        }
    }

    #[test]
    #[cfg(unix)]
    fn rust_analyzer_copies_the_launchers_and_a_rerun_is_a_no_op() {
        use std::os::unix::fs::PermissionsExt;
        let scratch = dx_test_scratch::scratch("dx-ide-rust-");
        let root = scratch.path();
        std::fs::write(root.join("lib.rs"), "").expect("rs");
        let store = root.join("store");
        std::fs::create_dir_all(&store).expect("store");
        let flycheck = store.join("flycheck");
        let discover = store.join("discover_bazel_rust_project");
        std::fs::write(&flycheck, "fly").expect("flycheck");
        std::fs::write(&discover, "disc").expect("discover");
        std::fs::create_dir_all(root.join(".dx/local/ide/rust")).expect("base");
        std::fs::write(root.join(".dx/local/ide/rust/keep"), "").expect("keep");
        let runner = RustRunner {
            files: vec![flycheck, discover],
        };
        let first = refresh(root, &runner).expect("first");
        assert_eq!(first.failed, []);
        let dir = root.join(".dx/local/ide/rust_analyzer");
        for (name, text) in [
            ("flycheck.exe", "fly"),
            ("discover_bazel_rust_project.exe", "disc"),
        ] {
            let path = dir.join(name);
            let meta = std::fs::symlink_metadata(&path).expect("meta");
            assert!(!meta.file_type().is_symlink(), "{name}");
            assert_eq!(meta.permissions().mode() & 0o111, 0o111, "{name}");
            assert_eq!(std::fs::read_to_string(&path).expect("read"), text);
        }
        assert_eq!(
            std::fs::read_to_string(dir.join("user_config.json")).expect("config"),
            "{\"clippy\": true}\n"
        );
        assert!(root.join(".dx/local/ide/rust/keep").exists());
        assert!(!refresh(root, &runner).expect("second").ran);
        scratch.close().expect("cleanup");
    }
}
