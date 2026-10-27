use std::path::Path;

use std::collections::{BTreeMap, BTreeSet};

use super::lock_inputs::{self, Hub, LockChange, LockKind};
use super::module_inputs;
use super::{PackageCache, QueryRunner, ResolveError, RuleGraph};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidenedScope {
    pub scope: String,
    pub reason: String,
    pub repos: Vec<String>,
}

#[derive(Debug)]
pub struct AffectedSelection {
    pub targets: Vec<String>,
    pub widened: Vec<WidenedScope>,
    pub graph: Option<RuleGraph>,
}

fn git_failure(what: &str, detail: String) -> String {
    format!("git {what} failed: {detail}")
}

fn first_diagnostic(bytes: &[u8]) -> String {
    dx_output::first_diagnostic_line(bytes, 200, "no Git diagnostic")
}

fn git_dir_of(workspace: &Path) -> Option<std::path::PathBuf> {
    let dot_git = workspace.join(".git");
    if dot_git.is_dir() {
        return Some(dot_git);
    }
    let pointer = std::fs::read_to_string(&dot_git).ok()?;
    let gitdir = pointer.strip_prefix("gitdir: ")?.trim();
    let gitdir_path = Path::new(gitdir);
    if gitdir_path.is_absolute() {
        return Some(gitdir_path.to_path_buf());
    }
    Some(workspace.join(gitdir_path))
}

fn shallow_clone(workspace: &Path) -> bool {
    let Some(git_dir) = git_dir_of(workspace) else {
        return false;
    };
    if git_dir.join("shallow").is_file() {
        return true;
    }
    let Ok(commondir) = std::fs::read_to_string(git_dir.join("commondir")) else {
        return false;
    };
    git_dir.join(commondir.trim()).join("shallow").is_file()
}

fn shallow_error() -> String {
    "git history is shallow: fetch full history with `git fetch --unshallow` or pass explicit scopes".to_owned()
}

pub fn diff_name_status(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    extra: &[String],
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(extra.iter().cloned());
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("diff", error.to_string()))?;
    if result.code != Some(0) {
        if shallow_clone(workspace) {
            return Err(shallow_error());
        }
        return Err(git_failure("diff", first_diagnostic(&result.stderr)));
    }
    dx_adopt::parse_name_status_nul(&result.stdout)
}

pub fn merge_base(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    first: &str,
    second: &str,
) -> Result<String, String> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(
        ["merge-base", first, second]
            .iter()
            .map(ToString::to_string),
    );
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("merge-base", error.to_string()))?;
    if result.code != Some(0) {
        if shallow_clone(workspace) {
            return Err(shallow_error());
        }
        return Err(git_failure("merge-base", first_diagnostic(&result.stderr)));
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|_| git_failure("merge-base", "output is not UTF-8".to_owned()))?;
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let base = lines
        .next()
        .ok_or_else(|| git_failure("merge-base", "output names no base commit".to_owned()))?;
    if lines.next().is_some() {
        return Err(git_failure(
            "merge-base",
            "output names more than one base commit".to_owned(),
        ));
    }
    Ok(base.to_owned())
}

fn parse_untracked(output: &[u8]) -> Result<Vec<dx_adopt::GitChange>, String> {
    let mut fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    let mut changes = Vec::with_capacity(fields.len());
    for field in fields {
        let path = std::str::from_utf8(field)
            .map_err(|_| "git path is not UTF-8: rename the file to a UTF-8 name".to_owned())?;
        if path.is_empty() {
            return Err("git change list has an empty path".to_owned());
        }
        if dx_adopt::is_dx_state(path) {
            continue;
        }
        changes.push(dx_adopt::GitChange {
            path: path.to_owned(),
            from: None,
            kind: dx_adopt::ChangeKind::Added,
        });
    }
    Ok(changes)
}

pub fn collect_working_tree(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let staged = diff_name_status(
        git,
        workspace,
        runner,
        &[
            "diff".to_owned(),
            "--cached".to_owned(),
            "--relative".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
        ],
    )?;
    let unstaged = diff_name_status(
        git,
        workspace,
        runner,
        &[
            "diff".to_owned(),
            "--relative".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
        ],
    )?;
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(
        ["ls-files", "--others", "--exclude-standard", "-z"]
            .iter()
            .map(ToString::to_string),
    );
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("ls-files", error.to_string()))?;
    if result.code != Some(0) {
        return Err(git_failure("ls-files", first_diagnostic(&result.stderr)));
    }
    let untracked = parse_untracked(&result.stdout)?;
    let mut changes = staged;
    changes.extend(unstaged);
    changes.extend(untracked);
    Ok(dx_adopt::dedupe_changes(changes))
}

pub fn collect_range(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: &str,
    head: &str,
) -> Result<Vec<dx_adopt::GitChange>, String> {
    let changes = diff_name_status(
        git,
        workspace,
        runner,
        &[
            "diff".to_owned(),
            "--relative".to_owned(),
            "--name-status".to_owned(),
            "-z".to_owned(),
            base.to_owned(),
            head.to_owned(),
        ],
    )?;
    Ok(dx_adopt::dedupe_changes(changes))
}

fn run_git_lines(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    what: &str,
    args: &[String],
) -> Result<Vec<String>, String> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(args.iter().cloned());
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure(what, error.to_string()))?;
    if result.code != Some(0) {
        return Err(git_failure(what, first_diagnostic(&result.stderr)));
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|_| "git path is not UTF-8: rename the file to a UTF-8 name".to_owned())?;
    Ok(text
        .split(['\0', '\n'])
        .filter(|field| !field.is_empty())
        .map(str::to_owned)
        .collect())
}

fn repo_relative(prefix: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = prefix.split('/').filter(|part| !part.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

pub fn local_override_changes(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: &str,
) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(workspace.join("MODULE.bazel")).unwrap_or_default();
    let overrides: Vec<String> = dx_adopt::local_path_overrides(&text)
        .into_iter()
        .filter(|path| !Path::new(path).is_absolute() && path.split('/').any(|part| part == ".."))
        .collect();
    if overrides.is_empty() {
        return Ok(Vec::new());
    }
    let prefix = run_git_lines(
        git,
        workspace,
        runner,
        "rev-parse",
        &["rev-parse".to_owned(), "--show-prefix".to_owned()],
    )?
    .into_iter()
    .next()
    .unwrap_or_default();
    let prefix = prefix.trim_end_matches('/');
    let mut roots: Vec<(String, String)> = Vec::new();
    for path in overrides {
        let Some(root) = repo_relative(prefix, &path) else {
            continue;
        };
        let inside = prefix.is_empty() || root == prefix || root.starts_with(&format!("{prefix}/"));
        if !root.is_empty() && !inside {
            roots.push((root, path.trim_end_matches('/').to_owned()));
        }
    }
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let pathspecs: Vec<String> = roots
        .iter()
        .map(|(root, _)| format!(":(top){root}"))
        .collect();
    let mut diff = vec![
        "diff".to_owned(),
        "--name-only".to_owned(),
        "-z".to_owned(),
        base.to_owned(),
        "--".to_owned(),
    ];
    diff.extend(pathspecs.iter().cloned());
    let mut untracked = vec![
        "ls-files".to_owned(),
        "--others".to_owned(),
        "--exclude-standard".to_owned(),
        "--full-name".to_owned(),
        "-z".to_owned(),
        "--".to_owned(),
    ];
    untracked.extend(pathspecs);
    let mut changed = run_git_lines(git, workspace, runner, "diff", &diff)?;
    changed.extend(run_git_lines(
        git, workspace, runner, "ls-files", &untracked,
    )?);
    let mut paths: Vec<String> = changed
        .iter()
        .filter_map(|file| {
            if !prefix.is_empty()
                && file
                    .strip_prefix(&format!("{prefix}/"))
                    .is_some_and(dx_adopt::is_dx_state)
            {
                return None;
            }
            roots.iter().find_map(|(root, shown)| {
                file.strip_prefix(&format!("{root}/"))
                    .filter(|rest| !dx_adopt::is_dx_state(rest))
                    .map(|rest| format!("{shown}/{rest}"))
            })
        })
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Workspace-relative paths named by `import` and `try-import %workspace%/...` lines.
fn rc_import_paths(content: &str) -> Vec<String> {
    let joined = content.replace("\\\r\n", " ").replace("\\\n", " ");
    let mut out = Vec::new();
    for line in joined.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        if !matches!(tokens.next(), Some("import" | "try-import")) {
            continue;
        }
        let Some(raw) = tokens.next() else { continue };
        let raw = raw
            .strip_prefix('"')
            .and_then(|r| r.strip_suffix('"'))
            .or_else(|| raw.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
            .unwrap_or(raw);
        let Some(rest) = raw.strip_prefix("%workspace%/") else {
            continue;
        };
        let parts: Vec<&str> = rest
            .split('/')
            .filter(|p| !p.is_empty() && *p != ".")
            .collect();
        if parts.is_empty() || parts.contains(&"..") {
            continue;
        }
        out.push(parts.join("/"));
    }
    out
}

/// Files the root `.bazelrc` reaches through workspace imports.
fn workspace_rc_files(workspace: &Path) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut visited: BTreeSet<String> = BTreeSet::from([".bazelrc".to_owned()]);
    let mut stack = vec![".bazelrc".to_owned()];
    while let Some(current) = stack.pop() {
        let Ok(content) = std::fs::read_to_string(workspace.join(&current)) else {
            continue;
        };
        for next in rc_import_paths(&content) {
            if visited.insert(next.clone()) {
                seen.insert(next.clone());
                stack.push(next);
            }
        }
    }
    seen
}

fn is_workspace_input(path: &str, rc_files: &BTreeSet<String>) -> bool {
    let name = file_name_of(path);
    matches!(
        name,
        "MODULE.bazel" | "MODULE.bazel.lock" | "WORKSPACE" | "WORKSPACE.bazel" | ".bazelversion"
    ) || name.starts_with(".bazelrc")
        || (name.ends_with(".MODULE.bazel") && !name.starts_with('.'))
        || rc_files.contains(path)
}

fn is_lock_input(path: &str, hubs: &[Hub]) -> bool {
    let name = file_name_of(path);
    (name.ends_with(".lock") && name != "MODULE.bazel.lock")
        || lock_inputs::classify(path, hubs) != LockKind::Unknown
}

const FALLBACK_INPUTS: &[&str] = &[
    "pnpm-lock.yaml",
    "package-lock.json",
    "go.mod",
    "go.sum",
    "go.work",
    "go.work.sum",
    "maven_install.json",
    "requirements_lock.txt",
];

struct ModuleInputPaths {
    paths: BTreeSet<String>,
    fallback: bool,
}

impl ModuleInputPaths {
    fn contains(&self, path: &str) -> bool {
        self.paths.contains(path)
            || (self.fallback && FALLBACK_INPUTS.contains(&file_name_of(path)))
    }
}

/// Lock and manifest inputs of the crate extension are handled by the lock
/// narrowing; other tags (such as `crate.annotation` patches and
/// `additive_build_file`) change repo contents and widen like any input.
fn is_crate_lock_input(extension: &Option<(String, String)>, tag: &str) -> bool {
    extension.as_ref().is_some_and(|(bzl, name)| {
        bzl.ends_with(lock_inputs::CRATE_EXTENSION_BZL)
            && name == "crate"
            && matches!(tag, "from_cargo" | "from_specs")
    })
}

fn module_inputs_of(workspace: &Path) -> (Vec<Hub>, ModuleInputPaths) {
    let hubs_of = |path: &Path| {
        std::fs::read_to_string(path).ok().and_then(|text| {
            let mut hubs = lock_inputs::crate_hubs(&text).ok()?;
            hubs.extend(lock_inputs::pip_hubs(&text));
            hubs.extend(lock_inputs::maven_hubs(&text));
            Some(hubs)
        })
    };
    let root_hubs = hubs_of(&workspace.join("MODULE.bazel"));
    let mut hubs = root_hubs.clone().unwrap_or_default();
    let mut segments_ok = root_hubs.is_some();
    let mut inputs = ModuleInputPaths {
        paths: BTreeSet::new(),
        fallback: true,
    };
    if let Ok(scanned) = dx_adopt::module_inputs(workspace) {
        inputs.fallback = !scanned.complete;
        for file in scanned.files {
            if file.tag == "include" {
                match hubs_of(&workspace.join(&file.path)) {
                    Some(found) => hubs.extend(found),
                    None => segments_ok = false,
                }
            }
            if is_crate_lock_input(&file.extension, &file.tag) {
                continue;
            }
            let sibling = match file_name_of(&file.path) {
                "go.mod" => Some("go.sum"),
                "go.work" => Some("go.work.sum"),
                _ => None,
            };
            if let Some(sibling) = sibling {
                let dir = &file.path[..file.path.len() - file_name_of(&file.path).len()];
                inputs.paths.insert(format!("{dir}{sibling}"));
            }
            inputs.paths.insert(file.path);
        }
    }
    if !segments_ok {
        hubs.clear();
    }
    (hubs, inputs)
}

/// Text of a file at the base commit, or None when it is absent there.
pub(crate) fn show_at_base(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: &str,
    path: &str,
) -> Result<Option<String>, String> {
    let argv = vec![
        git.to_string_lossy().into_owned(),
        "show".to_owned(),
        format!("{base}:./{path}"),
    ];
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("show", error.to_string()))?;
    if result.code != Some(0) {
        let stderr = String::from_utf8_lossy(&result.stderr);
        if stderr.contains("does not exist") || stderr.contains("exists on disk, but not in") {
            return Ok(None);
        }
        return Err(git_failure("show", first_diagnostic(&result.stderr)));
    }
    String::from_utf8(result.stdout)
        .map(Some)
        .map_err(|_| git_failure("show", "output is not UTF-8".to_owned()))
}

