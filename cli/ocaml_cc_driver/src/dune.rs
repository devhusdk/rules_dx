//! Runs one dune step in a scratch copy of the sources with the toolchain environment.
//!
//! `ocaml_cc_driver dune MODE ARG...` takes the toolchain variables from its environment, where
//! `$ROOT` stands for the execution root, and paths in ARG relative to that root.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What one dune step builds.
#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Bootstrap {
        source: String,
        output: String,
    },
    Build {
        workspace: String,
        dune: String,
        target: String,
        output: String,
    },
    Library {
        workspace: String,
        dune: String,
        package: String,
        stubs_name: String,
        stubs: Option<String>,
        tree: String,
        archives: Vec<(String, String)>,
    },
}

/// A parsed command line: the step, the job count and the OCaml compiler directory.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub step: Step,
    pub jobs: String,
    pub ocaml_bin: String,
}

fn flags(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let name = flag
            .strip_prefix("--")
            .ok_or_else(|| format!("unexpected argument '{flag}'"))?;
        let value = iter
            .next()
            .ok_or_else(|| format!("--{name} wants a value"))?;
        found
            .entry(name.to_owned())
            .or_default()
            .push(value.clone());
    }
    Ok(found)
}

fn one(found: &BTreeMap<String, Vec<String>>, name: &str) -> Result<String, String> {
    found
        .get(name)
        .and_then(|values| values.first())
        .cloned()
        .ok_or_else(|| format!("--{name} is required"))
}

fn pairs(
    found: &BTreeMap<String, Vec<String>>,
    name: &str,
) -> Result<Vec<(String, String)>, String> {
    found
        .get(name)
        .into_iter()
        .flatten()
        .map(|value| {
            value
                .split_once('=')
                .map(|(from, to)| (from.to_owned(), to.to_owned()))
                .ok_or_else(|| format!("--{name} wants FROM=TO, got '{value}'"))
        })
        .collect()
}

/// Parses `MODE --flag VALUE...` into a request.
pub fn parse(args: &[String]) -> Result<Request, String> {
    let (mode, rest) = args.split_first().ok_or("a mode is required")?;
    let found = flags(rest)?;
    let step = match mode.as_str() {
        "bootstrap" => Step::Bootstrap {
            source: one(&found, "source")?,
            output: one(&found, "output")?,
        },
        "build" => Step::Build {
            workspace: one(&found, "workspace")?,
            dune: one(&found, "dune")?,
            target: one(&found, "target")?,
            output: one(&found, "output")?,
        },
        "library" => Step::Library {
            workspace: one(&found, "workspace")?,
            dune: one(&found, "dune")?,
            package: one(&found, "package")?,
            stubs_name: one(&found, "stubs-name")?,
            stubs: found.get("stubs").and_then(|v| v.first()).cloned(),
            tree: one(&found, "tree")?,
            archives: pairs(&found, "archive")?,
        },
        other => return Err(format!("unknown mode '{other}'")),
    };
    Ok(Request {
        step,
        jobs: one(&found, "jobs")?,
        ocaml_bin: one(&found, "ocaml-bin")?,
    })
}

/// Drops the `(vendored_dirs` lines of a dune file; returns None when it has none.
pub fn without_vendored_dirs(text: &str) -> Option<String> {
    if !text.lines().any(|line| line.starts_with("(vendored_dirs")) {
        return None;
    }
    Some(
        text.lines()
            .filter(|line| !line.starts_with("(vendored_dirs"))
            .map(|line| format!("{line}\n"))
            .collect(),
    )
}

fn io(context: &str, error: std::io::Error) -> String {
    format!("{context}: {error}")
}

#[cfg(unix)]
fn add_write(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.file_type().is_symlink() {
            let mut permissions = meta.permissions();
            permissions.set_mode(permissions.mode() | 0o700);
            let _ = fs::set_permissions(path, permissions);
        }
    }
}

