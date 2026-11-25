#![deny(clippy::indexing_slicing, clippy::string_slice)]

use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

#[cfg(test)]
mod lib_tests;

pub const CONFIG: &str = "dx_ghc_musl.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub loader: String,
    pub library_path: Vec<String>,
    pub tools: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Plan {
    pub loader: PathBuf,
    pub args: Vec<OsString>,
}

pub fn repo_root(wrapper_dir: &Path) -> Result<PathBuf, String> {
    let package = wrapper_dir
        .parent()
        .ok_or_else(|| format!("{} has no parent", wrapper_dir.display()))?;
    let parts: Vec<Component<'_>> = package.components().collect();
    if let [prefix @ .., Component::Normal(out), _, Component::Normal(bin), Component::Normal(external), repo] =
        parts.as_slice()
        && *out == "bazel-out"
        && *bin == "bin"
        && *external == "external"
    {
        let mut root: PathBuf = prefix.iter().collect();
        root.push("external");
        root.push(repo);
        return Ok(root);
    }
    Ok(package.to_path_buf())
}

pub fn plan(
    argv0: &Path,
    config: &Config,
    root: &Path,
    args: Vec<OsString>,
) -> Result<Plan, String> {
    let name = argv0
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("{} has no file name", argv0.display()))?;
    let extra = config
        .tools
        .get(name)
        .ok_or_else(|| format!("{CONFIG} has no tool {name}"))?;
    let bin = argv0
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| format!("{} has no UTF-8 directory", argv0.display()))?;
    let root_text = root
        .to_str()
        .ok_or_else(|| format!("{} is not UTF-8", root.display()))?;
    let library_path = std::env::join_paths(config.library_path.iter().map(|dir| root.join(dir)))
        .map_err(|error| format!("{CONFIG} library_path: {error}"))?;
    let mut planned = vec![
        OsString::from("--library-path"),
        library_path,
        root.join("bin").join(name).into_os_string(),
    ];
    planned.extend(
        extra
            .iter()
            .map(|arg| OsString::from(arg.replace("{root}", root_text).replace("{bin}", bin))),
    );
    planned.extend(args);
    Ok(Plan {
        loader: root.join(&config.loader),
        args: planned,
    })
}

fn resolve(argv0: &OsStr, args: Vec<OsString>) -> Result<Plan, String> {
    let argv0 = Path::new(argv0);
    if argv0.parent().is_none_or(|dir| dir.as_os_str().is_empty()) {
        return Err(format!(
            "{} must be started by its path, not from PATH",
            argv0.display()
        ));
    }
    let argv0 = std::path::absolute(argv0)
        .map_err(|error| format!("{}: {error}", argv0.display()))?;
    let wrapper_dir = argv0
        .parent()
        .ok_or_else(|| format!("{} has no directory", argv0.display()))?;
    let root = repo_root(wrapper_dir)?;
    let path = root.join(CONFIG);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let config: Config = serde_json::from_str(&text)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    plan(&argv0, &config, &root, args)
}

#[cfg(unix)]
fn exec(plan: Plan) -> String {
    use std::os::unix::process::CommandExt;
    let error = std::process::Command::new(&plan.loader)
        .args(plan.args)
        .exec();
    format!("cannot run {}: {error}", plan.loader.display())
}

#[cfg(not(unix))]
fn exec(plan: Plan) -> String {
    format!(
        "{} runs only on Linux exec hosts",
        plan.loader.display()
    )
}

#[must_use]
pub fn run_tool(argv0: &OsStr, args: Vec<OsString>) -> i32 {
    let message = match resolve(argv0, args) {
        Ok(plan) => exec(plan),
        Err(error) => error,
    };
    eprintln!("ghc_musl: {message}");
    1
}