fn lock_change(
    path: &str,
    hubs: &[Hub],
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: Option<(&Path, &str)>,
) -> Result<Vec<(String, LockChange)>, String> {
    let kind = lock_inputs::classify(path, hubs);
    let workspace_input = || format!("workspace input {path}");
    if kind == LockKind::Unknown {
        return Err(workspace_input());
    }
    let Some((git, base)) = base else {
        return Err(workspace_input());
    };
    if !workspace.join(path).is_file() {
        return Err(workspace_input());
    }
    let head = std::fs::read_to_string(workspace.join(path))
        .map_err(|error| format!("cannot read {path}: {error}"))?;
    let base = show_at_base(git, workspace, runner, base, path)
        .map_err(|detail| format!("cannot read {path}: {detail}"))?
        .ok_or_else(workspace_input)?;
    let changed = if matches!(kind, LockKind::Go(_)) {
        lock_inputs::changed_go(path, &base, &head)
    } else {
        kind.changed(&base, &head)
    }
    .map_err(|detail| format!("cannot read {path}: {detail}"))?;
    Ok(kind
        .hubs()
        .iter()
        .map(|hub| (hub.clone(), changed.clone()))
        .collect())
}

const ROOT_MODULE: &str = "MODULE.bazel";
const ROOT_LOCK: &str = "MODULE.bazel.lock";

/// The modules a root MODULE.bazel bump changes, and the repositories they own.
struct ModuleBump {
    path: String,
    modules: BTreeSet<String>,
    loads: Vec<String>,
}

fn module_text(
    path: &str,
    changed: bool,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: (&Path, &str),
) -> Result<(String, String), String> {
    let workspace_input = || format!("workspace input {path}");
    let head = std::fs::read_to_string(workspace.join(path)).map_err(|_| workspace_input())?;
    if !changed {
        return Ok((head.clone(), head));
    }
    let (git, base) = base;
    let before = show_at_base(git, workspace, runner, base, path)
        .map_err(|detail| format!("cannot read {path}: {detail}"))?
        .ok_or_else(workspace_input)?;
    Ok((before, head))
}

fn module_change(
    paths: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: Option<(&Path, &str)>,
    startup_options: &[String],
) -> Result<ModuleBump, String> {
    let first = paths.first().map_or(ROOT_MODULE, String::as_str);
    let Some(base) = base else {
        return Err(format!("workspace input {first}"));
    };
    let changed = |path: &str| paths.iter().any(|changed| changed == path);
    let (base_module, head_module) =
        module_text(ROOT_MODULE, changed(ROOT_MODULE), workspace, runner, base)?;
    let bumped = module_inputs::bumped_deps(&base_module, &head_module)?;
    let (base_lock, head_lock) =
        module_text(ROOT_LOCK, changed(ROOT_LOCK), workspace, runner, base)?;
    let lock = module_inputs::lock_changes(&base_lock, &head_lock)?;
    module_inputs::lock_pins(&head_lock, &bumped)?;
    let read_include = |label: &str| {
        dx_module_bazel::label_path(label)
            .and_then(|rel| std::fs::read_to_string(workspace.join(&rel)).map_err(|_| rel))
            .map_err(|_| format!("workspace input {label}"))
    };
    let files = dx_module_bazel::load_with_includes(&head_module, &read_include)
        .map_err(|_| format!("workspace input {ROOT_MODULE}"))?;
    if let Some(reason) = module_inputs::overrides_that_widen(&files) {
        return Err(reason);
    }
    let caches = dx_process::repository_caches(
        &|name| std::env::var(name).ok(),
        workspace,
        &|path| std::fs::read_to_string(path).ok(),
        startup_options,
    );
    let read = |hash: &str| module_inputs::cached(&caches, hash);
    let mut changed_modules: BTreeSet<String> = bumped.into_keys().collect();
    changed_modules.extend(lock.modules);
    let graph = module_inputs::module_graph(&module_inputs::registry_files(&head_lock)?, &read)?;
    let old: Vec<_> = module_inputs::registry_files(&base_lock)?
        .into_iter()
        .filter(|file| changed_modules.contains(&file.name))
        .collect();
    let old_graph = module_inputs::module_graph(&old, &read)?;
    let modules = module_inputs::closure(&changed_modules, &graph.deps);
    if let Some(module) = modules
        .iter()
        .find(|module| graph.registers.contains(*module) || old_graph.registers.contains(*module))
    {
        return Err(format!("changed module {module} registers toolchains"));
    }
    let owners = module_inputs::repo_owners(&files);
    let owned_by_affected = |repo: &str| {
        owners
            .get(repo)
            .is_some_and(|owner| modules.contains(owner))
    };
    for file in &files {
        for call in file.calls().iter().filter(|call| {
            matches!(
                call.name.as_str(),
                "register_toolchains" | "register_execution_platforms"
            )
        }) {
            for value in &call.positional {
                let dx_module_bazel::Value::Str(label) = value else {
                    continue;
                };
                let repo = label
                    .value
                    .strip_prefix('@')
                    .map(|rest| rest.trim_start_matches('@'))
                    .and_then(|rest| rest.split_once("//"))
                    .map(|(repo, _)| repo);
                if repo.is_some_and(|repo| {
                    owned_by_affected(repo)
                        || modules
                            .iter()
                            .any(|module| repo.starts_with(&format!("{module}+")))
                }) {
                    return Err(format!("{ROOT_MODULE} registers {}", label.value));
                }
            }
        }
    }
    if let Some((key, _)) = lock
        .owned
        .iter()
        .find(|(_, module)| !modules.contains(module))
    {
        return Err(format!("{ROOT_LOCK} changed {key}"));
    }
    let mut loads: BTreeSet<String> = owners
        .iter()
        .filter(|(_, owner)| modules.contains(*owner))
        .map(|(repo, _)| repo.clone())
        .collect();
    for module in &modules {
        loads.insert(format!("{module}+"));
        loads.insert(format!("{module}~"));
    }
    let path = if changed(ROOT_MODULE) {
        ROOT_MODULE
    } else {
        ROOT_LOCK
    };
    Ok(ModuleBump {
        path: path.to_owned(),
        modules,
        loads: loads.into_iter().collect(),
    })
}

const SHOWN_REPOS: usize = 5;

fn rdeps_scope(repos: &[String]) -> String {
    if repos.len() > SHOWN_REPOS {
        format!("rdeps(//..., {} repos)", repos.len())
    } else {
        format!("rdeps(//..., {})", repos.join(", "))
    }
}

fn is_build_input(path: &str) -> bool {
    let name = file_name_of(path);
    matches!(name, "BUILD" | "BUILD.bazel")
}

fn is_starlark_module(path: &str) -> bool {
    path.ends_with(".bzl") || path.ends_with(".scl")
}

fn package_all(label: &str) -> String {
    let package = label.split_once(':').map_or(label, |(package, _)| package);
    format!("{package}:all")
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Fallback {
    Keep,
    Nearest,
    Package { scope: String, reason: String },
}

fn unowned(workspace: &Path, path: &str, fallback: &Fallback) -> Option<(String, String)> {
    match fallback {
        Fallback::Keep => None,
        Fallback::Nearest => Some((
            dx_adopt::nearest_package_pattern(workspace, path),
            format!("unowned {path}"),
        )),
        Fallback::Package { scope, reason } => Some((scope.clone(), reason.clone())),
    }
}

fn adds_package(change: &dx_adopt::GitChange, path: &str) -> bool {
    change.path == path
        && matches!(
            change.kind,
            dx_adopt::ChangeKind::Added
                | dx_adopt::ChangeKind::Copied
                | dx_adopt::ChangeKind::Renamed
        )
}

fn valid_change_path(path: &str) -> Result<(), ResolveError> {
    if path.is_empty() {
        return Err(ResolveError::EmptyScope);
    }
    if dx_path::is_absolute(path) || path.split('/').any(|part| part == "..") {
        return Err(ResolveError::OutsideWorkspace {
            scope: path.to_owned(),
        });
    }
    if path.chars().any(char::is_control) {
        return Err(ResolveError::UnsupportedName {
            scope: path.to_owned(),
        });
    }
    Ok(())
}

fn push_widened(widened: &mut Vec<WidenedScope>, scope: String, reason: String) {
    if !widened
        .iter()
        .any(|existing| existing.scope == scope && existing.reason == reason)
    {
        widened.push(WidenedScope {
            scope,
            reason,
            repos: Vec::new(),
        });
    }
}

fn widen_bzl_changes(
    paths: &[String],
    workspace: &Path,
    aspect_labels: &[String],
    widened: &mut Vec<WidenedScope>,
) {
    let impact = dx_adopt::bzl_impact(workspace, paths, aspect_labels);
    widen_impact(impact, "", widened);
}

fn widen_impact(
    impact: Result<BTreeMap<String, dx_adopt::BzlImpact>, String>,
    prefix: &str,
    widened: &mut Vec<WidenedScope>,
) {
    let impact = match impact {
        Ok(impact) => impact,
        Err(detail) => {
            push_widened(
                widened,
                "//...".to_owned(),
                format!("cannot read load graph: {detail}"),
            );
            return;
        }
    };
    let mut loaders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, effect) in &impact {
        match effect {
            dx_adopt::BzlImpact::Global(reason) => {
                push_widened(widened, "//...".to_owned(), reason.clone());
            }
            dx_adopt::BzlImpact::Packages(packages) => {
                for package in packages {
                    loaders
                        .entry(package.clone())
                        .or_default()
                        .push(format!("{prefix}{path}"));
                }
            }
        }
    }
    for (package, files) in loaders {
        push_widened(widened, package, format!("loads {}", files.join(", ")));
    }
}