#[cfg(not(unix))]
fn add_write(_path: &Path) {}

/// Makes a tree writable so it can be removed.
fn make_writable(path: &Path) {
    add_write(path);
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    make_writable(&entry.path());
                }
            }
        }
    }
}

/// Copies `from` into `to`, following symlinks, and leaves the copies writable.
pub fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    let meta = fs::metadata(from).map_err(|e| io(&from.display().to_string(), e))?;
    if meta.is_dir() {
        fs::create_dir_all(to).map_err(|e| io(&to.display().to_string(), e))?;
        add_write(to);
        for entry in fs::read_dir(from).map_err(|e| io(&from.display().to_string(), e))? {
            let entry = entry.map_err(|e| io(&from.display().to_string(), e))?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else {
        if to.exists() {
            make_writable(to);
            fs::remove_file(to).map_err(|e| io(&to.display().to_string(), e))?;
        }
        fs::copy(from, to).map_err(|e| io(&to.display().to_string(), e))?;
        add_write(to);
    }
    Ok(())
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        make_writable(&self.0);
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch_dir() -> Result<Scratch, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("dx_dune.{}.{nanos}", std::process::id()));
    fs::create_dir_all(&dir).map_err(|e| io(&dir.display().to_string(), e))?;
    Ok(Scratch(dir))
}

fn run(
    program: &Path,
    args: &[&str],
    dir: &Path,
    env: &BTreeMap<String, String>,
) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env)
        .status()
        .map_err(|e| io(&program.display().to_string(), e))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} failed with {status}", program.display()))
    }
}

/// The environment of the step: the inherited variables with `$ROOT` expanded, plus the scratch ones.
fn step_env(
    root: &Path,
    scratch: &Path,
    ocaml_bin: &Path,
) -> Result<BTreeMap<String, String>, String> {
    let root_text = root.to_string_lossy();
    let mut env: BTreeMap<String, String> = std::env::vars()
        .map(|(key, value)| (key, value.replace("$ROOT", &root_text)))
        .collect();
    let home = scratch.join("home");
    let tmp = scratch.join("tmp");
    for dir in [&home, &tmp] {
        fs::create_dir_all(dir).map_err(|e| io(&dir.display().to_string(), e))?;
    }
    let set = |env: &mut BTreeMap<String, String>, key: &str, value: &Path| {
        env.insert(key.to_owned(), value.to_string_lossy().into_owned());
    };
    env.insert("DUNE_CACHE".to_owned(), "disabled".to_owned());
    set(&mut env, "HOME", &home);
    set(&mut env, "XDG_CACHE_HOME", &home.join(".cache"));
    set(&mut env, "XDG_CONFIG_HOME", &home.join(".config"));
    set(&mut env, "XDG_DATA_HOME", &home.join(".local"));
    set(&mut env, "TMPDIR", &tmp);
    if cfg!(target_os = "macos") {
        let sdk = Command::new("/usr/bin/xcrun")
            .arg("--show-sdk-path")
            .env_clear()
            .env("PATH", "/dx_no_path")
            .output()
            .map_err(|e| io("/usr/bin/xcrun", e))?;
        env.insert(
            "SDKROOT".to_owned(),
            String::from_utf8_lossy(&sdk.stdout).trim().to_owned(),
        );
        let lib_dir = ocaml_bin.join("../lib");
        let lib = fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
        let mut mkexe = env.get("CONDA_OCAML_MKEXE").cloned().unwrap_or_default();
        let zstd = lib.join("libzstd.a");
        if zstd.is_file() {
            let static_dir = scratch.join("static");
            fs::create_dir_all(&static_dir).map_err(|e| io("static", e))?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&zstd, static_dir.join("libzstd.a"))
                .map_err(|e| io("libzstd.a", e))?;
            mkexe.push_str(&format!(
                " -Wl,-search_paths_first -L{}",
                static_dir.display()
            ));
        }
        mkexe.push_str(&format!(" -Wl,-rpath,{}", lib.display()));
        env.insert("CONDA_OCAML_MKEXE".to_owned(), mkexe);
    } else {
        let lib_dir = ocaml_bin.join("../lib");
        let lib = fs::canonicalize(&lib_dir).unwrap_or(lib_dir);
        let mkexe = env.get("CONDA_OCAML_MKEXE").cloned().unwrap_or_default();
        env.insert(
            "CONDA_OCAML_MKEXE".to_owned(),
            format!("{mkexe} -L{} -Wl,-rpath,{}", lib.display(), lib.display()),
        );
    }
    Ok(env)
}