pub fn resolve_affected(
    changes: &[dx_adopt::GitChange],
    workspace: &Path,
    runner: &dyn QueryRunner,
    startup_options: &[String],
    aspect_labels: &[String],
    base: Option<(&Path, &str)>,
) -> Result<AffectedSelection, ResolveError> {
    let mut targets: Vec<String> = Vec::new();
    let mut widened: Vec<WidenedScope> = Vec::new();
    let mut candidates: Vec<(String, String, Fallback)> = Vec::new();
    let mut bzl_paths: Vec<String> = Vec::new();
    let mut lock_paths: Vec<String> = Vec::new();
    let mut module_paths: Vec<String> = Vec::new();
    let mut cache = PackageCache::default();
    let (hubs, inputs) = module_inputs_of(workspace);
    let rc_files = workspace_rc_files(workspace);
    for change in changes {
        let mut paths = vec![change.path.clone()];
        if let Some(from) = &change.from {
            paths.push(from.clone());
        }
        for path in paths {
            valid_change_path(&path)?;
            if is_lock_input(&path, &hubs) {
                lock_paths.push(path);
                continue;
            }
            if path == ROOT_MODULE || path == ROOT_LOCK {
                module_paths.push(path);
                continue;
            }
            if is_workspace_input(&path, &rc_files) || inputs.contains(&path) {
                push_widened(
                    &mut widened,
                    "//...".to_owned(),
                    format!("workspace input {path}"),
                );
                continue;
            }
            let bzl = is_starlark_module(&path);
            if bzl {
                bzl_paths.push(path.clone());
                if !workspace.join(&path).is_file() {
                    continue;
                }
            }
            if !workspace.join(&path).is_file() && !workspace.join(&path).is_dir() {
                let reason = if change.kind == dx_adopt::ChangeKind::Deleted {
                    format!("deleted {path}")
                } else if change.from.as_deref() == Some(path.as_str()) {
                    format!("renamed from {path}")
                } else {
                    format!("removed {path}")
                };
                if is_build_input(&path) {
                    push_widened(&mut widened, "//...".to_owned(), reason);
                    continue;
                }
                match cache.file_label(workspace, &path, &path) {
                    Ok(label) => {
                        let scope = package_all(&label);
                        candidates.push((path, label, Fallback::Package { scope, reason }));
                    }
                    Err(ResolveError::NotAPackage { .. }) => {
                        push_widened(&mut widened, "//...".to_owned(), reason);
                    }
                    Err(error) => return Err(error),
                }
                continue;
            }
            match cache.file_label(workspace, &path, &path) {
                Ok(label) if is_build_input(&path) => {
                    if adds_package(change, &path) {
                        push_widened(
                            &mut widened,
                            "//...".to_owned(),
                            format!("new package {path}"),
                        );
                    } else {
                        push_widened(
                            &mut widened,
                            package_all(&label),
                            format!("build file {path}"),
                        );
                    }
                    candidates.push((path, label, Fallback::Keep));
                }
                Ok(label) => {
                    let fallback = if bzl {
                        Fallback::Keep
                    } else {
                        Fallback::Nearest
                    };
                    candidates.push((path, label, fallback));
                }
                Err(ResolveError::NotAPackage { .. }) if bzl => {}
                Err(ResolveError::NotAPackage { .. }) => push_widened(
                    &mut widened,
                    dx_adopt::nearest_package_pattern(workspace, &path),
                    format!("no enclosing package for {path}"),
                ),
                Err(error) => return Err(error),
            }
        }
    }
    bzl_paths.sort();
    bzl_paths.dedup();
    if !bzl_paths.is_empty() {
        widen_bzl_changes(&bzl_paths, workspace, aspect_labels, &mut widened);
    }
    lock_paths.sort();
    lock_paths.dedup();
    let mut narrowed: Vec<(String, Vec<(String, LockChange)>)> = Vec::new();
    for path in lock_paths {
        match lock_change(&path, &hubs, workspace, runner, base) {
            Err(reason) => push_widened(&mut widened, "//...".to_owned(), reason),
            Ok(changed) => {
                match cache.file_label(workspace, &path, &path) {
                    Ok(label) => candidates.push((path.clone(), label, Fallback::Keep)),
                    Err(ResolveError::NotAPackage { .. }) => {}
                    Err(error) => return Err(error),
                }
                narrowed.push((path, changed));
            }
        }
    }
    module_paths.sort();
    module_paths.dedup();
    let mut bump = None;
    if !module_paths.is_empty() {
        match module_change(&module_paths, workspace, runner, base, startup_options) {
            Err(reason) => push_widened(&mut widened, "//...".to_owned(), reason),
            Ok(found) => {
                let impact = dx_adopt::repo_impact(workspace, &found.loads, aspect_labels);
                widen_impact(impact, "@", &mut widened);
                bump = Some(found).filter(|found| !found.modules.is_empty());
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    let mut graph = None;
    if !candidates.is_empty() || !narrowed.is_empty() || bump.is_some() {
        let loaded = RuleGraph::load(workspace, runner, startup_options)?;
        if let Some(bump) = &bump {
            let seeds = module_inputs::module_seeds(loaded.external_inputs(), &bump.modules);
            if let Some(toolchain) = loaded
                .reached_of_kind(&seeds, |kind| kind == "toolchain")
                .first()
            {
                push_widened(
                    &mut widened,
                    "//...".to_owned(),
                    format!(
                        "changed modules in {} reach toolchain {toolchain}",
                        bump.path
                    ),
                );
            }
            targets.extend(seeds);
            let repos: Vec<String> = bump
                .modules
                .iter()
                .map(|module| format!("@{module}"))
                .collect();
            widened.push(WidenedScope {
                scope: rdeps_scope(&repos),
                reason: format!("changed modules in {}", bump.path),
                repos,
            });
        }
        for (path, changed) in &narrowed {
            let mut repos: BTreeSet<String> = BTreeSet::new();
            let mut seeds: Vec<String> = Vec::new();
            let go_changed = changed
                .iter()
                .any(|(_, change)| !change.go_repos.is_empty());
            for (hub, change) in changed {
                let LockChange {
                    crates,
                    aliases,
                    pip,
                    maven,
                    go_repos,
                    npm,
                } = change;
                if *maven {
                    seeds.extend(lock_inputs::maven_seeds(
                        loaded.external_inputs(),
                        hub,
                        crates,
                    ));
                    repos.extend(crates.keys().map(|name| format!("@{hub}//:{name}")));
                    continue;
                }
                if *pip {
                    seeds.extend(lock_inputs::pip_seeds(
                        loaded.external_inputs(),
                        hub,
                        crates,
                    ));
                    repos.extend(crates.keys().map(|name| format!("@{hub}//{name}")));
                    continue;
                }
                if *npm {
                    seeds.extend(lock_inputs::npm_seeds(
                        loaded.external_inputs(),
                        hub,
                        crates,
                    ));
                    repos.extend(crates.keys().map(|name| format!("@{hub}//{name}")));
                    continue;
                }
                seeds.extend(lock_inputs::go_seeds(loaded.external_inputs(), go_repos));
                repos.extend(go_repos.iter().map(|name| format!("@{name}//...")));
                seeds.extend(lock_inputs::seeds(
                    loaded.external_inputs(),
                    hub,
                    crates,
                    aliases,
                ));
                repos.extend(crates.keys().map(|name| format!("@{hub}//:{name}")));
                repos.extend(aliases.iter().map(|name| format!("@{hub}//:{name}")));
            }
            // Bazel picks a toolchain through resolution, so its users have no
            // edge to it; a changed crate behind one can reach any rule.
            if let Some(toolchain) = loaded
                .reached_of_kind(&seeds, |kind| kind == "toolchain")
                .first()
            {
                push_widened(
                    &mut widened,
                    "//...".to_owned(),
                    format!("changed crates in {path} reach toolchain {toolchain}"),
                );
            }
            targets.extend(seeds);
            if !repos.is_empty() {
                let repos: Vec<String> = repos.into_iter().collect();
                widened.push(WidenedScope {
                    scope: rdeps_scope(&repos),
                    reason: if go_changed {
                        format!("changed Go modules in {path}")
                    } else {
                        format!("changed crates in {path}")
                    },
                    repos,
                });
            }
        }
        graph = Some(loaded);
    }
    if let Some(loaded) = graph.as_ref().filter(|_| !candidates.is_empty()) {
        let labels: Vec<String> = candidates
            .iter()
            .map(|(_, label, _)| label.clone())
            .collect();
        let owners = loaded.owners_of(&labels);
        if owners.is_empty() {
            for (path, _, fallback) in &candidates {
                if let Some((scope, reason)) = unowned(workspace, path, fallback) {
                    push_widened(&mut widened, scope, reason);
                }
            }
        } else if candidates.len() == 1 {
            targets.extend(owners);
        } else {
            let sources = loaded.inputs_of(&owners);
            for (path, label, fallback) in &candidates {
                if sources.contains(label.as_str()) {
                    continue;
                }
                if let Some((scope, reason)) = unowned(workspace, path, fallback) {
                    push_widened(&mut widened, scope, reason);
                }
            }
            targets.extend(owners);
        }
    }
    targets.sort();
    targets.dedup();
    widened.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then_with(|| left.reason.cmp(&right.reason))
    });
    widened.dedup_by(|next, current| next.scope == current.scope && next.reason == current.reason);
    for widened_scope in widened.iter().filter(|entry| entry.repos.is_empty()) {
        if !targets.contains(&widened_scope.scope) {
            targets.push(widened_scope.scope.clone());
        }
    }
    targets.sort();
    targets.dedup();
    Ok(AffectedSelection {
        targets,
        widened,
        graph,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{resolve, FakeQuery, NeverQuery, QueryFailure, QueryResult};
    use dx_test_scratch::write;
    use dx_testing::cli::strings;

    fn graph(rules: &[(&str, &[&str])]) -> QueryResult {
        let lines: Vec<String> = rules
            .iter()
            .map(|(name, inputs)| {
                serde_json::json!({"type": "RULE", "rule": {"name": name, "ruleInput": inputs}})
                    .to_string()
            })
            .collect();
        FakeQuery::ok(&lines.join("\n"))
    }

    fn graph_with_kinds(rules: &[(&str, &str, &[&str])]) -> QueryResult {
        let lines: Vec<String> = rules
            .iter()
            .map(|(name, class, inputs)| {
                serde_json::json!({"type": "RULE", "rule": {"name": name, "ruleClass": class, "ruleInput": inputs}})
                    .to_string()
            })
            .collect();
        FakeQuery::ok(&lines.join("\n"))
    }

    fn change(path: &str, kind: dx_adopt::ChangeKind) -> dx_adopt::GitChange {
        dx_adopt::GitChange {
            path: path.to_owned(),
            from: None,
            kind,
        }
    }

    fn renamed(from: &str, path: &str) -> dx_adopt::GitChange {
        dx_adopt::GitChange {
            path: path.to_owned(),
            from: Some(from.to_owned()),
            kind: dx_adopt::ChangeKind::Renamed,
        }
    }

    #[test]
    fn deleted_unowned_file_widens_to_its_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[("//pkg:lib", &["//pkg:a.py"])])]);
        let selection = resolve_affected(
            &[change("pkg/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("deleted widens");
        assert_eq!(selection.targets, strings(&["//pkg:all"]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].scope, "//pkg:all");
        assert_eq!(selection.widened[0].reason, "deleted pkg/gone.py");
        assert!(!selection.widened.is_empty());
        assert!(selection.graph.is_some());
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn deleted_file_still_named_by_a_rule_selects_it() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-named-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        let query = FakeQuery::new(vec![graph(&[("//pkg:lib", &["//pkg:gone.py"])])]);
        let selection = resolve_affected(
            &[change("pkg/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("deleted resolves");
        assert_eq!(selection.targets, strings(&["//pkg:lib"]));
        assert!(selection.widened.is_empty());
    }

    #[test]
    fn deleted_build_file_widens_to_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-build-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "");
        std::fs::create_dir_all(workspace.join("pkg")).expect("mkdir");
        let selection = resolve_affected(
            &[change("pkg/BUILD.bazel", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &NeverQuery,
            &[],
            &[],
            None,
        )
        .expect("deleted build widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "deleted pkg/BUILD.bazel");
    }

    #[test]
    fn deleted_file_without_package_widens_to_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-nopkg-");
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join("docs")).expect("mkdir");
        let selection = resolve_affected(
            &[change("docs/gone.md", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &NeverQuery,
            &[],
            &[],
            None,
        )
        .expect("deleted widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "deleted docs/gone.md");
    }

    #[test]
    fn deleted_root_file_widens_to_root_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-root-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("deleted widens");
        assert_eq!(selection.targets, strings(&["//:all"]));
    }

    #[test]
    fn rename_keeps_target_and_widens_missing_source_to_its_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-rename-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[("//pkg:lib", &["//pkg:a.py"])])]);
        let selection = resolve_affected(
            &[renamed("pkg/was.py", "pkg/a.py")],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("rename resolves");
        assert_eq!(
            selection.targets,
            strings(&["//pkg:all", "//pkg:lib"]),
            "existing side keeps its owner while the missing source widens"
        );
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].reason, "renamed from pkg/was.py");
        assert_eq!(query.calls().len(), 1, "one batched ownership query");
    }

    #[test]
    fn build_inputs_add_their_package_and_owners() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-owned-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/space name.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[(
            "//pkg:lib",
            &["//pkg:BUILD.bazel", "//pkg:space name.py"],
        )])]);
        let selection = resolve_affected(
            &[
                change("pkg/space name.py", dx_adopt::ChangeKind::Added),
                change("pkg/BUILD.bazel", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("attributed");
        assert_eq!(selection.targets, strings(&["//pkg:all", "//pkg:lib"]));
        assert_eq!(
            selection.widened,
            vec![WidenedScope {
                scope: "//pkg:all".to_owned(),
                reason: "build file pkg/BUILD.bazel".to_owned(),
                repos: Vec::new(),
            }]
        );
        assert!(!selection.widened.is_empty());
        assert_eq!(query.calls().len(), 1, "one graph query");
        assert!(selection.graph.is_some());
    }

    #[test]
    fn unowned_build_inputs_widen_to_their_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-unowned-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[
                change("pkg/a.py", dx_adopt::ChangeKind::Modified),
                change("pkg/BUILD.bazel", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("unowned widens");
        assert_eq!(selection.targets, strings(&["//pkg/...", "//pkg:all"]));
        assert!(!selection.targets.iter().any(|target| target == "//..."));
        assert_eq!(query.calls().len(), 1, "one graph query");
        let mut reasons: Vec<&str> = selection
            .widened
            .iter()
            .map(|widened| widened.reason.as_str())
            .collect();
        reasons.sort();
        assert_eq!(
            reasons,
            vec!["build file pkg/BUILD.bazel", "unowned pkg/a.py"]
        );
    }

    #[test]
    fn build_only_edit_with_sole_corpus_owner_selects_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-corpus-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[
            ("//pkg:corpus_starlark", &["//pkg:BUILD.bazel"]),
            ("//pkg:lib", &["//pkg:a.py"]),
        ])]);
        let selection = resolve_affected(
            &[change("pkg/BUILD.bazel", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("build edit resolves");
        assert_eq!(
            selection.targets,
            strings(&["//pkg:all", "//pkg:corpus_starlark"])
        );
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].scope, "//pkg:all");
        assert_eq!(selection.widened[0].reason, "build file pkg/BUILD.bazel");
        assert_eq!(query.calls().len(), 1, "one graph query");
    }

    #[test]
    fn added_build_file_widens_to_everything() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-added-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/new/BUILD.bazel", "");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("pkg/new/BUILD.bazel", dx_adopt::ChangeKind::Added)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("new package widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].scope, "//...");
        assert_eq!(
            selection.widened[0].reason,
            "new package pkg/new/BUILD.bazel"
        );
    }

    #[test]
    fn root_build_edit_selects_root_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-root-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD", "");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("BUILD", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("root build edit resolves");
        assert_eq!(selection.targets, strings(&["//:all"]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].reason, "build file BUILD");
    }

    #[test]
    fn single_unowned_file_widens_without_attribution_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-single-unowned-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("pkg/a.py", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("single unowned widens");
        assert_eq!(selection.targets, strings(&["//pkg/..."]));
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn workspace_inputs_widen_to_repository_without_query() {
        for changed in [
            "MODULE.bazel",
            "MODULE.bazel.lock",
            "deps/go.MODULE.bazel",
            "Cargo.lock",
            ".bazelrc",
        ] {
            let scratch = dx_test_scratch::scratch("dx-changed-workspace-");
            let workspace = scratch.path().to_path_buf();
            write(&workspace, "pkg/BUILD.bazel", "");
            write(&workspace, changed, "x\n");
            let selection = resolve_affected(
                &[change(changed, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &crate::resolve::NeverQuery,
                &[],
                &[],
                None,
            )
            .expect("workspace input widens");
            assert_eq!(selection.targets, strings(&["//..."]), "{changed}");
            assert_eq!(selection.widened.len(), 1, "{changed}");
            assert_eq!(
                selection.widened[0].reason,
                format!("workspace input {changed}"),
                "{changed}"
            );
        }
    }

    #[test]
    fn imported_rc_files_widen_to_repository_without_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-rc-imports-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(
            &workspace,
            ".bazelrc",
            "import %workspace%/tools/bazelrc/correctness.bazelrc\ntry-import %workspace%/user.bazelrc\n",
        );
        write(&workspace, "tools/bazelrc/BUILD.bazel", "");
        write(
            &workspace,
            "tools/bazelrc/correctness.bazelrc",
            "import \"%workspace%/tools/bazelrc/nested.bazelrc\"\nimport %workspace%/.bazelrc\n",
        );
        write(&workspace, "tools/bazelrc/nested.bazelrc", "build --x\n");
        for changed in [
            "tools/bazelrc/correctness.bazelrc",
            "tools/bazelrc/nested.bazelrc",
        ] {
            let selection = resolve_affected(
                &[change(changed, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &crate::resolve::NeverQuery,
                &[],
                &[],
                None,
            )
            .expect("imported rc widens");
            assert_eq!(selection.targets, strings(&["//..."]), "{changed}");
            assert_eq!(selection.widened.len(), 1, "{changed}");
            assert_eq!(
                selection.widened[0].reason,
                format!("workspace input {changed}"),
                "{changed}"
            );
        }
    }

    #[test]
    fn unimported_rc_files_do_not_widen() {
        let scratch = dx_test_scratch::scratch("dx-changed-rc-unimported-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, ".bazelrc", "build --x\n");
        write(&workspace, "consumers/x/BUILD.bazel", "");
        write(&workspace, "consumers/x/consumer.bazelrc", "build --y\n");
        let query = FakeQuery::new(vec![graph(&[(
            "//consumers/x:t",
            &["//consumers/x:consumer.bazelrc"],
        )])]);
        let selection = resolve_affected(
            &[change(
                "consumers/x/consumer.bazelrc",
                dx_adopt::ChangeKind::Modified,
            )],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("owned");
        assert_eq!(selection.targets, strings(&["//consumers/x:t"]));
        assert!(selection.widened.is_empty());
    }

    #[test]
    fn rc_import_paths_skips_unsafe_lines() {
        let content = "# import %workspace%/a.rc\n\
            import \"%workspace%/q.rc\"\n\
            try-import \\\n  %workspace%/c.rc\n\
            import /abs.rc\n\
            import %workspace%/../up.rc\n\
            import rel.rc\n\
            common %workspace%/z.rc\n";
        assert_eq!(rc_import_paths(content), strings(&["q.rc", "c.rc"]));
    }

    fn widens_as_workspace_input(workspace: &Path, changed: &str) {
        let selection = resolve_affected(
            &[change(changed, dx_adopt::ChangeKind::Modified)],
            workspace,
            &NeverQuery,
            &[],
            &[],
            base(),
        )
        .expect("module input widens");
        assert_eq!(selection.targets, strings(&["//..."]), "{changed}");
        assert_eq!(selection.widened.len(), 1, "{changed}");
        assert_eq!(
            selection.widened[0].reason,
            format!("workspace input {changed}"),
            "{changed}"
        );
    }

    #[test]
    fn lock_files_named_in_module_bazel_widen_without_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-module-inputs-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            concat!(
                "module(name = \"demo\")\n",
                "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n",
                "npm.npm_translate_lock(name = \"npm\", pnpm_lock = \"//:pnpm-lock.yaml\")\n",
                "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\n",
                "go_deps.from_file(go_mod = \"//third_party/go:go.mod\")\n",
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n",
                "maven.install(lock_file = \"//third_party/jvm:maven_install.json\")\n",
                "pip = use_extension(\"@rules_python//python/extensions:pip.bzl\", \"pip\")\n",
                "pip.parse(hub_name = \"pypi\", requirements_lock = \"//:requirements_lock.txt\")\n",
                "pip.parse(hub_name = \"pypi\", requirements_by_platform = {\"//py:requirements_linux.txt\": \"linux_*\"})\n",
                "include(\"//deps:go.MODULE.bazel\")\n",
            ),
        );
        write(
            workspace,
            "deps/go.MODULE.bazel",
            "bazel_dep(name = \"rules_go\", version = \"0.50.0\")\n",
        );
        for changed in ["py/requirements_linux.txt", "deps/go.MODULE.bazel"] {
            write(workspace, changed, "x\n");
            widens_as_workspace_input(workspace, changed);
        }
    }

    #[test]
    fn crate_annotation_patches_and_build_files_widen() {
        let scratch = crate_workspace("dx-changed-crate-annotation-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.annotation(crate = \"serde\", patches = [\"//third_party:serde.patch\"], additive_build_file = \"//third_party:serde.BUILD\")\n",
        );
        write(workspace, "third_party/serde.patch", "x\n");
        write(workspace, "third_party/serde.BUILD", "x\n");
        for changed in ["third_party/serde.patch", "third_party/serde.BUILD"] {
            widens_as_workspace_input(workspace, changed);
        }
    }

    #[test]
    fn unnamed_lock_files_and_crate_manifests_stay_ordinary_files() {
        let scratch = crate_workspace("dx-changed-unnamed-lock-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(cargo_lockfile = \"//:Cargo.lock\", manifests = [\"//a:Cargo.toml\"])\n",
        );
        write(workspace, "a/Cargo.toml", "[package]\n");
        write(workspace, "b/package-lock.json", "{}\n");
        for (changed, owner) in [
            ("a/Cargo.toml", "//a:lib"),
            ("b/package-lock.json", "//b:lib"),
        ] {
            let query = FakeQuery::new(vec![graph(&[
                ("//a:lib", &["//a:Cargo.toml"]),
                ("//b:lib", &["//b:package-lock.json"]),
            ])]);
            let selection = resolve_affected(
                &[change(changed, dx_adopt::ChangeKind::Modified)],
                workspace,
                &query,
                &[],
                &[],
                base(),
            )
            .expect("ordinary file");
            assert_eq!(selection.targets, strings(&[owner]), "{changed}");
            assert!(
                selection.widened.is_empty(),
                "{changed}: {:?}",
                selection.widened
            );
        }
    }

    #[test]
    fn well_known_lock_names_widen_when_module_bazel_is_unreadable() {
        let scratch = dx_test_scratch::scratch("dx-changed-fallback-inputs-");
        let workspace = scratch.path();
        write(workspace, "web/BUILD.bazel", "");
        write(workspace, "web/pnpm-lock.yaml", "x\n");
        widens_as_workspace_input(workspace, "web/pnpm-lock.yaml");
        write(
            workspace,
            "MODULE.bazel",
            "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(pnpm_lock = LOCK)\n",
        );
        widens_as_workspace_input(workspace, "web/pnpm-lock.yaml");
        write(workspace, "MODULE.bazel", "module(name = \n");
        widens_as_workspace_input(workspace, "web/pnpm-lock.yaml");
    }

    const CRATE_MODULE: &str = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(\n    cargo_lockfile = \"//:Cargo.lock\",\n    lockfile = \"//:cargo-bazel-lock.json\",\n)\n";

    fn cargo_lock(serde: &str) -> String {
        format!(
            "version = 3\n\n[[package]]\nname = \"serde\"\nversion = \"{serde}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n[[package]]\nname = \"itoa\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
        )
    }

    #[test]
    fn included_segment_crate_hubs_count_as_lock_inputs() {
        let scratch = dx_test_scratch::scratch("dx-changed-segment-hubs-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "include(\"//deps:rust.MODULE.bazel\")\n",
        );
        write(workspace, "deps/BUILD.bazel", "");
        write(workspace, "deps/rust.MODULE.bazel", CRATE_MODULE);
        let (hubs, _) = module_inputs_of(workspace);
        assert_ne!(
            lock_inputs::classify("cargo-bazel-lock.json", &hubs),
            LockKind::Unknown
        );
    }

    #[test]
    fn root_extension_binding_does_not_reach_a_segment() {
        let scratch = dx_test_scratch::scratch("dx-changed-segment-binding-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ninclude(\"//deps:rust.MODULE.bazel\")\n",
        );
        write(workspace, "deps/BUILD.bazel", "");
        write(
            workspace,
            "deps/rust.MODULE.bazel",
            "crate.from_cargo(cargo_lockfile = \"//:Cargo.lock\", lockfile = \"//:cargo-bazel-lock.json\")\n",
        );
        let (hubs, _) = module_inputs_of(workspace);
        assert_eq!(
            lock_inputs::classify("cargo-bazel-lock.json", &hubs),
            LockKind::Unknown
        );
    }

    fn crate_workspace(prefix: &str) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(prefix);
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", CRATE_MODULE);
        write(workspace, "a/BUILD.bazel", "");
        write(workspace, "b/BUILD.bazel", "");
        write(workspace, "Cargo.lock", &cargo_lock("1.0.2"));
        scratch
    }

    fn crate_graph() -> QueryResult {
        graph(&[
            ("//a:lib", &["@@rules_rust++crate+crates//:serde"]),
            ("//a:t", &["//a:lib"]),
            (
                "//b:lib",
                &["@@rules_rust++crate+crates//:itoa", "//b:b.rs"],
            ),
        ])
    }

    fn base() -> Option<(&'static Path, &'static str)> {
        Some((Path::new("git"), "base"))
    }

    #[test]
    fn one_crate_cargo_lock_bump_selects_only_its_users() {
        let scratch = crate_workspace("dx-changed-cargo-lock-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![FakeQuery::ok(&cargo_lock("1.0.1")), crate_graph()]);
        let selection = resolve_affected(
            &[change("Cargo.lock", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            base(),
        )
        .expect("lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["@@rules_rust++crate+crates//:serde"])
        );
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: "rdeps(//..., @crates//:serde)".to_owned(),
                reason: "changed crates in Cargo.lock".to_owned(),
                repos: strings(&["@crates//:serde"]),
            }]
        );
        let graph = selection.graph.as_ref().expect("graph");
        assert_eq!(graph.affected(&selection.targets), ["//a:lib", "//a:t"]);
        let calls = query.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, strings(&["git", "show", "base:./Cargo.lock"]));
    }

    struct Bump {
        leaf: &'static str,
        leaf_extra: &'static str,
        user_extra: &'static str,
        root_extra: &'static str,
        lock_extra: serde_json::Value,
    }

    const BUMP: Bump = Bump {
        leaf: "leaf",
        leaf_extra: "",
        user_extra: "",
        root_extra: "",
        lock_extra: serde_json::Value::Null,
    };

    struct BumpWorkspace {
        scratch: tempfile::TempDir,
        base_module: String,
        base_lock: String,
    }

    fn cache_put(workspace: &Path, text: &str) -> String {
        let hash = dx_digest::sha256_hex(text.as_bytes());
        let dir = workspace
            .join("cache/content_addressable/sha256")
            .join(&hash);
        std::fs::create_dir_all(&dir).expect("cache dir");
        std::fs::write(dir.join("file"), text).expect("cache file");
        hash
    }

    fn bump_workspace(prefix: &str, bump: Bump) -> BumpWorkspace {
        let scratch = dx_test_scratch::scratch(prefix);
        let workspace = scratch.path();
        let leaf = bump.leaf;
        let module = |version: &str| {
            format!(
                "module(name = \"root\")\nbazel_dep(name = \"user\", version = \"1.0\")\nbazel_dep(name = \"{leaf}\", version = \"{version}\")\nbazel_dep(name = \"other\", version = \"1.0\")\n{}",
                bump.root_extra
            )
        };
        write(workspace, "MODULE.bazel", &module("1.1"));
        write(
            workspace,
            ".bazelrc",
            &format!("common --repository_cache={}/cache\n", workspace.display()),
        );
        write(
            workspace,
            "d/BUILD.bazel",
            &format!("load(\"@{leaf}//:defs.bzl\", \"x\")\n"),
        );
        write(workspace, "e/BUILD.bazel", "");
        let leaf_old = cache_put(
            workspace,
            &format!("module(name = \"{leaf}\", version = \"1.0\")\n"),
        );
        let leaf_new = cache_put(
            workspace,
            &format!(
                "module(name = \"{leaf}\", version = \"1.1\")\n{}",
                bump.leaf_extra
            ),
        );
        let user = cache_put(
            workspace,
            &format!(
                "module(name = \"user\")\nbazel_dep(name = \"{leaf}\", version = \"1.0\")\n{}",
                bump.user_extra
            ),
        );
        let other = cache_put(workspace, "module(name = \"other\")\n");
        let url = |name: &str, version: &str, file: &str| {
            format!("https://bcr.bazel.build/modules/{name}/{version}/{file}")
        };
        let mut hashes = serde_json::Map::new();
        for (key, value) in [
            (url(leaf, "1.0", "MODULE.bazel"), leaf_old),
            (url("user", "1.0", "MODULE.bazel"), user),
            (url("user", "1.0", "source.json"), "u".to_owned()),
            (url("other", "1.0", "MODULE.bazel"), other),
            (url("other", "1.0", "source.json"), "o".to_owned()),
        ] {
            hashes.insert(key, value.into());
        }
        let mut base_hashes = hashes.clone();
        base_hashes.insert(url(leaf, "1.0", "source.json"), "s10".into());
        hashes.insert(url(leaf, "1.1", "MODULE.bazel"), leaf_new.into());
        hashes.insert(url(leaf, "1.1", "source.json"), "s11".into());
        let lock = |hashes: serde_json::Map<String, serde_json::Value>,
                    extra: &serde_json::Value| {
            let mut root = serde_json::json!({"lockFileVersion": 24, "registryFileHashes": hashes});
            if let (Some(root), serde_json::Value::Object(extra)) = (root.as_object_mut(), extra) {
                root.extend(extra.clone());
            }
            root.to_string()
        };
        write(
            workspace,
            "MODULE.bazel.lock",
            &lock(hashes, &bump.lock_extra),
        );
        BumpWorkspace {
            scratch,
            base_module: module("1.0"),
            base_lock: lock(base_hashes, &serde_json::Value::Null),
        }
    }

    fn bump_graph() -> QueryResult {
        graph_with_kinds(&[
            ("//d:lib", "cc_library", &["@@leaf+//:lib"]),
            ("//u:lib", "cc_library", &["@@user+//:lib"]),
            ("//e:lib", "cc_library", &["@@other+//:lib"]),
        ])
    }

    fn resolve_bump(fixture: &BumpWorkspace, query: &FakeQuery) -> AffectedSelection {
        resolve_affected(
            &[
                change("MODULE.bazel", dx_adopt::ChangeKind::Modified),
                change("MODULE.bazel.lock", dx_adopt::ChangeKind::Modified),
            ],
            fixture.scratch.path(),
            query,
            &[],
            &[],
            base(),
        )
        .expect("bump resolves")
    }

    fn widened_reason(fixture: &BumpWorkspace, query: &FakeQuery) -> String {
        let selection = resolve_bump(fixture, query);
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1, "{:?}", selection.widened);
        selection.widened[0].reason.clone()
    }

    fn base_reads(fixture: &BumpWorkspace) -> FakeQuery {
        FakeQuery::new(vec![
            FakeQuery::ok(&fixture.base_module),
            FakeQuery::ok(&fixture.base_lock),
        ])
    }

    #[test]
    fn version_only_bazel_dep_bump_selects_its_users() {
        let fixture = bump_workspace("dx-changed-bump-", BUMP);
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&fixture.base_module),
            FakeQuery::ok(&fixture.base_lock),
            bump_graph(),
        ]);
        let selection = resolve_bump(&fixture, &query);
        assert_eq!(
            selection.targets,
            strings(&["//d:all", "@@leaf+//:lib", "@@user+//:lib"])
        );
        assert_eq!(
            selection.widened,
            [
                WidenedScope {
                    scope: "//d:all".to_owned(),
                    reason: "loads @leaf".to_owned(),
                    repos: Vec::new(),
                },
                WidenedScope {
                    scope: "rdeps(//..., @leaf, @user)".to_owned(),
                    reason: "changed modules in MODULE.bazel".to_owned(),
                    repos: strings(&["@leaf", "@user"]),
                },
            ]
        );
        let graph = selection.graph.as_ref().expect("graph");
        let seeds = strings(&["@@leaf+//:lib", "@@user+//:lib"]);
        assert_eq!(graph.affected(&seeds), ["//d:lib", "//u:lib"]);
        let calls = query.calls();
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].0, strings(&["git", "show", "base:./MODULE.bazel"]));
        assert_eq!(
            calls[1].0,
            strings(&["git", "show", "base:./MODULE.bazel.lock"])
        );
    }

    #[test]
    fn bump_of_toolchain_registering_module_widens() {
        for (bump, module) in [
            (
                Bump {
                    leaf_extra: "register_toolchains(\"//:t\")\n",
                    ..BUMP
                },
                "leaf",
            ),
            (
                Bump {
                    user_extra: "register_execution_platforms(\"//:p\")\n",
                    ..BUMP
                },
                "user",
            ),
        ] {
            let fixture = bump_workspace("dx-changed-bump-tc-", bump);
            let query = base_reads(&fixture);
            assert_eq!(
                widened_reason(&fixture, &query),
                format!("changed module {module} registers toolchains")
            );
            assert_eq!(query.calls().len(), 2);
        }
    }

    #[test]
    fn bump_of_bazel_tools_dep_widens() {
        let fixture = bump_workspace(
            "dx-changed-bump-cc-",
            Bump {
                leaf: "rules_cc",
                ..BUMP
            },
        );
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "changed module bazel_tools registers toolchains"
        );
    }

    #[test]
    fn non_version_module_edit_widens() {
        let fixture = bump_workspace("dx-changed-bump-edit-", BUMP);
        let edited = fixture.base_module.replace("\"root\"", "\"renamed\"");
        let query = FakeQuery::new(vec![FakeQuery::ok(&edited)]);
        assert_eq!(
            widened_reason(&fixture, &query),
            "workspace input MODULE.bazel"
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn module_bump_without_lock_update_widens() {
        let fixture = bump_workspace("dx-changed-bump-stale-", BUMP);
        write(
            fixture.scratch.path(),
            "MODULE.bazel.lock",
            &fixture.base_lock,
        );
        let query = FakeQuery::new(vec![FakeQuery::ok(&fixture.base_module)]);
        let selection = resolve_affected(
            &[change("MODULE.bazel", dx_adopt::ChangeKind::Modified)],
            fixture.scratch.path(),
            &query,
            &[],
            &[],
            base(),
        )
        .expect("bump resolves");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1, "{:?}", selection.widened);
        assert_eq!(
            selection.widened[0].reason,
            "workspace input MODULE.bazel.lock"
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn missing_cached_module_file_widens() {
        let fixture = bump_workspace("dx-changed-bump-miss-", BUMP);
        std::fs::remove_dir_all(fixture.scratch.path().join("cache")).expect("drop cache");
        let query = base_reads(&fixture);
        assert!(widened_reason(&fixture, &query).starts_with("no cached MODULE.bazel for "));
    }

    #[test]
    fn cache_file_with_wrong_hash_is_ignored() {
        let fixture = bump_workspace("dx-changed-bump-hash-", BUMP);
        let user = "module(name = \"user\")\nbazel_dep(name = \"leaf\", version = \"1.0\")\n";
        let hash = dx_digest::sha256_hex(user.as_bytes());
        let file = fixture
            .scratch
            .path()
            .join("cache/content_addressable/sha256")
            .join(hash)
            .join("file");
        std::fs::write(file, "module(name = \"user\")\n").expect("tamper");
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "no cached MODULE.bazel for user@1.0"
        );
    }

    #[test]
    fn unowned_lock_extension_change_widens() {
        let extra = serde_json::json!({"moduleExtensions": {"@@other+//:e.bzl%e": {"x": 1}}});
        let fixture = bump_workspace(
            "dx-changed-bump-ext-",
            Bump {
                lock_extra: extra,
                ..BUMP
            },
        );
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "MODULE.bazel.lock changed @@other+//:e.bzl%e"
        );
    }

    #[test]
    fn local_path_override_widens() {
        let fixture = bump_workspace(
            "dx-changed-bump-local-",
            Bump {
                root_extra: "local_path_override(module_name = \"other\", path = \"o\")\n",
                ..BUMP
            },
        );
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "MODULE.bazel has local_path_override for other"
        );
    }

    #[test]
    fn nested_module_bazel_still_widens() {
        let scratch = dx_test_scratch::scratch("dx-changed-nested-module-");
        let workspace = scratch.path();
        write(workspace, "sub/MODULE.bazel", "module(name = \"sub\")\n");
        let selection = resolve_affected(
            &[change("sub/MODULE.bazel", dx_adopt::ChangeKind::Modified)],
            workspace,
            &NeverQuery,
            &[],
            &[],
            base(),
        )
        .expect("nested module widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "workspace input sub/MODULE.bazel"
        );
    }

    #[test]
    fn lock_bump_behind_a_toolchain_widens_to_everything() {
        let scratch = crate_workspace("dx-changed-toolchain-lock-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&cargo_lock("1.0.1")),
            graph_with_kinds(&[
                (
                    "//tc:prost_impl",
                    "rust_library",
                    &["@@rules_rust++crate+crates//:serde"],
                ),
                ("//tc:prost", "toolchain", &["//tc:prost_impl"]),
                ("//a:lib", "rust_library", &[]),
            ]),
        ]);
        let selection = resolve_affected(
            &[change("Cargo.lock", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            base(),
        )
        .expect("lock widens");
        assert_eq!(
            selection.widened,
            [
                WidenedScope {
                    scope: "//...".to_owned(),
                    reason: "changed crates in Cargo.lock reach toolchain //tc:prost".to_owned(),
                    repos: Vec::new(),
                },
                WidenedScope {
                    scope: "rdeps(//..., @crates//:serde)".to_owned(),
                    reason: "changed crates in Cargo.lock".to_owned(),
                    repos: strings(&["@crates//:serde"]),
                },
            ]
        );
    }

    #[test]
    fn pnpm_lock_bump_selects_only_users_of_changed_packages() {
        let scratch = dx_test_scratch::scratch("dx-changed-pnpm-lock-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            concat!(
                "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n",
                "npm.npm_translate_lock(name = \"npm\", pnpm_lock = \"//:pnpm-lock.yaml\")\n",
            ),
        );
        write(workspace, "BUILD.bazel", "");
        let lock = |lodash: &str| {
            format!(
                "lockfileVersion: '9.0'\n\npackages:\n\n  left-pad@1.0.0:\n    resolution: {{integrity: sha512-a}}\n\n  lodash@{lodash}:\n    resolution: {{integrity: sha512-b}}\n"
            )
        };
        write(workspace, "pnpm-lock.yaml", &lock("4.1.0"));
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&lock("4.0.0")),
            graph(&[
                ("//a:lib", &["@@aspect_rules_js++npm+npm//lodash:pkg"]),
                ("//b:lib", &["@@aspect_rules_js++npm+npm//left-pad:pkg"]),
            ]),
        ]);
        let selection = resolve_affected(
            &[change("pnpm-lock.yaml", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            base(),
        )
        .expect("pnpm lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["@@aspect_rules_js++npm+npm//lodash:pkg"])
        );
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: "rdeps(//..., @npm//lodash)".to_owned(),
                reason: "changed crates in pnpm-lock.yaml".to_owned(),
                repos: strings(&["@npm//lodash"]),
            }]
        );
    }

    #[test]
    fn lock_bump_nothing_uses_selects_nothing_but_names_its_crates() {
        let scratch = crate_workspace("dx-changed-unused-lock-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&cargo_lock("1.0.1")),
            graph(&[("//b:lib", &["@@rules_rust++crate+crates//:itoa"])]),
        ]);
        let selection = resolve_affected(
            &[change("Cargo.lock", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            base(),
        )
        .expect("lock narrows");
        assert!(selection.targets.is_empty(), "{:?}", selection.targets);
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: "rdeps(//..., @crates//:serde)".to_owned(),
                reason: "changed crates in Cargo.lock".to_owned(),
                repos: strings(&["@crates//:serde"]),
            }]
        );
    }

    #[test]
    fn cargo_bazel_lock_json_bump_selects_only_its_users() {
        let scratch = crate_workspace("dx-changed-bazel-lock-");
        let workspace = scratch.path();
        let lock = |serde: &str| {
            serde_json::json!({
                "checksum": serde,
                "crates": {
                    format!("serde {serde}"): {},
                    "itoa 1.0.0": {},
                    "app 0.1.0": {"common_attrs": {"deps": [{"id": format!("serde {serde}"), "target": "serde"}]}},
                },
                "conditions": {},
                "direct_deps": [format!("serde {serde}")],
                "direct_dev_deps": [],
                "workspace_members": {"app 0.1.0": "app"},
            })
            .to_string()
        };
        write(workspace, "cargo-bazel-lock.json", &lock("1.0.2"));
        let query = FakeQuery::new(vec![FakeQuery::ok(&lock("1.0.1")), crate_graph()]);
        let selection = resolve_affected(
            &[change(
                "cargo-bazel-lock.json",
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            base(),
        )
        .expect("lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["@@rules_rust++crate+crates//:serde"])
        );
        assert_eq!(selection.widened[0].repos, strings(&["@crates//:serde"]));
    }

    #[test]
    fn unknown_lock_format_still_widens() {
        let scratch = crate_workspace("dx-changed-unknown-lock-");
        let workspace = scratch.path();
        write(workspace, "x.lock", "x\n");
        let selection = resolve_affected(
            &[change("x.lock", dx_adopt::ChangeKind::Modified)],
            workspace,
            &NeverQuery,
            &[],
            &[],
            base(),
        )
        .expect("unknown lock widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "workspace input x.lock");
        assert!(selection.widened[0].repos.is_empty());
    }

    #[test]
    fn added_deleted_or_unparsable_cargo_lock_widens() {
        let scratch = crate_workspace("dx-changed-added-lock-");
        let workspace = scratch.path();
        let modified = [change("Cargo.lock", dx_adopt::ChangeKind::Modified)];
        let added = FakeQuery::new(vec![FakeQuery::failed(
            "fatal: path 'Cargo.lock' exists on disk, but not in 'base'\n",
        )]);
        let selection =
            resolve_affected(&modified, workspace, &added, &[], &[], base()).expect("added widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "workspace input Cargo.lock");

        let unparsable = FakeQuery::new(vec![FakeQuery::ok("not toml [[[")]);
        let selection = resolve_affected(&modified, workspace, &unparsable, &[], &[], base())
            .expect("unparsable widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert!(
            selection.widened[0]
                .reason
                .starts_with("cannot read Cargo.lock: invalid Cargo.lock"),
            "{:?}",
            selection.widened
        );

        let broken = FakeQuery::new(vec![FakeQuery::failed("fatal: bad object base\n")]);
        let selection =
            resolve_affected(&modified, workspace, &broken, &[], &[], base()).expect("git widens");
        assert_eq!(
            selection.widened[0].reason,
            "cannot read Cargo.lock: git show failed: fatal: bad object base"
        );

        let selection = resolve_affected(&modified, workspace, &NeverQuery, &[], &[], None)
            .expect("no base widens");
        assert_eq!(selection.widened[0].reason, "workspace input Cargo.lock");

        std::fs::remove_file(workspace.join("Cargo.lock")).expect("remove");
        let selection = resolve_affected(
            &[change("Cargo.lock", dx_adopt::ChangeKind::Deleted)],
            workspace,
            &NeverQuery,
            &[],
            &[],
            base(),
        )
        .expect("deleted widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "workspace input Cargo.lock");
    }

    #[test]
    fn lock_that_is_a_rule_input_keeps_its_owner() {
        let scratch = crate_workspace("dx-changed-owned-lock-");
        let workspace = scratch.path();
        write(workspace, "BUILD.bazel", "");
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&cargo_lock("1.0.1")),
            graph(&[
                ("//:vendor", &["//:Cargo.lock"]),
                ("//a:lib", &["@@rules_rust++crate+crates//:serde"]),
                ("//b:lib", &["//b:b.rs"]),
            ]),
        ]);
        let selection = resolve_affected(
            &[change("Cargo.lock", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            base(),
        )
        .expect("owned lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["//:vendor", "@@rules_rust++crate+crates//:serde"])
        );
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(query.calls().len(), 2, "one git show and one graph query");
    }

    #[test]
    fn affected_matches_plain_resolve_for_attributed_files() {
        let workspace_files = ["pkg/a.py", "pkg/b.py"];
        let rules: [(&str, &[&str]); 2] = [
            ("//pkg:lib", &["//pkg:a.py", "//pkg:b.py"]),
            ("//pkg:extra", &["//pkg:a.py", "//pkg:b.py"]),
        ];
        let mut affected_targets = Vec::new();
        let mut resolved_targets = Vec::new();
        for _ in 0..2 {
            let scratch = dx_test_scratch::scratch("dx-changed-parity-");
            let workspace = scratch.path().to_path_buf();
            write(&workspace, "pkg/BUILD.bazel", "");
            write(&workspace, "pkg/a.py", "x = 1\n");
            write(&workspace, "pkg/b.py", "x = 1\n");
            let affected_query = FakeQuery::new(vec![graph(&rules)]);
            let affected = resolve_affected(
                &workspace_files
                    .iter()
                    .map(|path| change(path, dx_adopt::ChangeKind::Modified))
                    .collect::<Vec<_>>(),
                &workspace,
                &affected_query,
                &[],
                &[],
                None,
            )
            .expect("affected");
            assert!(affected.widened.is_empty());
            assert_eq!(affected_query.calls().len(), 1);
            assert!(affected.graph.is_some());
            affected_targets.push(affected.targets);
            let resolved_query = FakeQuery::new(vec![graph(&rules)]);
            let resolved = resolve(
                &workspace_files
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>(),
                &workspace,
                &resolved_query,
                &[],
            )
            .expect("resolve");
            resolved_targets.push(resolved.targets);
        }
        assert_eq!(affected_targets[0], affected_targets[1]);
        assert_eq!(affected_targets, resolved_targets);
    }

    #[test]
    fn resolve_rejects_deleted_paths_that_affected_widens() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-parity-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        let err = resolve(&strings(&["pkg/gone.py"]), &workspace, &NeverQuery, &[])
            .expect_err("deleted path has no worktree file");
        assert_eq!(
            err,
            ResolveError::PathNotFound {
                scope: "pkg/gone.py".to_owned(),
            }
        );
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("pkg/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("affected widens instead of failing");
        assert_eq!(selection.targets, strings(&["//pkg:all"]));
    }

    #[test]
    fn control_names_fail_before_any_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-control-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/ok.py", "x = 1\n");
        let err = resolve_affected(
            &[change("pkg/we\nird.py", dx_adopt::ChangeKind::Added)],
            &workspace,
            &NeverQuery,
            &[],
            &[],
            None,
        )
        .expect_err("control characters fail");
        assert_eq!(
            err,
            ResolveError::UnsupportedName {
                scope: "pkg/we\nird.py".to_owned(),
            }
        );
    }

    #[test]
    fn escaping_paths_fail_before_any_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-escape-");
        let workspace = scratch.path().to_path_buf();
        for scope in ["../escape.py", "pkg/../../escape.py", "/abs/path.py"] {
            let err = resolve_affected(
                &[change(scope, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &NeverQuery,
                &[],
                &[],
                None,
            )
            .expect_err("escape fails");
            assert_eq!(
                err,
                ResolveError::OutsideWorkspace {
                    scope: scope.to_owned(),
                },
                "{scope}"
            );
        }
    }

    #[test]
    fn files_without_enclosing_package_widen_to_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-no-package-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "docs/guide.md", "# guide\n");
        let selection = resolve_affected(
            &[change("docs/guide.md", dx_adopt::ChangeKind::Added)],
            &workspace,
            &NeverQuery,
            &[],
            &[],
            None,
        )
        .expect("no package widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(
            selection.widened[0].reason,
            "no enclosing package for docs/guide.md"
        );
    }

    #[test]
    fn nested_packages_widen_to_the_nearest_pattern() {
        let scratch = dx_test_scratch::scratch("dx-changed-nested-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "outer/BUILD.bazel", "");
        write(&workspace, "outer/inner/BUILD.bazel", "");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("outer/inner/gone.py", dx_adopt::ChangeKind::Deleted)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("nested widens");
        assert_eq!(selection.targets, strings(&["//outer/inner:all"]));
    }

    fn bzl_workspace(prefix: &str) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(prefix);
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", "module(name = \"demo\")\n");
        write(workspace, "tools/BUILD.bazel", "");
        write(workspace, "tools/defs.bzl", "x = 1\n");
        write(
            workspace,
            "app/BUILD.bazel",
            "load(\"//tools:defs.bzl\", \"x\")\n",
        );
        write(workspace, "app/main.py", "x = 1\n");
        scratch
    }

    fn bzl_resolve(
        workspace: &Path,
        changes: &[dx_adopt::GitChange],
        query: &FakeQuery,
        aspects: &[&str],
    ) -> AffectedSelection {
        let selection = resolve_affected(changes, workspace, query, &[], &strings(aspects), None)
            .expect("bzl resolves");
        for (argv, _) in query.calls() {
            assert!(
                !argv.iter().any(|arg| arg.contains("rbuildfiles")),
                "{argv:?}"
            );
        }
        selection
    }

    fn widened(entries: &[(&str, &str)]) -> Vec<WidenedScope> {
        entries
            .iter()
            .map(|(scope, reason)| WidenedScope {
                scope: (*scope).to_owned(),
                reason: (*reason).to_owned(),
                repos: Vec::new(),
            })
            .collect()
    }

    #[test]
    fn bzl_loaded_by_one_package_selects_only_that_package() {
        let scratch = bzl_workspace("dx-changed-bzl-one-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//app:all"]));
        assert_eq!(
            selection.widened,
            widened(&[("//app:all", "loads tools/defs.bzl")])
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn scl_loaded_through_bzl_selects_only_that_package() {
        let scratch = bzl_workspace("dx-changed-scl-one-");
        let workspace = scratch.path();
        write(workspace, "tools/x.scl", "X = 1\n");
        write(workspace, "tools/defs.bzl", "load(\":x.scl\", \"X\")\n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/x.scl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//app:all"]));
        assert_eq!(
            selection.widened,
            widened(&[("//app:all", "loads tools/x.scl")])
        );
    }

    #[test]
    fn deleted_scl_selects_packages_that_still_load_it() {
        let scratch = bzl_workspace("dx-changed-scl-deleted-");
        let workspace = scratch.path();
        write(
            workspace,
            "app/BUILD.bazel",
            "load(\"//tools:x.scl\", \"X\")\n",
        );
        let query = FakeQuery::new(Vec::new());
        let selection = bzl_resolve(
            workspace,
            &[change("tools/x.scl", dx_adopt::ChangeKind::Deleted)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//app:all"]));
        assert_eq!(
            selection.widened,
            widened(&[("//app:all", "loads tools/x.scl")])
        );
    }

    #[test]
    fn bzl_loaded_by_nothing_selects_nothing() {
        let scratch = bzl_workspace("dx-changed-bzl-none-");
        let workspace = scratch.path();
        write(workspace, "app/BUILD.bazel", "");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert!(selection.targets.is_empty(), "{:?}", selection.targets);
        assert!(selection.widened.is_empty());
    }

    #[test]
    fn bzl_owned_by_a_rule_keeps_its_owner() {
        let scratch = bzl_workspace("dx-changed-bzl-owned-");
        let workspace = scratch.path();
        write(workspace, "app/BUILD.bazel", "");
        let query = FakeQuery::new(vec![graph(&[("//tools:defs", &["//tools:defs.bzl"])])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//tools:defs"]));
        assert!(selection.widened.is_empty());
    }

    #[test]
    fn bzl_used_by_module_extension_widens() {
        let scratch = bzl_workspace("dx-changed-bzl-extension-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:defs.bzl\", \"x\")\n",
        );
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened,
            widened(&[("//...", "module extension input tools/defs.bzl")])
        );
    }

    #[test]
    fn extension_helper_bzl_widens() {
        let scratch = bzl_workspace("dx-changed-bzl-helper-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\n",
        );
        write(workspace, "tools/ext.bzl", "load(\":defs.bzl\", \"x\")\n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened,
            widened(&[("//...", "module extension input tools/defs.bzl")])
        );
    }

    #[test]
    fn aspect_input_bzl_widens() {
        let scratch = bzl_workspace("dx-changed-bzl-aspect-");
        let workspace = scratch.path();
        write(
            workspace,
            "quality/aspects.bzl",
            "load(\"//tools:defs.bzl\", \"x\")\n",
        );
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &["@demo//quality:aspects.bzl%lint_aspect"],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened,
            widened(&[("//...", "aspect input tools/defs.bzl")])
        );
    }

    #[test]
    fn toolchain_input_bzl_widens() {
        let scratch = bzl_workspace("dx-changed-bzl-toolchain-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\"//tools/tc:all\")\n",
        );
        write(
            workspace,
            "tools/tc/BUILD.bazel",
            "load(\"//tools:defs.bzl\", \"x\")\n",
        );
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened,
            widened(&[("//...", "toolchain input tools/defs.bzl")])
        );
    }

    #[test]
    fn unreadable_module_widens_bzl_changes() {
        let scratch = bzl_workspace("dx-changed-bzl-bad-module-");
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", "module(name = \n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1);
        assert!(
            selection.widened[0]
                .reason
                .starts_with("cannot read load graph: MODULE.bazel"),
            "{:?}",
            selection.widened
        );
    }

    #[test]
    fn unparsable_build_file_widens_bzl_changes() {
        let scratch = bzl_workspace("dx-changed-bzl-bad-build-");
        let workspace = scratch.path();
        write(workspace, "app/BUILD.bazel", "load(\"//tools:defs.bzl\"\n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].scope, "//...");
        assert!(
            selection.widened[0]
                .reason
                .starts_with("cannot read load graph: app/BUILD.bazel"),
            "{:?}",
            selection.widened
        );
    }

    #[test]
    fn deleted_bzl_loaded_by_nothing_selects_nothing() {
        let scratch = bzl_workspace("dx-changed-bzl-deleted-");
        let workspace = scratch.path();
        let query = FakeQuery::new(Vec::new());
        let selection = bzl_resolve(
            workspace,
            &[change("tools/gone.bzl", dx_adopt::ChangeKind::Deleted)],
            &query,
            &[],
        );
        assert!(selection.targets.is_empty(), "{:?}", selection.targets);
        assert!(selection.widened.is_empty());
        assert!(query.calls().is_empty());
    }

    #[test]
    fn deleted_bzl_still_loaded_selects_its_loader() {
        let scratch = bzl_workspace("dx-changed-bzl-deleted-loaded-");
        let workspace = scratch.path();
        std::fs::remove_file(workspace.join("tools/defs.bzl")).expect("remove");
        let query = FakeQuery::new(Vec::new());
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Deleted)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//app:all"]));
        assert_eq!(
            selection.widened,
            widened(&[("//app:all", "loads tools/defs.bzl")])
        );
    }

    #[test]
    fn mixed_bzl_and_source_changes_get_per_package_reasons() {
        let scratch = bzl_workspace("dx-changed-bzl-mixed-");
        let workspace = scratch.path();
        write(workspace, "tools/more.bzl", "y = 1\n");
        write(workspace, "BUILD", "load(\"//tools:more.bzl\", \"y\")\n");
        write(
            workspace,
            "lib/BUILD.bazel",
            "load(\"//tools:defs.bzl\", \"x\")\nload(\"//tools:more.bzl\", \"y\")\n",
        );
        let query = FakeQuery::new(vec![graph(&[("//app:bin", &["//app:main.py"])])]);
        let selection = bzl_resolve(
            workspace,
            &[
                change("tools/more.bzl", dx_adopt::ChangeKind::Added),
                change("app/main.py", dx_adopt::ChangeKind::Modified),
                change("tools/defs.bzl", dx_adopt::ChangeKind::Modified),
            ],
            &query,
            &[],
        );
        assert_eq!(
            selection.targets,
            strings(&["//:all", "//app:all", "//app:bin", "//lib:all"])
        );
        assert_eq!(
            selection.widened,
            widened(&[
                ("//:all", "loads tools/more.bzl"),
                ("//app:all", "loads tools/defs.bzl"),
                ("//lib:all", "loads tools/defs.bzl, tools/more.bzl"),
            ])
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn unrelated_unowned_files_stay_in_their_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-unrelated-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/notes.txt", "x\n");
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = resolve_affected(
            &[change("pkg/notes.txt", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("unowned");
        assert_eq!(selection.targets, strings(&["//pkg/..."]));
    }

    #[test]
    fn spaced_names_resolve_verbatim() {
        let scratch = dx_test_scratch::scratch("dx-changed-spaced-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/space name.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[("//pkg:lib", &["//pkg:space name.py"])])]);
        let selection = resolve_affected(
            &[change("pkg/space name.py", dx_adopt::ChangeKind::Added)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("spaced resolves");
        assert_eq!(selection.targets, strings(&["//pkg:lib"]));
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn ownership_failures_stay_query_failed() {
        let scratch = dx_test_scratch::scratch("dx-changed-owner-fail-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/a.py", "x = 1\n");
        let query = FakeQuery::new(vec![FakeQuery::failed("ERROR: no such package\nmore\n")]);
        let err = resolve_affected(
            &[change("pkg/a.py", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect_err("failed");
        assert_eq!(
            err,
            ResolveError::QueryFailed {
                label: "kind('rule', //...)".to_owned(),
                failure: QueryFailure::exit_for_test(2, "ERROR: no such package\nmore"),
            }
        );
    }

    #[test]
    fn partially_owned_batches_widen_only_the_orphans() {
        let scratch = dx_test_scratch::scratch("dx-changed-partial-");
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "");
        write(&workspace, "pkg/owned.py", "x = 1\n");
        write(&workspace, "pkg/orphan.py", "x = 1\n");
        let query = FakeQuery::new(vec![graph(&[("//pkg:lib", &["//pkg:owned.py"])])]);
        let selection = resolve_affected(
            &[
                change("pkg/owned.py", dx_adopt::ChangeKind::Modified),
                change("pkg/orphan.py", dx_adopt::ChangeKind::Modified),
            ],
            &workspace,
            &query,
            &[],
            &[],
            None,
        )
        .expect("partial widens");
        assert_eq!(selection.targets, strings(&["//pkg/...", "//pkg:lib"]));
        assert_eq!(selection.widened.len(), 1);
        assert_eq!(selection.widened[0].reason, "unowned pkg/orphan.py");
    }

    #[test]
    fn working_tree_collects_staged_unstaged_and_untracked() {
        let scratch = dx_test_scratch::scratch("dx-changed-worktree-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/b.py\0".to_vec()),
            FakeQuery::raw(b"pkg/c.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        let mut paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["pkg/a.py", "pkg/b.py", "pkg/c.py"]);
        let untracked = changes
            .iter()
            .find(|change| change.path == "pkg/c.py")
            .expect("untracked");
        assert_eq!(untracked.kind, dx_adopt::ChangeKind::Added);
        let calls = query.calls();
        assert_eq!(calls.len(), 3);
        assert!(calls[0].0.contains(&"--cached".to_owned()), "{calls:?}");
        assert!(!calls[1].0.contains(&"--cached".to_owned()), "{calls:?}");
        assert!(calls[2].0.contains(&"ls-files".to_owned()), "{calls:?}");
        assert!(
            calls[..2]
                .iter()
                .all(|(argv, _)| argv.contains(&"--relative".to_owned())),
            "{calls:?}"
        );
        assert!(
            calls
                .iter()
                .all(|(argv, _)| argv.contains(&"-z".to_owned())),
            "{calls:?}"
        );
    }

    #[test]
    fn working_tree_dedupes_across_inputs() {
        let scratch = dx_test_scratch::scratch("dx-changed-worktree-dedupe-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"pkg/a.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "pkg/a.py");
    }

    #[test]
    fn untracked_dx_state_is_not_a_change() {
        let scratch = dx_test_scratch::scratch("dx-changed-untracked-state-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(Vec::new()),
            FakeQuery::raw(Vec::new()),
            FakeQuery::raw(b".dx/leases/process-groups/1-7.lock\0pkg/c.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        let paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(paths, vec!["pkg/c.py"]);
    }

    #[test]
    fn untracked_names_must_be_utf8() {
        let scratch = dx_test_scratch::scratch("dx-changed-untracked-utf8-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(vec![0xff, 0x00]),
        ]);
        let err =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect_err("utf8");
        assert!(err.contains("UTF-8"), "{err}");
    }

    #[test]
    fn range_diffs_base_against_head() {
        let scratch = dx_test_scratch::scratch("dx-changed-range-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::raw(b"M\0pkg/a.py\0".to_vec())]);
        let changes = collect_range(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            "base-sha",
            "head-sha",
        )
        .expect("range");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "pkg/a.py");
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains(&"base-sha".to_owned()), "{calls:?}");
        assert!(calls[0].0.contains(&"head-sha".to_owned()), "{calls:?}");
        assert!(calls[0].0.contains(&"--relative".to_owned()), "{calls:?}");
        assert!(calls[0].0.contains(&"-z".to_owned()), "{calls:?}");
    }

    #[test]
    fn merge_base_returns_the_trimmed_sha() {
        let scratch = dx_test_scratch::scratch("dx-changed-merge-base-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::raw(b"  abc123\n".to_vec())]);
        let base = merge_base(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            "main",
            "HEAD",
        )
        .expect("base");
        assert_eq!(base, "abc123");
        let calls = query.calls();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].0.contains(&"merge-base".to_owned()), "{calls:?}");
    }

    #[test]
    fn merge_base_rejects_empty_and_ambiguous_output() {
        for (name, stdout) in [("empty", b"".to_vec()), ("two", b"a\nb\n".to_vec())] {
            let scratch = dx_test_scratch::scratch("dx-changed-merge-base-bad-");
            let workspace = scratch.path().to_path_buf();
            let query = FakeQuery::new(vec![FakeQuery::raw(stdout)]);
            let err = merge_base(
                Path::new("/hermetic/git"),
                &workspace,
                &query,
                "main",
                "HEAD",
            )
            .expect_err("bad base");
            assert!(err.contains("merge-base"), "{name}: {err}");
        }
    }

    #[test]
    fn shallow_clone_turns_diff_failure_into_actionable_error() {
        let scratch = dx_test_scratch::scratch("dx-changed-shallow-");
        let workspace = scratch.path().to_path_buf();
        std::fs::create_dir_all(workspace.join(".git")).expect("git dir");
        std::fs::write(workspace.join(".git/shallow"), "abc\n").expect("shallow");
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: no such commit")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("shallow");
        assert!(err.contains("shallow"), "{err}");
        assert!(err.contains("fetch --unshallow"), "{err}");
        assert_eq!(query.calls().len(), 1, "shallow needs no extra process");
    }

    #[test]
    fn worktree_shallow_marker_is_found_through_the_gitdir_pointer() {
        let scratch = dx_test_scratch::scratch("dx-changed-shallow-wt-");
        let workspace = scratch.path().to_path_buf();
        let common = scratch.path().join("common.git");
        let gitdir = common.join("worktrees/wt");
        std::fs::create_dir_all(&gitdir).expect("gitdir");
        std::fs::write(
            workspace.join(".git"),
            format!("gitdir: {}\n", gitdir.to_string_lossy()),
        )
        .expect("pointer");
        std::fs::write(common.join("worktrees/wt/commondir"), "../..\n").expect("commondir");
        std::fs::write(common.join("shallow"), "abc\n").expect("shallow");
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: no such commit")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("shallow worktree");
        assert!(err.contains("shallow"), "{err}");
    }

    #[test]
    fn non_shallow_diff_failure_stays_plain() {
        let scratch = dx_test_scratch::scratch("dx-changed-diff-fail-");
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: bad revision")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("plain failure");
        assert!(err.contains("git diff failed"), "{err}");
        assert!(err.contains("bad revision"), "{err}");
    }

    #[test]
    fn override_changes_need_no_git_without_outside_overrides() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-none-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"x\", path = \"third_party/x\")\n",
        );
        let changes =
            local_override_changes(Path::new("/hermetic/git"), &workspace, &NeverQuery, "base")
                .expect("no overrides");
        assert!(changes.is_empty());
    }

    #[test]
    fn override_changes_list_files_under_outside_overrides() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\nlocal_path_override(module_name = \"far\", path = \"../../far\")\n",
        );
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"sub/\n".to_vec()),
            FakeQuery::raw(b"lib/x.rs\0".to_vec()),
            FakeQuery::raw(b"lib/new.rs\0".to_vec()),
        ]);
        let changes =
            local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "base-sha")
                .expect("override changes");
        assert_eq!(changes, strings(&["../lib/new.rs", "../lib/x.rs"]));
        let calls = query.calls();
        assert_eq!(calls.len(), 3, "{calls:?}");
        assert!(
            calls[0].0.contains(&"--show-prefix".to_owned()),
            "{calls:?}"
        );
        assert!(calls[1].0.contains(&"base-sha".to_owned()), "{calls:?}");
        assert!(calls[1].0.ends_with(&[":(top)lib".to_owned()]), "{calls:?}");
        assert!(calls[2].0.contains(&"--full-name".to_owned()), "{calls:?}");
    }

    #[test]
    fn override_changes_skip_the_override_modules_dx_state() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-dx-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\n",
        );
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"sub/\n".to_vec()),
            FakeQuery::raw(b"lib/x.rs\0".to_vec()),
            FakeQuery::raw(b"lib/.dx/leases/process-groups/1-7.lock\0lib/a/.dx/y\0".to_vec()),
        ]);
        let changes = local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "b")
            .expect("override changes");
        assert_eq!(changes, strings(&["../lib/a/.dx/y", "../lib/x.rs"]));
    }

    #[test]
    fn override_changes_skip_the_workspace_dx_state_under_an_ancestor_override() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-anc-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"up\", path = \"../../a\")\n",
        );
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"a/b/\n".to_vec()),
            FakeQuery::raw(b"a/x.rs\0".to_vec()),
            FakeQuery::raw(b"a/b/.dx/leases/process-groups/1-7.lock\0".to_vec()),
        ]);
        let changes = local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "b")
            .expect("override changes");
        assert_eq!(changes, strings(&["../../a/x.rs"]));
    }

    #[test]
    fn override_changes_skip_overrides_outside_the_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-outside-");
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\n",
        );
        let query = FakeQuery::new(vec![FakeQuery::raw(b"\n".to_vec())]);
        let changes =
            local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "base")
                .expect("outside repo");
        assert!(changes.is_empty());
        assert_eq!(query.calls().len(), 1);
    }
}