fn require_file(path: &Path, message: &str) -> Result<(), String> {
    if path.is_file() {
        Ok(())
    } else {
        Err(format!("dx_dune_library: {} {message}", path.display()))
    }
}

/// Runs the step in the current directory, the execution root.
pub fn execute(request: &Request) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|e| io("current directory", e))?;
    let scratch = scratch_dir()?;
    let ocaml_bin = root.join(&request.ocaml_bin);
    let env = step_env(&root, &scratch.0, &ocaml_bin)?;
    let jobs = request.jobs.as_str();
    match &request.step {
        Step::Bootstrap { source, output } => {
            let dir = scratch.0.join("dune");
            copy_tree(&root.join(source), &dir)?;
            run(
                &ocaml_bin.join("ocaml"),
                &["boot/bootstrap.ml", "-j", jobs],
                &dir,
                &env,
            )?;
            copy_tree(&dir.join("_boot/dune.exe"), &root.join(output))
        }
        Step::Build {
            workspace,
            dune,
            target,
            output,
        } => {
            let dir = scratch.0.join("ws");
            copy_tree(&root.join(workspace), &dir)?;
            let goal = format!("./{target}");
            run(
                &root.join(dune),
                &["build", "--release", "--display=quiet", "-j", jobs, &goal],
                &dir,
                &env,
            )?;
            copy_tree(&dir.join("_build/default").join(target), &root.join(output))
        }
        Step::Library {
            workspace,
            dune,
            package,
            stubs_name,
            stubs,
            tree,
            archives,
        } => {
            let dir = scratch.0.join("ws");
            copy_tree(&root.join(workspace), &dir)?;
            let dune_file = dir.join("dune");
            let vendored = fs::read_to_string(&dune_file)
                .ok()
                .and_then(|text| without_vendored_dirs(&text));
            let install = format!("@{package}/install");
            let args: Vec<&str> = if let Some(text) = vendored {
                fs::write(&dune_file, text).map_err(|e| io("dune", e))?;
                vec!["build", "--display=quiet", "-j", jobs, &install]
            } else {
                vec![
                    "build",
                    "--display=quiet",
                    "-j",
                    jobs,
                    "-p",
                    package,
                    "@install",
                ]
            };
            run(&root.join(dune), &args, &dir, &env)?;
            let lib = dir.join("_build/install/default/lib").join(package);
            let built = lib.join(stubs_name);
            match stubs {
                Some(output) => {
                    require_file(
                        &built,
                        "was not built; stubs = True needs the package's C stubs",
                    )?;
                    copy_tree(&built, &root.join(output))?;
                }
                None if built.is_file() => {
                    return Err(format!(
                        "dx_dune_library: {} holds C stubs; set stubs = True to link them",
                        built.display()
                    ));
                }
                None => {}
            }
            for (name, _) in archives {
                require_file(
                    &lib.join(name),
                    "was not built; set library = to the dune (name) of the library",
                )?;
            }
            copy_tree(&lib, &root.join(tree))?;
            for (name, output) in archives {
                copy_tree(&lib.join(name), &root.join(output))?;
            }
            Ok(())
        }
    }
}

/// Parses the arguments after `dune` and runs the step.
pub fn run_from_args(args: &[String]) -> Result<i32, String> {
    execute(&parse(args)?)?;
    Ok(0)
}
