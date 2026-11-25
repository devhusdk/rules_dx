use std::path::Path;

use std::collections::{BTreeMap, BTreeSet};

use super::extension_scope::{self, Names, Scope};
use super::lock_inputs::{self, Hub, LockChange, LockKind};
use super::module_diff::{self, Named};
use super::module_inputs;
use super::rc_diff;
use super::{PackageCache, QueryRunner, ResolveError, RuleGraph};
use dx_adopt::dependency_sets::Ecosystem;
use dx_path::label::Label;

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

fn git_failure(what: &str, detail: &str) -> dx_error::DxError {
    dx_error::DxError::infra(format!("git {what} failed: {detail}"))
}

#[must_use]
pub fn git_exit_detail(stderr: &[u8], code: Option<i32>) -> String {
    let text = String::from_utf8_lossy(stderr);
    let cause = text
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("fatal:") || line.starts_with("error:"));
    let line = match cause {
        Some(line) => dx_output::first_diagnostic_line(line.as_bytes(), 200, "no Git diagnostic"),
        None => dx_output::first_diagnostic_line(stderr, 200, "no Git diagnostic"),
    };
    match code {
        Some(code) => format!("{line} (exit {code})"),
        None => format!("{line} (ended by a signal)"),
    }
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

#[must_use]
pub fn shallow_clone(workspace: &Path) -> bool {
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

#[must_use]
pub fn shallow_error() -> String {
    "git history is shallow: fetch full history with `git fetch --unshallow` or pass explicit scopes".to_owned()
}

#[must_use]
pub fn missing_base_shallow_error() -> String {
    "git history is shallow and the base branch is missing: set `fetch-depth: 0` in actions/checkout, run `git fetch origin <base>`, or pass --base".to_owned()
}

pub fn diff_name_status(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    extra: &[String],
) -> Result<Vec<dx_adopt::GitChange>, dx_error::DxError> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(extra.iter().cloned());
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("diff", &error.to_string()))?;
    if result.code != Some(0) {
        if shallow_clone(workspace) {
            return Err(dx_error::DxError::failure(shallow_error()));
        }
        return Err(git_failure(
            "diff",
            &git_exit_detail(&result.stderr, result.code),
        ));
    }
    dx_adopt::parse_name_status_nul(&result.stdout)
}

pub fn merge_base(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    first: &str,
    second: &str,
) -> Result<String, dx_error::DxError> {
    let mut argv = vec![git.to_string_lossy().into_owned()];
    argv.extend(
        ["merge-base", first, second]
            .iter()
            .map(ToString::to_string),
    );
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("merge-base", &error.to_string()))?;
    if result.code != Some(0) {
        if shallow_clone(workspace) {
            return Err(dx_error::DxError::failure(shallow_error()));
        }
        return Err(git_failure(
            "merge-base",
            &git_exit_detail(&result.stderr, result.code),
        ));
    }
    let text = std::str::from_utf8(&result.stdout)
        .map_err(|error| git_failure("merge-base", &format!("output is not UTF-8: {error}")))?;
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let base = lines
        .next()
        .ok_or_else(|| git_failure("merge-base", "output names no base commit"))?;
    if lines.next().is_some() {
        return Err(git_failure(
            "merge-base",
            "output names more than one base commit",
        ));
    }
    Ok(base.to_owned())
}

fn parse_untracked(output: &[u8]) -> Result<Vec<dx_adopt::GitChange>, dx_error::DxError> {
    let mut fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    let mut changes = Vec::with_capacity(fields.len());
    for field in fields {
        let path = std::str::from_utf8(field).map_err(|error| {
            dx_error::DxError::failure(format!(
                "git path is not UTF-8 ({error}): rename the file to a UTF-8 name"
            ))
        })?;
        if path.is_empty() {
            return Err(dx_error::DxError::failure(
                "git change list has an empty path".to_owned(),
            ));
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
) -> Result<Vec<dx_adopt::GitChange>, dx_error::DxError> {
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
        .map_err(|error| git_failure("ls-files", &error.to_string()))?;
    if result.code != Some(0) {
        return Err(git_failure(
            "ls-files",
            &git_exit_detail(&result.stderr, result.code),
        ));
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
) -> Result<Vec<dx_adopt::GitChange>, dx_error::DxError> {
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
) -> Result<Vec<String>, dx_error::DxError> {
    let mut command_line = vec![git.to_string_lossy().into_owned()];
    command_line.extend(args.iter().cloned());
    let result = runner
        .run_query(&command_line, workspace)
        .map_err(|error| git_failure(what, &error.to_string()))?;
    if result.code != Some(0) {
        return Err(git_failure(
            what,
            &git_exit_detail(&result.stderr, result.code),
        ));
    }
    let text = std::str::from_utf8(&result.stdout).map_err(|error| {
        dx_error::DxError::failure(format!(
            "git path is not UTF-8 ({error}): rename the file to a UTF-8 name"
        ))
    })?;
    let delimiter = if args.iter().any(|arg| arg == "-z") {
        '\0'
    } else {
        '\n'
    };
    Ok(text
        .split(delimiter)
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
) -> Result<Vec<String>, dx_error::DxError> {
    let text = std::fs::read_to_string(workspace.join("MODULE.bazel")).unwrap_or_default();
    let overrides: Vec<String> = dx_adopt::local_path_overrides(&text)
        .into_iter()
        .filter(|path| !Path::new(path).is_absolute())
        .map(|path| path.replace('\\', "/"))
        .filter(|path| path.split('/').any(|part| part == ".."))
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
    paths.sort_unstable();
    paths.dedup();
    Ok(paths)
}

fn file_name_of(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

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

fn workspace_rc_files(workspace: &Path, startup_options: &[String]) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut visited: BTreeSet<String> = BTreeSet::from([".bazelrc".to_owned()]);
    let mut stack = vec![".bazelrc".to_owned()];
    let env_get = |name: &str| std::env::var(name).ok();
    for path in dx_process::rc::explicit_rc_files(&env_get, startup_options) {
        let relative = path.strip_prefix(workspace).unwrap_or(&path);
        if relative.is_absolute() {
            continue;
        }
        let parts: Vec<String> = relative
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(p) => Some(p.to_string_lossy().into_owned()),
                std::path::Component::Prefix(_)
                | std::path::Component::RootDir
                | std::path::Component::CurDir
                | std::path::Component::ParentDir => None,
            })
            .collect();
        if parts.is_empty()
            || relative
                .components()
                .any(|c| c == std::path::Component::ParentDir)
        {
            continue;
        }
        let name = parts.join("/");
        seen.insert(name.clone());
        if visited.insert(name.clone()) {
            stack.push(name);
        }
    }
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
        "MODULE.bazel" | "MODULE.bazel.lock" | ".bazelversion" | ".bazeliskrc" | ".bazelignore"
    ) || name.starts_with(".bazelrc")
        || (name.ends_with(".MODULE.bazel") && !name.starts_with('.'))
        || rc_files.contains(path)
}

fn rc_keeps_builds(
    path: &str,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: Option<(&Path, &str)>,
) -> bool {
    let Some((git, base)) = base else {
        return false;
    };
    let Ok(head) = std::fs::read_to_string(workspace.join(path)) else {
        return false;
    };
    matches!(
        show_at_base(git, workspace, runner, base, path),
        Ok(Some(old)) if !rc_diff::changes_builds(&old, &head)
    )
}

fn is_lock_input(path: &str, hubs: &[Hub]) -> bool {
    let name = file_name_of(path);
    (Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lock"))
        && name != "MODULE.bazel.lock")
        || lock_inputs::classify(path, hubs) != LockKind::Unknown
}

fn fallback_inputs() -> [&'static str; 8] {
    [
        Ecosystem::Npm.lockfile(),
        "package-lock.json",
        Ecosystem::Go.manifest_name(),
        Ecosystem::Go.lockfile(),
        "go.work",
        "go.work.sum",
        Ecosystem::Maven.lockfile(),
        "requirements_lock.txt",
    ]
}

struct ModuleInputPaths {
    paths: BTreeSet<String>,
    routed: BTreeSet<String>,
    readers: BTreeMap<String, BTreeSet<(String, String)>>,
    fallback: bool,
}

impl ModuleInputPaths {
    fn contains(&self, path: &str) -> bool {
        self.paths.contains(path)
            || (self.fallback && fallback_inputs().contains(&file_name_of(path)))
    }
}

fn is_crate_lock_input(extension: Option<&(String, String)>, tag: &str) -> bool {
    extension.is_some_and(|(bzl, name)| {
        lock_inputs::is_crate_extension(bzl, name) && matches!(tag, "from_cargo" | "from_specs")
    })
}

fn module_inputs_of(workspace: &Path) -> (Vec<Hub>, ModuleInputPaths) {
    let hubs_of = |path: &Path| {
        std::fs::read_to_string(path).ok().and_then(|text| {
            let mut hubs = lock_inputs::crate_hubs(&text).ok()?;
            hubs.extend(lock_inputs::pip_hubs(&text));
            hubs.extend(lock_inputs::maven_hubs(&text));
            hubs.extend(lock_inputs::gem_hubs(&text).ok()?);
            Some(hubs)
        })
    };
    let root_hubs = hubs_of(&workspace.join("MODULE.bazel"));
    let mut hubs = root_hubs.clone().unwrap_or_default();
    let mut segments_ok = root_hubs.is_some();
    let mut inputs = ModuleInputPaths {
        paths: BTreeSet::new(),
        routed: BTreeSet::new(),
        readers: BTreeMap::new(),
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
            if let Some(extension) = &file.extension {
                let dir = file
                    .path
                    .strip_suffix(file_name_of(&file.path))
                    .unwrap_or_default();
                let siblings = if file_name_of(&file.path) == Ecosystem::Go.manifest_name() {
                    vec![format!("{dir}{}", Ecosystem::Go.lockfile())]
                } else {
                    Vec::new()
                };
                for path in siblings.into_iter().chain([file.path.clone()]) {
                    inputs
                        .readers
                        .entry(path)
                        .or_default()
                        .insert(extension.clone());
                }
            }
            if is_crate_lock_input(file.extension.as_ref(), &file.tag) {
                continue;
            }
            if file.tag == "include" || file.tag.ends_with("_override") || file.extension.is_some()
            {
                inputs.routed.insert(file.path.clone());
            }
            let name = file_name_of(&file.path);
            let sibling = if name == Ecosystem::Go.manifest_name() {
                Some(Ecosystem::Go.lockfile())
            } else if name == "go.work" {
                Some("go.work.sum")
            } else {
                None
            };
            if let Some(sibling) = sibling {
                let dir = file
                    .path
                    .strip_suffix(file_name_of(&file.path))
                    .unwrap_or_default();
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

pub(crate) fn show_at_base(
    git: &Path,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: &str,
    path: &str,
) -> Result<Option<String>, dx_error::DxError> {
    let argv = vec![
        git.to_string_lossy().into_owned(),
        "show".to_owned(),
        format!("{base}:./{path}"),
    ];
    let result = runner
        .run_query(&argv, workspace)
        .map_err(|error| git_failure("show", &error.to_string()))?;
    if result.code != Some(0) {
        let stderr = String::from_utf8_lossy(&result.stderr);
        if stderr.contains("does not exist") || stderr.contains("exists on disk, but not in") {
            return Ok(None);
        }
        return Err(git_failure(
            "show",
            &git_exit_detail(&result.stderr, result.code),
        ));
    }
    String::from_utf8(result.stdout)
        .map(Some)
        .map_err(|error| git_failure("show", &format!("output is not UTF-8: {error}")))
}

fn lock_change(
    path: &str,
    hubs: &[Hub],
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: Option<(&Path, &str)>,
) -> Result<Vec<(String, LockChange)>, dx_error::DxError> {
    let workspace_input = || dx_error::DxError::failure(format!("workspace input {path}"));
    let kind = lock_inputs::classify(path, hubs);
    if kind == LockKind::Unknown {
        return Err(workspace_input());
    }
    let Some((git, base)) = base else {
        return Err(workspace_input());
    };
    if !workspace.join(path).is_file() {
        return Err(workspace_input());
    }
    let head = std::fs::read_to_string(workspace.join(path)).map_err(|error| {
        dx_error::DxError::from_io(format!("cannot read {path}: {error}"), &error)
    })?;
    let base = show_at_base(git, workspace, runner, base, path)
        .map_err(|detail| dx_error::DxError::infra(format!("cannot read {path}: {detail}")))?
        .ok_or_else(workspace_input)?;
    let changed = if matches!(kind, LockKind::Go(_)) {
        lock_inputs::changed_go(path, &base, &head)
    } else {
        kind.changed(&base, &head)
    }
    .map_err(|detail| dx_error::DxError::infra(format!("cannot read {path}: {detail}")))?;
    Ok(kind
        .hubs()
        .iter()
        .map(|hub| (hub.clone(), changed.clone()))
        .collect())
}

const ROOT_MODULE: &str = "MODULE.bazel";
const ROOT_LOCK: &str = "MODULE.bazel.lock";

struct ModuleEdit {
    path: String,
    modules: BTreeSet<String>,
    scopes: BTreeSet<Scope>,
    loads: Vec<String>,
}

fn module_text(
    path: &str,
    changed: bool,
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: (&Path, &str),
) -> Result<(String, String), dx_error::DxError> {
    let workspace_input = || dx_error::DxError::failure(format!("workspace input {path}"));
    if !changed && path == ROOT_LOCK && !workspace.join(path).exists() {
        return Ok(("{}".to_owned(), "{}".to_owned()));
    }
    let head = std::fs::read_to_string(workspace.join(path))
        .map_err(|error| dx_error::DxError::failure(format!("workspace input {path}: {error}")))?;
    if !changed {
        return Ok((head.clone(), head));
    }
    let (git, base) = base;
    let before = show_at_base(git, workspace, runner, base, path)
        .map_err(|detail| dx_error::DxError::infra(format!("cannot read {path}: {detail}")))?
        .ok_or_else(workspace_input)?;
    Ok((before, head))
}

type ModuleFiles = (Vec<Option<String>>, Vec<dx_module_bazel::ModuleFile>);

fn module_files(
    text: &str,
    read: &dyn Fn(&str) -> Result<String, String>,
) -> Result<ModuleFiles, dx_error::DxError> {
    let read_include = |label: &str| {
        let rel = dx_module_bazel::label_path(label)
            .map_err(|error| format!("workspace input {label}: {error}"))?;
        read(&rel)
    };
    dx_module_bazel::load_with_includes_labeled(text, &read_include)
        .map(|files| files.into_iter().unzip())
        .map_err(|error| {
            dx_error::DxError::failure(format!("workspace input {ROOT_MODULE}: {error}"))
        })
}

#[expect(
    clippy::too_many_lines,
    reason = "one linear sequence of steps read top to bottom"
)]
fn module_change(
    paths: &[String],
    workspace: &Path,
    runner: &dyn QueryRunner,
    base: Option<(&Path, &str)>,
    startup_options: &[String],
) -> Result<ModuleEdit, dx_error::DxError> {
    let first = paths.first().map_or(ROOT_MODULE, String::as_str);
    let Some(base) = base else {
        return Err(dx_error::DxError::failure(format!(
            "workspace input {first}"
        )));
    };
    let changed = |path: &str| paths.iter().any(|changed| changed == path);
    let read_head = |rel: &str| {
        std::fs::read_to_string(workspace.join(rel))
            .map_err(|error| format!("workspace input {rel}: {error}"))
    };
    let read_base = |rel: &str| {
        if !changed(rel) {
            return read_head(rel);
        }
        show_at_base(base.0, workspace, runner, base.1, rel)
            .map_err(|detail| format!("cannot read {rel}: {detail}"))?
            .ok_or_else(|| format!("workspace input {rel}"))
    };
    let (base_module, head_module) =
        module_text(ROOT_MODULE, changed(ROOT_MODULE), workspace, runner, base)?;
    let (head_labels, head_files) = module_files(&head_module, &read_head)?;
    let (base_labels, base_files) = module_files(&base_module, &read_base)?;
    let diff = module_diff::module_diff(&base_files, &head_files)?;
    let mut modules: BTreeSet<String> = diff.deps.keys().cloned().collect();
    modules.extend(diff.modules);
    let mut scopes = diff.scopes;
    let includes: BTreeSet<String> = head_labels
        .iter()
        .chain(&base_labels)
        .flatten()
        .filter_map(|label| dx_module_bazel::label_path(label).ok())
        .collect();
    let mut named = module_diff::named_files(&head_files, &head_labels);
    for (path, targets) in module_diff::named_files(&base_files, &base_labels) {
        named.entry(path).or_default().extend(targets);
    }
    for path in paths {
        if path == ROOT_MODULE || path == ROOT_LOCK || includes.contains(path) {
            continue;
        }
        let targets = named
            .get(path)
            .ok_or_else(|| dx_error::DxError::failure(format!("workspace input {path}")))?;
        for target in targets {
            match target {
                Named::Scope(scope) => {
                    scopes.insert(scope.clone());
                }
                Named::Module(module) => {
                    modules.insert(module.clone());
                }
            }
        }
    }
    let (base_lock, head_lock) =
        module_text(ROOT_LOCK, changed(ROOT_LOCK), workspace, runner, base)?;
    let lock = module_inputs::lock_changes(&base_lock, &head_lock)?;
    module_inputs::lock_pins(&head_lock, &diff.deps)?;
    modules.extend(lock.modules);
    scopes.extend(lock.scopes);
    let names = Names::of(&head_files, extension_scope::ROOT);
    let mut affected = BTreeSet::new();
    if !modules.is_empty()
        || !lock.owned.is_empty()
        || scopes
            .iter()
            .any(|scope| scope.owner() != extension_scope::ROOT)
    {
        let read = module_cache(workspace, startup_options);
        let graph = module_inputs::module_graph(
            &module_inputs::registry_files(&head_lock)?,
            &read,
            &names.multiple,
        )?;
        let old: Vec<_> = module_inputs::registry_files(&base_lock)?
            .into_iter()
            .filter(|file| modules.contains(&file.name))
            .collect();
        let old_graph = module_inputs::module_graph(&old, &read, &names.multiple)?;
        affected = module_inputs::closure(&modules, &graph.deps);
        if let Some(module) = affected.iter().find(|module| {
            graph.registers.contains(*module) || old_graph.registers.contains(*module)
        }) {
            return Err(dx_error::DxError::failure(format!(
                "changed module {module} registers toolchains"
            )));
        }
        for module in &modules {
            if graph.opaque.contains(module) || old_graph.opaque.contains(module) {
                return Err(dx_error::DxError::failure(format!(
                    "changed module {module} uses an extension outside its deps"
                )));
            }
            for usages in [&graph.usages, &old_graph.usages] {
                scopes.extend(usages.get(module).into_iter().flatten().cloned());
            }
        }
        for scope in scopes
            .iter()
            .filter(|scope| scope.owner() != extension_scope::ROOT)
        {
            if let Some(reason) = module_inputs::shared_scope(&graph, scope, &affected) {
                return Err(dx_error::DxError::failure(reason));
            }
            if let Some((module, _)) = old_graph
                .registered
                .iter()
                .find(|(_, registered)| registered.iter().any(|known| known.overlaps(scope)))
            {
                return Err(dx_error::DxError::failure(format!(
                    "module {module} registers toolchains from {}",
                    scope.display()
                )));
            }
            if let Some(reason) = module_inputs::other_user(&old_graph, scope, &affected) {
                return Err(dx_error::DxError::failure(reason));
            }
        }
        let mut patched = affected.clone();
        if scopes
            .iter()
            .any(|scope| scope.owner() != extension_scope::ROOT)
        {
            patched.extend(graph.deps.keys().cloned());
        }
        let read_patch = |rel: &str| read_head(rel).ok();
        if let Some(reason) =
            module_inputs::overrides_that_widen(&head_files, &patched, &read_patch)
        {
            return Err(dx_error::DxError::failure(reason));
        }
        if let Some((key, _)) = lock
            .owned
            .iter()
            .find(|(_, module)| !affected.contains(module))
        {
            return Err(dx_error::DxError::failure(format!(
                "{ROOT_LOCK} changed {key}"
            )));
        }
    }
    if affected.is_empty() && scopes.is_empty() {
        return Ok(ModuleEdit {
            path: first.to_owned(),
            modules: affected,
            scopes,
            loads: Vec::new(),
        });
    }
    let owners = module_inputs::repo_owners(&head_files);
    let imported = module_diff::imported(&head_files, extension_scope::ROOT);
    let reached = |repo: &str| {
        owners
            .get(repo)
            .is_some_and(|owner| affected.contains(owner))
            || imported.get(repo).is_some_and(|scope| {
                scopes
                    .iter()
                    .any(|changed| module_diff::covers(changed, scope))
            })
    };
    for file in &head_files {
        for call in file.calls().iter().filter(|call| {
            matches!(
                call.name.as_str(),
                "register_toolchains" | "register_execution_platforms"
            )
        }) {
            if call.unpacked {
                return Err(dx_error::DxError::failure(format!(
                    "{ROOT_MODULE} {} has no literal target pattern",
                    call.name
                )));
            }
            for value in &call.positional {
                let dx_module_bazel::Value::Str(label) = value else {
                    return Err(dx_error::DxError::failure(format!(
                        "{ROOT_MODULE} {} has no literal target pattern",
                        call.name
                    )));
                };
                if module_inputs::registered_repo(&label.value).is_some_and(|repo| {
                    reached(repo)
                        || affected
                            .iter()
                            .any(|module| repo.starts_with(&format!("{module}+")))
                        || scopes.iter().any(|scope| scope.matches(repo))
                }) {
                    return Err(dx_error::DxError::failure(format!(
                        "{ROOT_MODULE} registers {}",
                        label.value
                    )));
                }
            }
        }
    }
    let mut loads: BTreeSet<String> = owners
        .keys()
        .chain(imported.keys())
        .filter(|repo| reached(repo))
        .cloned()
        .collect();
    for module in &affected {
        loads.insert(format!("{module}+"));
    }
    let path = if changed(ROOT_MODULE) {
        ROOT_MODULE
    } else if changed(ROOT_LOCK) {
        ROOT_LOCK
    } else {
        first
    };
    Ok(ModuleEdit {
        path: path.to_owned(),
        modules: affected,
        scopes,
        loads: loads.into_iter().collect(),
    })
}

fn lock_repos(
    path: &str,
    readers: &BTreeSet<(String, String)>,
    workspace: &Path,
    startup_options: &[String],
) -> Result<Vec<String>, dx_error::DxError> {
    let read_head = |rel: &str| {
        std::fs::read_to_string(workspace.join(rel))
            .map_err(|error| format!("workspace input {rel}: {error}"))
    };
    let text = read_head(ROOT_MODULE).map_err(dx_error::DxError::failure)?;
    let (_, files) = module_files(&text, &read_head)?;
    let names = Names::of(&files, extension_scope::ROOT);
    let mut scopes = BTreeSet::new();
    for (bzl, name) in readers {
        scopes.insert(names.extension(bzl, name).ok_or_else(|| {
            dx_error::DxError::failure(format!("{path} feeds extension {name} of {bzl}"))
        })?);
    }
    if scopes.is_empty() {
        return Err(dx_error::DxError::failure(format!(
            "{path} feeds no extension dx can name"
        )));
    }
    if scopes
        .iter()
        .any(|scope| scope.owner() != extension_scope::ROOT)
    {
        let lock = read_head(ROOT_LOCK).map_err(dx_error::DxError::failure)?;
        let read = module_cache(workspace, startup_options);
        let graph = module_inputs::module_graph(
            &module_inputs::registry_files(&lock)?,
            &read,
            &names.multiple,
        )?;
        for scope in &scopes {
            if let Some(reason) = module_inputs::shared_scope(&graph, scope, &BTreeSet::new()) {
                return Err(dx_error::DxError::failure(reason));
            }
        }
        let every: BTreeSet<String> = graph.deps.keys().cloned().collect();
        let read_patch = |rel: &str| read_head(rel).ok();
        if let Some(reason) = module_inputs::overrides_that_widen(&files, &every, &read_patch) {
            return Err(dx_error::DxError::failure(reason));
        }
    }
    Ok(module_diff::imported(&files, extension_scope::ROOT)
        .into_iter()
        .filter(|(_, repo)| scopes.iter().any(|scope| module_diff::covers(scope, repo)))
        .map(|(name, _)| name)
        .collect())
}

fn module_cache(workspace: &Path, startup_options: &[String]) -> impl Fn(&str) -> Option<String> {
    let caches = dx_process::repository_caches(
        &|name| std::env::var(name).ok(),
        workspace,
        &|path| std::fs::read_to_string(path).ok(),
        startup_options,
    );
    move |hash: &str| module_inputs::cached(&caches, hash)
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
    Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bzl") || ext.eq_ignore_ascii_case("scl"))
}

fn package_all(label: &str) -> String {
    match Label::parse(label) {
        Some(parsed) if !parsed.relative => format!("{}//{}:all", parsed.repo, parsed.package),
        _ => "//...".to_owned(),
    }
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

type Extensions = BTreeMap<String, BTreeSet<String>>;

fn widen_impact(
    impact: Result<BTreeMap<String, dx_adopt::BzlImpact>, dx_error::DxError>,
    prefix: &str,
    widened: &mut Vec<WidenedScope>,
    extensions: &mut Extensions,
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
            dx_adopt::BzlImpact::Extensions { names, packages } => {
                for package in packages {
                    loaders
                        .entry(package.clone())
                        .or_default()
                        .push(format!("{prefix}{path}"));
                }
                for name in names {
                    extensions
                        .entry(name.clone())
                        .or_default()
                        .insert(format!("{prefix}{path}"));
                }
            }
        }
    }
    for (package, files) in loaders {
        push_widened(widened, package, format!("loads {}", files.join(", ")));
    }
}

fn widen_repo_users(
    workspace: &Path,
    aspect_labels: &[String],
    graph: &RuleGraph,
    seeds: &[String],
    what: &str,
    widened: &mut Vec<WidenedScope>,
) {
    if seeds.is_empty() {
        return;
    }
    if let Some(toolchain) = graph
        .reached_of_kind(seeds, |kind| kind == "toolchain")
        .first()
    {
        push_widened(
            widened,
            "//...".to_owned(),
            format!("{what} reach toolchain {toolchain}"),
        );
    }
    let implicit = match dx_adopt::implicit_dependencies(workspace, aspect_labels) {
        Ok(implicit) => implicit,
        Err(detail) => {
            push_widened(
                widened,
                "//...".to_owned(),
                format!("cannot read load graph: {detail}"),
            );
            return;
        }
    };
    if let Some(used) = graph
        .reached_rules(seeds)
        .iter()
        .find(|label| implicit.contains(label.as_str()))
    {
        push_widened(
            widened,
            "//...".to_owned(),
            format!("{what} reach {used}, which aspects or bazelrc flags use"),
        );
    }
}

fn widen_implicit_users(
    workspace: &Path,
    aspect_labels: &[String],
    graph: &RuleGraph,
    labels: &[String],
    owners: &[String],
    widened: &mut Vec<WidenedScope>,
) {
    if let Some(toolchain) = graph
        .reached_of_kind(owners, |kind| kind == "toolchain")
        .first()
    {
        push_widened(
            widened,
            "//...".to_owned(),
            format!("changed files reach toolchain {toolchain}"),
        );
    }
    let implicit = match dx_adopt::implicit_dependencies(workspace, aspect_labels) {
        Ok(implicit) => implicit,
        Err(detail) => {
            push_widened(
                widened,
                "//...".to_owned(),
                format!("cannot read load graph: {detail}"),
            );
            return;
        }
    };
    let reached = graph.reached_rules(owners);
    if let Some(used) = labels
        .iter()
        .chain(&reached)
        .find(|label| implicit.contains(label.as_str()))
    {
        push_widened(
            widened,
            "//...".to_owned(),
            format!("changed files reach {used}, which aspects or bazelrc flags use"),
        );
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "one linear sequence of steps read top to bottom"
)]
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
    let rc_files = workspace_rc_files(workspace, startup_options);
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
            if path == ROOT_MODULE || path == ROOT_LOCK || inputs.routed.contains(&path) {
                module_paths.push(path);
                continue;
            }
            if (file_name_of(&path).starts_with(".bazelrc") || rc_files.contains(&path))
                && rc_keeps_builds(&path, workspace, runner, base)
            {
                match cache.file_label(workspace, &path, &path) {
                    Ok(label) => candidates.push((path, label, Fallback::Keep)),
                    Err(ResolveError::NotAPackage { .. }) => {}
                    Err(error) => return Err(error),
                }
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
    bzl_paths.sort_unstable();
    bzl_paths.dedup();
    let mut extensions = Extensions::new();
    if !bzl_paths.is_empty() {
        let impact = dx_adopt::bzl_impact(workspace, &bzl_paths, aspect_labels);
        widen_impact(impact, "", &mut widened, &mut extensions);
    }
    lock_paths.sort_unstable();
    lock_paths.dedup();
    let mut narrowed: Vec<(String, Vec<(String, LockChange)>)> = Vec::new();
    for path in lock_paths {
        match lock_change(&path, &hubs, workspace, runner, base) {
            Err(reason) => push_widened(
                &mut widened,
                "//...".to_owned(),
                reason.message().to_owned(),
            ),
            Ok(narrowing) => {
                let readers = inputs.readers.get(&path).cloned().unwrap_or_default();
                let repos = match lock_repos(&path, &readers, workspace, startup_options) {
                    Ok(repos) => repos,
                    Err(reason) => {
                        push_widened(
                            &mut widened,
                            "//...".to_owned(),
                            reason.message().to_owned(),
                        );
                        continue;
                    }
                };
                let impact = dx_adopt::repo_impact(workspace, &repos, aspect_labels);
                widen_impact(impact, "@", &mut widened, &mut extensions);
                match cache.file_label(workspace, &path, &path) {
                    Ok(label) => candidates.push((path.clone(), label, Fallback::Keep)),
                    Err(ResolveError::NotAPackage { .. }) => {}
                    Err(error) => return Err(error),
                }
                narrowed.push((path, narrowing));
            }
        }
    }
    module_paths.sort_unstable();
    module_paths.dedup();
    let mut update = None;
    if !module_paths.is_empty() {
        match module_change(&module_paths, workspace, runner, base, startup_options) {
            Err(reason) => push_widened(
                &mut widened,
                "//...".to_owned(),
                reason.message().to_owned(),
            ),
            Ok(found) => {
                let impact = dx_adopt::repo_impact(workspace, &found.loads, aspect_labels);
                widen_impact(impact, "@", &mut widened, &mut extensions);
                for path in module_paths
                    .iter()
                    .filter(|path| *path != ROOT_MODULE && *path != ROOT_LOCK)
                {
                    match cache.file_label(workspace, path, path) {
                        Ok(label) => candidates.push((path.clone(), label, Fallback::Keep)),
                        Err(ResolveError::NotAPackage { .. }) => {}
                        Err(error) => return Err(error),
                    }
                }
                update = Some(found)
                    .filter(|found| !found.modules.is_empty() || !found.scopes.is_empty());
            }
        }
    }
    candidates.sort();
    candidates.dedup();
    let mut graph = None;
    if !candidates.is_empty() || !narrowed.is_empty() || update.is_some() || !extensions.is_empty()
    {
        let loaded = RuleGraph::load(workspace, runner, startup_options)?;
        for (name, files) in &extensions {
            let scope = Scope::extension(extension_scope::ROOT, name);
            let seeds =
                extension_scope::seeds(loaded.external_inputs(), &BTreeSet::from([scope.clone()]));
            let files: Vec<&str> = files.iter().map(String::as_str).collect();
            widen_repo_users(
                workspace,
                aspect_labels,
                &loaded,
                &seeds,
                &format!("repos of module extension {name}"),
                &mut widened,
            );
            targets.extend(seeds);
            let repos = vec![scope.display()];
            widened.push(WidenedScope {
                scope: rdeps_scope(&repos),
                reason: format!("changed module extension {name} in {}", files.join(", ")),
                repos,
            });
        }
        if let Some(update) = &update {
            let mut seeds = if update.modules.is_empty() {
                Vec::new()
            } else {
                module_inputs::module_seeds(loaded.external_inputs(), &update.modules)
            };
            seeds.extend(extension_scope::seeds(
                loaded.external_inputs(),
                &update.scopes,
            ));
            widen_repo_users(
                workspace,
                aspect_labels,
                &loaded,
                &seeds,
                &format!("changed modules in {}", update.path),
                &mut widened,
            );
            targets.extend(seeds);
            let repos: Vec<String> = update
                .modules
                .iter()
                .map(|module| format!("@{module}"))
                .chain(update.scopes.iter().map(Scope::display))
                .collect();
            widened.push(WidenedScope {
                scope: rdeps_scope(&repos),
                reason: format!("changed modules in {}", update.path),
                repos,
            });
        }
        let gem_hubs: BTreeSet<&String> = hubs
            .iter()
            .filter(|hub| hub.gems)
            .map(|hub| &hub.name)
            .collect();
        for (path, changed) in &narrowed {
            let mut repos: BTreeSet<String> = BTreeSet::new();
            let mut seeds: Vec<String> = Vec::new();
            let go_changed = changed.iter().any(|(_, change)| change.go);
            for (hub, change) in changed {
                let LockChange {
                    crates,
                    aliases,
                    pip,
                    maven,
                    go,
                    npm,
                } = change;
                if gem_hubs.contains(hub) {
                    seeds.extend(lock_inputs::gem_seeds(loaded.external_inputs(), hub));
                    repos.extend(crates.keys().map(|name| format!("@{hub}//:{name}")));
                    continue;
                }
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
                if *go {
                    seeds.extend(lock_inputs::go_seeds(loaded.external_inputs(), hub));
                    repos.insert(format!("@@*+{hub}+*"));
                    continue;
                }
                seeds.extend(lock_inputs::seeds(
                    loaded.external_inputs(),
                    hub,
                    crates,
                    aliases,
                ));
                repos.extend(crates.keys().map(|name| format!("@{hub}//:{name}")));
                repos.extend(aliases.iter().map(|name| format!("@{hub}//:{name}")));
            }
            widen_repo_users(
                workspace,
                aspect_labels,
                &loaded,
                &seeds,
                &format!("changed crates in {path}"),
                &mut widened,
            );
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
        widen_implicit_users(
            workspace,
            aspect_labels,
            loaded,
            &labels,
            &owners,
            &mut widened,
        );
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
    use dx_adopt::dependency_sets::Ecosystem;

    fn manifest_of(ecosystem: Ecosystem) -> &'static str {
        ecosystem.manifest_name()
    }

    use super::*;
    use crate::resolve::{FakeQuery, NeverQuery, QueryFailure, QueryResult, resolve};
    use dx_test_scratch::write;

    #[test]
    fn package_all_keeps_the_repo_and_drops_the_name() {
        for (label, scope) in [
            ("//p:n", "//p:all"),
            ("//:n", "//:all"),
            ("//a/b", "//a/b:all"),
            ("@r//p:n", "@r//p:all"),
            ("@@r+//p:n", "@@r+//p:all"),
            (":n", "//..."),
            ("p:n", "//..."),
        ] {
            assert_eq!(package_all(label), scope, "{label}");
        }
    }
    use dx_testing::cli::strings;

    fn hub_label(ecosystem: &str, rest: &str) -> String {
        let hub =
            dx_families::hub(ecosystem).unwrap_or_else(|| panic!("setup.hubs entry {ecosystem}"));
        format!("@{}{rest}", hub.hub)
    }

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
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
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
        assert_ne!(selection.widened, [] as [WidenedScope; 0]);
        assert!(selection.graph.is_some());
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn deleted_file_still_named_by_a_rule_selects_it() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-named-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
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
        assert_eq!(selection.widened, [] as [WidenedScope; 0]);
    }

    #[test]
    fn deleted_build_file_widens_to_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-build-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-nopkg-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-root-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD.bazel", "").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-rename-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-build-owned-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/space name.py", "x = 1\n").unwrap();
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
        assert_ne!(selection.widened, [] as [WidenedScope; 0]);
        assert_eq!(query.calls().len(), 1, "one graph query");
        assert!(selection.graph.is_some());
    }

    #[test]
    fn unowned_build_inputs_widen_to_their_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-unowned-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
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
        reasons.sort_unstable();
        assert_eq!(
            reasons,
            vec!["build file pkg/BUILD.bazel", "unowned pkg/a.py"]
        );
    }

    #[test]
    fn build_only_edit_with_sole_corpus_owner_selects_package() {
        let scratch = dx_test_scratch::scratch("dx-changed-build-corpus-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-build-added-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/new/BUILD.bazel", "").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-build-root-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "BUILD", "").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-single-unowned-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
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
            Ecosystem::Cargo.lockfile(),
            ".bazelrc",
            ".bazelrc.user",
            ".bazeliskrc",
            ".bazelignore",
            ".bazelversion",
        ] {
            let scratch = dx_test_scratch::scratch("dx-changed-workspace-").unwrap();
            let workspace = scratch.path().to_path_buf();
            write(&workspace, "pkg/BUILD.bazel", "").unwrap();
            write(&workspace, changed, "x\n").unwrap();
            let selection = resolve_affected(
                &[change(changed, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &NeverQuery,
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
    fn explicit_rc_files_widen_to_repository_without_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-explicit-rc-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "tools/ci.bazelrc", "x\n").unwrap();
        let selection = resolve_affected(
            &[change("tools/ci.bazelrc", dx_adopt::ChangeKind::Modified)],
            &workspace,
            &NeverQuery,
            &["--bazelrc=tools/ci.bazelrc".to_owned()],
            &[],
            None,
        )
        .expect("explicit rc widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "workspace input tools/ci.bazelrc"
        );
    }

    #[test]
    fn imported_rc_files_widen_to_repository_without_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-rc-imports-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(
            &workspace,
            ".bazelrc",
            "import %workspace%/.dx/bazelrc/correctness.bazelrc\ntry-import %workspace%/user.bazelrc\n",
        ).unwrap();
        write(&workspace, ".dx/bazelrc/BUILD.bazel", "").unwrap();
        write(
            &workspace,
            ".dx/bazelrc/correctness.bazelrc",
            "import \"%workspace%/.dx/bazelrc/nested.bazelrc\"\nimport %workspace%/.bazelrc\n",
        )
        .unwrap();
        write(&workspace, ".dx/bazelrc/nested.bazelrc", "build --x\n").unwrap();
        for changed in [
            ".dx/bazelrc/correctness.bazelrc",
            ".dx/bazelrc/nested.bazelrc",
        ] {
            let selection = resolve_affected(
                &[change(changed, dx_adopt::ChangeKind::Modified)],
                &workspace,
                &NeverQuery,
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

    fn rc_edit(prefix: &str, base_rc: &str, head_rc: &str) -> (AffectedSelection, usize) {
        let scratch = dx_test_scratch::scratch(prefix).unwrap();
        let workspace = scratch.path();
        write(workspace, "BUILD.bazel", "").unwrap();
        write(workspace, ".bazelrc", head_rc).unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(base_rc),
            graph(&[("//:rc_test", &["//:.bazelrc"])]),
        ]);
        let selection = resolve_affected(
            &[change(".bazelrc", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("rc edit resolves");
        (selection, query.calls().len())
    }

    #[test]
    fn rc_edits_to_inert_flags_select_only_rc_users() {
        let (selection, calls) = rc_edit(
            "dx-changed-rc-inert-",
            "build --config=strict\nbuild --jobs=8\n",
            "# tuned\nbuild --config=strict\nbuild --jobs 16 --disk_cache=~/.cache/d\n",
        );
        assert_eq!(selection.targets, strings(&["//:rc_test"]));
        assert_eq!(selection.widened, [] as [WidenedScope; 0]);
        assert_eq!(calls, 2);
    }

    #[test]
    fn rc_edits_to_build_flags_widen() {
        let (selection, calls) = rc_edit(
            "dx-changed-rc-build-",
            "build --config=strict\nbuild --jobs=8\n",
            "build --config=strict\nbuild --jobs=8 --copt=-O2\n",
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "workspace input .bazelrc");
        assert_eq!(calls, 1);
    }

    #[test]
    fn unimported_rc_files_do_not_widen() {
        let scratch = dx_test_scratch::scratch("dx-changed-rc-unimported-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, ".bazelrc", "build --x\n").unwrap();
        write(&workspace, "consumers/x/BUILD.bazel", "").unwrap();
        write(&workspace, "consumers/x/consumer.bazelrc", "build --y\n").unwrap();
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
        assert_eq!(selection.widened, [] as [WidenedScope; 0]);
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
            Some(base()),
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
    fn files_of_unresolved_extensions_widen_without_query() {
        let scratch = dx_test_scratch::scratch("dx-changed-module-inputs-").unwrap();
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            &[
                "module(name = \"demo\")\n".to_owned(),
                "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n".to_owned(),
                format!(
                    "npm.npm_translate_lock(name = \"npm\", pnpm_lock = \"//:{}\")\n",
                    Ecosystem::Npm.lockfile()
                ),
                "go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\n".to_owned(),
                format!(
                    "go_deps.from_file(go_mod = \"//third_party/go:{}\")\n",
                    manifest_of(Ecosystem::Go)
                ),
                "maven = use_extension(\"@rules_jvm_external//:extensions.bzl\", \"maven\")\n".to_owned(),
                format!(
                    "maven.install(lock_file = \"//third_party/jvm:{}\")\n",
                    Ecosystem::Maven.lockfile()
                ),
                "pip = use_extension(\"@rules_python//python/extensions:pip.bzl\", \"pip\")\n".to_owned(),
                "pip.parse(hub_name = \"pypi\", requirements_lock = \"//:requirements_lock.txt\")\n".to_owned(),
                "pip.parse(hub_name = \"pypi\", requirements_by_platform = {\"//py:requirements_linux.txt\": \"linux_*\"})\n".to_owned(),
                "include(\"//deps:go.MODULE.bazel\")\n".to_owned(),
            ].concat(),
        ).unwrap();
        write(
            workspace,
            "deps/go.MODULE.bazel",
            "bazel_dep(name = \"rules_go\", version = \"0.50.0\")\n",
        )
        .unwrap();
        write(workspace, "py/requirements_linux.txt", "x\n").unwrap();
        widens_as_workspace_input(workspace, "py/requirements_linux.txt");
        write(
            workspace,
            "deps/go.MODULE.bazel",
            "bazel_dep(name = \"rules_go\", version = \"0.51.0\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![FakeQuery::ok(
            "bazel_dep(name = \"rules_go\", version = \"0.50.0\")\n",
        )]);
        let selection = resolve_affected(
            &[change(
                "deps/go.MODULE.bazel",
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("included segment bump widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "workspace input MODULE.bazel.lock"
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn crate_annotation_patches_and_build_files_widen() {
        let scratch = crate_workspace("dx-changed-crate-annotation-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.annotation(crate = \"serde\", patches = [\"//third_party:serde.patch\"], additive_build_file = \"//third_party:serde.BUILD\")\n",
        ).unwrap();
        write(workspace, "third_party/serde.patch", "x\n").unwrap();
        write(workspace, "third_party/serde.BUILD", "x\n").unwrap();
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
            &format!(
                "module(name = \"demo\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(cargo_lockfile = \"//:{}\", manifests = [\"//a:{}\"])\n",
                Ecosystem::Cargo.lockfile(),
                manifest_of(Ecosystem::Cargo)
            ),
        ).unwrap();
        write(
            workspace,
            &format!("a/{}", manifest_of(Ecosystem::Cargo)),
            "[package]\n",
        )
        .unwrap();
        write(workspace, "b/package-lock.json", "{}\n").unwrap();
        for (changed, owner) in [
            (format!("a/{}", manifest_of(Ecosystem::Cargo)), "//a:lib"),
            ("b/package-lock.json".to_owned(), "//b:lib"),
        ] {
            let query = FakeQuery::new(vec![graph(&[
                (
                    "//a:lib",
                    &[&format!("//a:{}", manifest_of(Ecosystem::Cargo))],
                ),
                ("//b:lib", &["//b:package-lock.json"]),
            ])]);
            let selection = resolve_affected(
                &[change(&changed, dx_adopt::ChangeKind::Modified)],
                workspace,
                &query,
                &[],
                &[],
                Some(base()),
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
        let scratch = dx_test_scratch::scratch("dx-changed-fallback-inputs-").unwrap();
        let workspace = scratch.path();
        write(workspace, "web/BUILD.bazel", "").unwrap();
        write(
            workspace,
            &format!("web/{}", Ecosystem::Npm.lockfile()),
            "x\n",
        )
        .unwrap();
        widens_as_workspace_input(workspace, &format!("web/{}", Ecosystem::Npm.lockfile()));
        write(
            workspace,
            "MODULE.bazel",
            "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\nnpm.npm_translate_lock(pnpm_lock = LOCK)\n",
        ).unwrap();
        widens_as_workspace_input(workspace, &format!("web/{}", Ecosystem::Npm.lockfile()));
        write(workspace, "MODULE.bazel", "module(name = \n").unwrap();
        widens_as_workspace_input(workspace, &format!("web/{}", Ecosystem::Npm.lockfile()));
    }

    fn crate_module() -> String {
        format!(
            "bazel_dep(name = \"rules_rust\", version = \"1.0\")\ncrate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(\n    cargo_lockfile = \"//:{}\",\n    lockfile = \"//:cargo-bazel-lock.json\",\n)\n",
            Ecosystem::Cargo.lockfile()
        )
    }

    fn cargo_lock(serde: &str) -> String {
        format!(
            "version = 3\n\n[[package]]\nname = \"serde\"\nversion = \"{serde}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n\n[[package]]\nname = \"itoa\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n"
        )
    }

    #[test]
    fn included_segment_crate_hubs_count_as_lock_inputs() {
        let scratch = dx_test_scratch::scratch("dx-changed-segment-hubs-").unwrap();
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "include(\"//deps:rust.MODULE.bazel\")\n",
        )
        .unwrap();
        write(workspace, "deps/BUILD.bazel", "").unwrap();
        write(workspace, "deps/rust.MODULE.bazel", &crate_module()).unwrap();
        let (hubs, _) = module_inputs_of(workspace);
        assert_ne!(
            lock_inputs::classify("cargo-bazel-lock.json", &hubs),
            LockKind::Unknown
        );
    }

    #[test]
    fn root_extension_binding_does_not_reach_a_segment() {
        let scratch = dx_test_scratch::scratch("dx-changed-segment-binding-").unwrap();
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ninclude(\"//deps:rust.MODULE.bazel\")\n",
        ).unwrap();
        write(workspace, "deps/BUILD.bazel", "").unwrap();
        write(
            workspace,
            "deps/rust.MODULE.bazel",
            &format!(
                "crate.from_cargo(cargo_lockfile = \"//:{}\", lockfile = \"//:cargo-bazel-lock.json\")\n",
                Ecosystem::Cargo.lockfile()
            ),
        ).unwrap();
        let (hubs, _) = module_inputs_of(workspace);
        assert_eq!(
            lock_inputs::classify("cargo-bazel-lock.json", &hubs),
            LockKind::Unknown
        );
    }

    fn crate_workspace(prefix: &str) -> tempfile::TempDir {
        let scratch = dx_test_scratch::scratch(prefix).unwrap();
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", &crate_module()).unwrap();
        write(workspace, "a/BUILD.bazel", "").unwrap();
        write(workspace, "b/BUILD.bazel", "").unwrap();
        write(workspace, Ecosystem::Cargo.lockfile(), &cargo_lock("1.0.2")).unwrap();
        write_registry(workspace, &[RULES_RUST]);
        scratch
    }

    const RULES_RUST: (&str, &str) = ("rules_rust", "module(name = \"rules_rust\")\n");

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

    fn base() -> (&'static Path, &'static str) {
        (Path::new("git"), "base")
    }

    #[test]
    fn lock_update_widens_when_the_extension_module_has_no_registry_module_file() {
        let scratch = crate_workspace("dx-changed-unregistered-lock-");
        let workspace = scratch.path();
        write_registry(workspace, &[]);
        let query = FakeQuery::new(vec![FakeQuery::ok(&cargo_lock("1.0.1")), crate_graph()]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Cargo.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("lock widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "changed @@rules_rust++crate+* belongs to module rules_rust, which has no registry MODULE.bazel"
        );
    }

    #[test]
    fn one_crate_cargo_lock_update_selects_only_its_users() {
        let scratch = crate_workspace("dx-changed-cargo-lock-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![FakeQuery::ok(&cargo_lock("1.0.1")), crate_graph()]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Cargo.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["@@rules_rust++crate+crates//:serde"])
        );
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: format!("rdeps(//..., {})", hub_label("cargo", "//:serde")),
                reason: format!("changed crates in {}", Ecosystem::Cargo.lockfile()),
                repos: vec![hub_label("cargo", "//:serde")],
            }]
        );
        let graph = selection.graph.as_ref().expect("graph");
        assert_eq!(graph.affected(&selection.targets), ["//a:lib", "//a:t"]);
        let calls = query.calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0].0,
            strings(&[
                "git",
                "show",
                &format!("base:./{}", Ecosystem::Cargo.lockfile())
            ])
        );
    }

    struct Update {
        leaf: &'static str,
        leaf_extra: &'static str,
        user_extra: &'static str,
        other_extra: &'static str,
        root_extra: &'static str,
        lock_extra: serde_json::Value,
    }

    const UPDATE: Update = Update {
        leaf: "leaf",
        leaf_extra: "",
        user_extra: "",
        other_extra: "",
        root_extra: "",
        lock_extra: serde_json::Value::Null,
    };

    struct EditWorkspace {
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

    #[expect(
        clippy::needless_pass_by_value,
        reason = "callers build the update in the call"
    )]
    fn edit_workspace(prefix: &str, update: Update) -> EditWorkspace {
        let scratch = dx_test_scratch::scratch(prefix).unwrap();
        let workspace = scratch.path();
        let leaf = update.leaf;
        let module = |version: &str| {
            format!(
                "module(name = \"root\")\nbazel_dep(name = \"user\", version = \"1.0\")\nbazel_dep(name = \"{leaf}\", version = \"{version}\")\nbazel_dep(name = \"other\", version = \"1.0\")\n{}",
                update.root_extra
            )
        };
        write(workspace, "MODULE.bazel", &module("1.1")).unwrap();
        write(
            workspace,
            ".bazelrc",
            &format!("common --repository_cache={}/cache\n", workspace.display()),
        )
        .unwrap();
        write(
            workspace,
            "d/BUILD.bazel",
            &format!("load(\"@{leaf}//:defs.bzl\", \"x\")\n"),
        )
        .unwrap();
        write(workspace, "e/BUILD.bazel", "").unwrap();
        let leaf_old = cache_put(
            workspace,
            &format!("module(name = \"{leaf}\", version = \"1.0\")\n"),
        );
        let leaf_new = cache_put(
            workspace,
            &format!(
                "module(name = \"{leaf}\", version = \"1.1\")\n{}",
                update.leaf_extra
            ),
        );
        let user = cache_put(
            workspace,
            &format!(
                "module(name = \"user\")\nbazel_dep(name = \"{leaf}\", version = \"1.0\")\n{}",
                update.user_extra
            ),
        );
        let other = cache_put(
            workspace,
            &format!("module(name = \"other\")\n{}", update.other_extra),
        );
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
            &lock(hashes, &update.lock_extra),
        )
        .unwrap();
        EditWorkspace {
            scratch,
            base_module: module("1.0"),
            base_lock: lock(base_hashes, &serde_json::Value::Null),
        }
    }

    fn update_graph() -> QueryResult {
        graph_with_kinds(&[
            ("//d:lib", "cc_library", &["@@leaf+//:lib"]),
            ("//u:lib", "cc_library", &["@@user+//:lib"]),
            ("//e:lib", "cc_library", &["@@other+//:lib"]),
        ])
    }

    fn resolve_edit(fixture: &EditWorkspace, query: &FakeQuery) -> AffectedSelection {
        resolve_affected(
            &[
                change("MODULE.bazel", dx_adopt::ChangeKind::Modified),
                change("MODULE.bazel.lock", dx_adopt::ChangeKind::Modified),
            ],
            fixture.scratch.path(),
            query,
            &[],
            &[],
            Some(base()),
        )
        .expect("update resolves")
    }

    fn widened_reason(fixture: &EditWorkspace, query: &FakeQuery) -> String {
        let selection = resolve_edit(fixture, query);
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened.len(), 1, "{:?}", selection.widened);
        selection.widened[0].reason.clone()
    }

    fn base_reads(fixture: &EditWorkspace) -> FakeQuery {
        FakeQuery::new(vec![
            FakeQuery::ok(&fixture.base_module),
            FakeQuery::ok(&fixture.base_lock),
        ])
    }

    #[test]
    fn version_only_bazel_dep_update_selects_its_users() {
        let fixture = edit_workspace("dx-changed-update-", UPDATE);
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&fixture.base_module),
            FakeQuery::ok(&fixture.base_lock),
            update_graph(),
        ]);
        let selection = resolve_edit(&fixture, &query);
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
    fn update_of_toolchain_registering_module_widens() {
        for (update, module) in [
            (
                Update {
                    leaf_extra: "register_toolchains(\"//:t\")\n",
                    ..UPDATE
                },
                "leaf",
            ),
            (
                Update {
                    user_extra: "register_execution_platforms(\"//:p\")\n",
                    ..UPDATE
                },
                "user",
            ),
        ] {
            let fixture = edit_workspace("dx-changed-update-tc-", update);
            let query = base_reads(&fixture);
            assert_eq!(
                widened_reason(&fixture, &query),
                format!("changed module {module} registers toolchains")
            );
            assert_eq!(query.calls().len(), 2);
        }
    }

    #[test]
    fn update_of_bazel_tools_dep_widens() {
        let fixture = edit_workspace(
            "dx-changed-update-cc-",
            Update {
                leaf: "rules_cc",
                ..UPDATE
            },
        );
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "changed module bazel_tools registers toolchains"
        );
    }

    #[test]
    fn module_call_edit_widens() {
        let fixture = edit_workspace("dx-changed-update-edit-", UPDATE);
        let edited = fixture.base_module.replace("\"root\"", "\"renamed\"");
        let query = FakeQuery::new(vec![FakeQuery::ok(&edited)]);
        assert_eq!(
            widened_reason(&fixture, &query),
            "MODULE.bazel changes module"
        );
        assert_eq!(query.calls().len(), 1);
    }

    #[test]
    fn bump_next_to_an_unchanged_patched_override_narrows() {
        let fixture = edit_workspace(
            "dx-changed-update-patched-",
            Update {
                root_extra: "single_version_override(module_name = \"other\", patches = [\"//p:o.patch\"])\n",
                ..UPDATE
            },
        );
        write(
            fixture.scratch.path(),
            "p/o.patch",
            "--- a/MODULE.bazel\n+++ b/MODULE.bazel\n-module(name = \"other\")\n+module(name = \"other\", compatibility_level = 1)\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&fixture.base_module),
            FakeQuery::ok(&fixture.base_lock),
            update_graph(),
        ]);
        let selection = resolve_edit(&fixture, &query);
        assert_eq!(
            selection.targets,
            strings(&["//d:all", "@@leaf+//:lib", "@@user+//:lib"])
        );
    }

    #[test]
    fn module_update_without_lock_update_widens() {
        let fixture = edit_workspace("dx-changed-update-stale-", UPDATE);
        write(
            fixture.scratch.path(),
            "MODULE.bazel.lock",
            &fixture.base_lock,
        )
        .unwrap();
        let query = FakeQuery::new(vec![FakeQuery::ok(&fixture.base_module)]);
        let selection = resolve_affected(
            &[change("MODULE.bazel", dx_adopt::ChangeKind::Modified)],
            fixture.scratch.path(),
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("update resolves");
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
        let fixture = edit_workspace("dx-changed-update-miss-", UPDATE);
        std::fs::remove_dir_all(fixture.scratch.path().join("cache")).expect("drop cache");
        let query = base_reads(&fixture);
        assert!(widened_reason(&fixture, &query).starts_with("no cached MODULE.bazel for "));
    }

    #[test]
    fn cache_file_with_wrong_hash_is_ignored() {
        let fixture = edit_workspace("dx-changed-update-hash-", UPDATE);
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
    fn lock_extension_change_selects_its_repos() {
        let extra = serde_json::json!({"moduleExtensions": {"@@other+//:e.bzl%e": {"x": 1}}});
        let fixture = edit_workspace(
            "dx-changed-update-ext-",
            Update {
                lock_extra: extra,
                ..UPDATE
            },
        );
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&fixture.base_module),
            FakeQuery::ok(&fixture.base_lock),
            graph_with_kinds(&[
                ("//d:lib", "cc_library", &["@@leaf+//:lib"]),
                ("//e:lib", "cc_library", &["@@other+//:lib"]),
                ("//h:lib", "cc_library", &["@@other++e+hub//:lib"]),
            ]),
        ]);
        let selection = resolve_edit(&fixture, &query);
        assert_eq!(
            selection.targets,
            strings(&["//d:all", "@@leaf+//:lib", "@@other++e+hub//:lib"])
        );
        assert_eq!(
            selection.widened.last().map(|entry| entry.repos.clone()),
            Some(strings(&["@leaf", "@user", "@@other++e+*"]))
        );
    }

    #[test]
    fn lock_extension_change_widens_when_its_module_registers_its_repos() {
        let extra = serde_json::json!({"moduleExtensions": {"@@other+//:e.bzl%e": {"x": 1}}});
        let fixture = edit_workspace(
            "dx-changed-update-ext-tc-",
            Update {
                other_extra: "e = use_extension(\"//:e.bzl\", \"e\")\nuse_repo(e, \"hub\")\nregister_toolchains(\"@hub//:all\")\n",
                lock_extra: extra,
                ..UPDATE
            },
        );
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "module other registers toolchains from @@other++e+*"
        );
    }

    #[test]
    fn local_path_override_widens() {
        let fixture = edit_workspace(
            "dx-changed-update-local-",
            Update {
                root_extra: "local_path_override(module_name = \"other\", path = \"o\")\n",
                ..UPDATE
            },
        );
        let query = base_reads(&fixture);
        assert_eq!(
            widened_reason(&fixture, &query),
            "MODULE.bazel local_path_override for other hides its MODULE.bazel"
        );
    }

    const GO_MODULE: &str = "module(name = \"root\")\n\
bazel_dep(name = \"gazelle\", version = \"0.54.0\")\n\
go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\n\
go_deps.from_file(go_mod = \"//:go.mod\")\n\
use_repo(go_deps, \"com_github_x_y\")\n\
tools = use_extension(\"//tools:extensions.bzl\", \"tools\")\n\
tools.pin(name = \"lint\", version = \"1\")\n\
use_repo(tools, \"lint\")\n\
register_toolchains(\"//tc:all\")\n";

    fn go_graph() -> QueryResult {
        graph_with_kinds(&[
            (
                "//tools/ci:actionlint_test",
                "sh_test",
                &["@@gazelle++go_deps+com_github_rhysd_actionlint//cmd/actionlint:actionlint"],
            ),
            (
                "//app:lib",
                "go_library",
                &["@@gazelle++go_deps+com_github_x_y//:y"],
            ),
            ("//lint:check", "sh_test", &["@@+tools+lint//:bin"]),
            ("//other:lib", "cc_library", &["@@rules_cc+//:x"]),
        ])
    }

    const GAZELLE: (&str, &str) = ("gazelle", "module(name = \"gazelle\")\n");

    fn module_edit(prefix: &str, head: &str) -> (AffectedSelection, usize) {
        registry_edit(prefix, GO_MODULE, head, &[GAZELLE])
    }

    fn write_registry(workspace: &Path, registry: &[(&str, &str)]) {
        write(
            workspace,
            ".bazelrc",
            &format!("common --repository_cache={}/cache\n", workspace.display()),
        )
        .unwrap();
        let mut hashes = serde_json::Map::new();
        for (name, text) in registry {
            hashes.insert(
                format!("https://bcr.bazel.build/modules/{name}/1.0/MODULE.bazel"),
                cache_put(workspace, text).into(),
            );
        }
        write(
            workspace,
            "MODULE.bazel.lock",
            &serde_json::json!({"lockFileVersion": 24, "registryFileHashes": hashes}).to_string(),
        )
        .unwrap();
    }

    fn registry_edit(
        prefix: &str,
        before: &str,
        head: &str,
        registry: &[(&str, &str)],
    ) -> (AffectedSelection, usize) {
        let scratch = dx_test_scratch::scratch(prefix).unwrap();
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", head).unwrap();
        write(
            workspace,
            "go.mod",
            "module m\n\nrequire github.com/x/y v1.0.0\n",
        )
        .unwrap();
        write_registry(workspace, registry);
        let query = FakeQuery::new(vec![FakeQuery::ok(before), go_graph()]);
        let selection = resolve_affected(
            &[change("MODULE.bazel", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("module edit resolves");
        (selection, query.calls().len())
    }

    const EVERY_GO_DEPS_USER: [&str; 2] = [
        "@@gazelle++go_deps+com_github_rhysd_actionlint//cmd/actionlint:actionlint",
        "@@gazelle++go_deps+com_github_x_y//:y",
    ];

    #[test]
    fn dev_go_deps_usage_selects_every_go_deps_user() {
        let head = format!(
            "{GO_MODULE}dev = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\", dev_dependency = True)\n\
dev.module(path = \"github.com/rhysd/actionlint\", sum = \"h1:a\", version = \"v1.7.13\")\n\
use_repo(dev, \"com_github_rhysd_actionlint\")\n"
        );
        let (selection, _) = module_edit("dx-changed-dev-go-deps-", &head);
        assert_eq!(selection.targets, strings(&EVERY_GO_DEPS_USER));
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: "rdeps(//..., @@gazelle++go_deps+*, @@gazelle++go_deps+com_github_rhysd_actionlint)".to_owned(),
                reason: "changed modules in MODULE.bazel".to_owned(),
                repos: strings(&[
                    "@@gazelle++go_deps+*",
                    "@@gazelle++go_deps+com_github_rhysd_actionlint"
                ]),
            }]
        );
    }

    const RULES_GO: (&str, &str) = (
        "rules_go",
        "module(name = \"rules_go\")\n\
bazel_dep(name = \"gazelle\", version = \"1.0\")\n\
go_sdk = use_extension(\"//go:extensions.bzl\", \"go_sdk\")\n\
use_repo(go_sdk, \"go_toolchains\")\n\
register_toolchains(\"@go_toolchains//:all\")\n\
go_deps = use_extension(\"@gazelle//:extensions.bzl\", \"go_deps\")\n\
use_repo(go_deps, \"org_golang_google_protobuf\")\n",
    );

    #[test]
    fn go_sdk_edit_widens_because_rules_go_registers_its_toolchains() {
        let base = format!(
            "{GO_MODULE}bazel_dep(name = \"rules_go\", version = \"1.0\")\n\
go_sdk = use_extension(\"@rules_go//go:extensions.bzl\", \"go_sdk\")\n\
go_sdk.download(version = \"1.25.0\")\n"
        );
        let head = base.replace("1.25.0", "1.26.0");
        let (selection, _) =
            registry_edit("dx-changed-go-sdk-", &base, &head, &[GAZELLE, RULES_GO]);
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "module rules_go registers toolchains from @@rules_go++go_sdk+*"
        );
        let base = format!("{GO_MODULE}bazel_dep(name = \"rules_go\", version = \"1.0\")\n");
        let head = format!(
            "{base}go_deps.module(path = \"github.com/x/y\", sum = \"h1:a\", version = \"v1.1.0\")\n"
        );
        let (selection, _) = registry_edit(
            "dx-changed-go-deps-rules-go-",
            &base,
            &head,
            &[GAZELLE, RULES_GO],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "module rules_go also uses @@gazelle++go_deps+*"
        );
    }

    #[test]
    fn repo_passed_to_another_tag_widens() {
        let base = format!(
            "{GO_MODULE}bazel_dep(name = \"rules_rust\", version = \"1.0\")\n\
http_file = use_repo_rule(\"@bazel_tools//tools/build_defs/repo:http.bzl\", \"http_file\")\n\
http_file(name = \"protoc_bin\", sha256 = \"a\", urls = [\"https://example.com/protoc\"])\n\
http_file(name = \"plain_bin\", sha256 = \"a\", urls = [\"https://example.com/plain\"])\n\
crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\n\
crate.annotation(crate = \"prost-build\", build_script_data = [\"@protoc_bin//file\"])\n"
        );
        let head = base.replace(
            "name = \"protoc_bin\", sha256 = \"a\"",
            "name = \"protoc_bin\", sha256 = \"b\"",
        );
        let (selection, _) = registry_edit(
            "dx-changed-passed-repo-",
            &base,
            &head,
            &[GAZELLE, RULES_RUST],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "MODULE.bazel call names @protoc_bin"
        );
        let through_variable = |text: &str| {
            text.replace(
                "build_script_data = [\"@protoc_bin//file\"]",
                "build_script_data = [PROTOC]",
            )
            .replace(
                "crate.annotation(",
                "PROTOC = \"@protoc_bin//file\"\ncrate.annotation(",
            )
        };
        let (selection, _) = registry_edit(
            "dx-changed-passed-var-",
            &through_variable(&base),
            &through_variable(&head),
            &[GAZELLE, RULES_RUST],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "MODULE.bazel call names @protoc_bin"
        );
        let head = base.replace(
            "name = \"plain_bin\", sha256 = \"a\"",
            "name = \"plain_bin\", sha256 = \"b\"",
        );
        let (selection, _) = registry_edit(
            "dx-changed-plain-repo-",
            &base,
            &head,
            &[GAZELLE, RULES_RUST],
        );
        assert_eq!(selection.targets, Vec::<String>::new());
    }

    #[test]
    fn python_toolchain_edit_widens_and_pip_edit_narrows() {
        let rules_python = (
            "rules_python",
            "module(name = \"rules_python\")\n\
python = use_extension(\"//python/extensions:python.bzl\", \"python\")\n\
use_repo(python, \"pythons_hub\")\n\
register_toolchains(\"@pythons_hub//:all\")\n\
pyrefly = use_extension(\"//python/private:pyrefly.bzl\", \"pyrefly\", dev_dependency = True)\n\
use_repo(pyrefly, \"pyrefly_toolchains\")\n\
register_toolchains(\"@pyrefly_toolchains//:all\", dev_dependency = True)\n",
        );
        let base = format!(
            "{GO_MODULE}bazel_dep(name = \"rules_python\", version = \"1.0\")\n\
python = use_extension(\"@rules_python//python/extensions:python.bzl\", \"python\")\n\
python.toolchain(python_version = \"3.13\")\n\
pip = use_extension(\"@rules_python//python/extensions:pip.bzl\", \"pip\")\n\
pip.parse(hub_name = \"pypi\", python_version = \"3.13\")\n\
use_repo(pip, \"pypi\")\n"
        );
        let head = base.replace(
            "python.toolchain(python_version = \"3.13\")",
            "python.toolchain(python_version = \"3.14\")",
        );
        let (selection, _) =
            registry_edit("dx-changed-python-", &base, &head, &[GAZELLE, rules_python]);
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "module rules_python registers toolchains from @@rules_python++python+*"
        );
        let head = base.replace(
            "pip.parse(hub_name = \"pypi\", python_version = \"3.13\")",
            "pip.parse(hub_name = \"pypi\", python_version = \"3.14\")",
        );
        let (selection, _) =
            registry_edit("dx-changed-pip-", &base, &head, &[GAZELLE, rules_python]);
        assert!(selection.targets.is_empty(), "{:?}", selection.targets);
        assert_eq!(
            selection.widened.last().map(|entry| entry.scope.clone()),
            Some("rdeps(//..., @@rules_python++pip+*)".to_owned())
        );
    }

    #[test]
    fn extension_of_a_module_outside_the_registry_widens() {
        let (selection, _) = registry_edit(
            "dx-changed-go-unregistered-",
            GO_MODULE,
            &format!(
                "{GO_MODULE}go_deps.module(path = \"github.com/x/y\", sum = \"h1:a\", version = \"v1.1.0\")\n"
            ),
            &[],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "changed @@gazelle++go_deps+* belongs to module gazelle, which has no registry MODULE.bazel"
        );
    }

    #[test]
    fn go_deps_tags_select_every_go_deps_user() {
        for tag in [
            "go_deps.module(path = \"github.com/x/y\", sum = \"h1:a\", version = \"v1.1.0\")\n",
            "go_deps.module(path = \"github.com/fatih/color\", indirect = True, sum = \"h1:b\", version = \"v1.19.0\")\n",
            "go_deps.gazelle_default_attributes(directives = [\"gazelle:proto disable\"])\n",
        ] {
            let (selection, _) = module_edit("dx-changed-go-tag-", &format!("{GO_MODULE}{tag}"));
            assert_eq!(selection.targets, strings(&EVERY_GO_DEPS_USER), "{tag}");
        }
    }

    #[test]
    fn go_sum_update_selects_every_go_deps_user() {
        let scratch = dx_test_scratch::scratch("dx-changed-go-sum-").unwrap();
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", GO_MODULE).unwrap();
        write_registry(workspace, &[GAZELLE]);
        write(
            workspace,
            "go.mod",
            "module m\n\nrequire github.com/x/y v1.0.0\n",
        )
        .unwrap();
        let sum = "github.com/x/y v1.0.0 h1:a\n";
        write(
            workspace,
            Ecosystem::Go.lockfile(),
            &sum.replace("h1:a", "h1:b"),
        )
        .unwrap();
        let query = FakeQuery::new(vec![FakeQuery::ok(sum), go_graph()]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Go.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("go.sum narrows");
        assert_eq!(selection.targets, strings(&EVERY_GO_DEPS_USER));
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: "rdeps(//..., @@*+go_deps+*)".to_owned(),
                reason: format!("changed Go modules in {}", Ecosystem::Go.lockfile()),
                repos: strings(&["@@*+go_deps+*"]),
            }]
        );
    }

    #[test]
    fn go_sum_update_widens_when_another_module_uses_go_deps() {
        let scratch = dx_test_scratch::scratch("dx-changed-go-sum-shared-").unwrap();
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            &format!("{GO_MODULE}bazel_dep(name = \"rules_go\", version = \"1.0\")\n"),
        )
        .unwrap();
        write_registry(workspace, &[GAZELLE, RULES_GO]);
        write(
            workspace,
            "go.mod",
            "module m\n\nrequire google.golang.org/protobuf v1.36.0\n",
        )
        .unwrap();
        let sum = "google.golang.org/protobuf v1.36.0 h1:a\n";
        write(
            workspace,
            Ecosystem::Go.lockfile(),
            &sum.replace("v1.36.0 h1:a", "v1.36.1 h1:b"),
        )
        .unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(sum),
            graph_with_kinds(&[(
                "//proto:foo_go_proto",
                "go_proto_library",
                &["@@rules_go+//proto:go_proto"],
            )]),
        ]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Go.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("go.sum widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "module rules_go also uses @@gazelle++go_deps+*"
        );
    }

    #[test]
    fn root_extension_tag_edit_selects_that_extensions_users() {
        let head = GO_MODULE.replace("version = \"1\")", "version = \"2\")");
        let (selection, _) = module_edit("dx-changed-root-ext-", &head);
        assert_eq!(selection.targets, strings(&["@@+tools+lint//:bin"]));
    }

    #[test]
    fn register_toolchains_edit_still_widens() {
        let head = GO_MODULE.replace("//tc:all", "//tc:other");
        let (selection, calls) = module_edit("dx-changed-register-", &head);
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "MODULE.bazel changes register_toolchains"
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn files_named_by_known_extension_tags_select_its_users() {
        let scratch = dx_test_scratch::scratch("dx-changed-tag-file-").unwrap();
        let workspace = scratch.path();
        let module = GO_MODULE.replace(
            "tools.pin(name = \"lint\", version = \"1\")",
            "tools.pin(name = \"lint\", config = \"//tools:lint.json\")",
        );
        write(workspace, "MODULE.bazel", &module).unwrap();
        write(workspace, "tools/lint.json", "{}\n").unwrap();
        let query = FakeQuery::new(vec![go_graph()]);
        let selection = resolve_affected(
            &[change("tools/lint.json", dx_adopt::ChangeKind::Modified)],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("tag file narrows");
        assert_eq!(selection.targets, strings(&["@@+tools+lint//:bin"]));
    }

    #[test]
    fn included_segment_edit_diffs_like_the_root_module() {
        let scratch = dx_test_scratch::scratch("dx-changed-include-").unwrap();
        let workspace = scratch.path();
        let segment = "tools = use_extension(\"//tools:extensions.bzl\", \"tools\")\ntools.pin(name = \"lint\", version = \"1\")\nuse_repo(tools, \"lint\")\n";
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"root\")\ninclude(\"//deps:tools.MODULE.bazel\")\n",
        )
        .unwrap();
        write(
            workspace,
            "deps/tools.MODULE.bazel",
            &segment.replace("\"1\"", "\"2\""),
        )
        .unwrap();
        let query = FakeQuery::new(vec![FakeQuery::ok(segment), go_graph()]);
        let selection = resolve_affected(
            &[change(
                "deps/tools.MODULE.bazel",
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("segment narrows");
        assert_eq!(selection.targets, strings(&["@@+tools+lint//:bin"]));
        assert_eq!(
            query.calls()[0].0,
            strings(&["git", "show", "base:./deps/tools.MODULE.bazel"])
        );
    }

    #[test]
    fn nested_module_bazel_still_widens() {
        let scratch = dx_test_scratch::scratch("dx-changed-nested-module-").unwrap();
        let workspace = scratch.path();
        write(workspace, "sub/MODULE.bazel", "module(name = \"sub\")\n").unwrap();
        let selection = resolve_affected(
            &[change("sub/MODULE.bazel", dx_adopt::ChangeKind::Modified)],
            workspace,
            &NeverQuery,
            &[],
            &[],
            Some(base()),
        )
        .expect("nested module widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            "workspace input sub/MODULE.bazel"
        );
    }

    #[test]
    fn lock_update_behind_a_toolchain_widens_to_everything() {
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
            &[change(
                Ecosystem::Cargo.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("lock widens");
        assert_eq!(
            selection.widened,
            [
                WidenedScope {
                    scope: "//...".to_owned(),
                    reason: format!(
                        "changed crates in {} reach toolchain //tc:prost",
                        Ecosystem::Cargo.lockfile()
                    ),
                    repos: Vec::new(),
                },
                WidenedScope {
                    scope: format!("rdeps(//..., {})", hub_label("cargo", "//:serde")),
                    reason: format!("changed crates in {}", Ecosystem::Cargo.lockfile()),
                    repos: vec![hub_label("cargo", "//:serde")],
                },
            ]
        );
    }

    #[test]
    fn pnpm_lock_update_selects_only_users_of_changed_packages() {
        let scratch = dx_test_scratch::scratch("dx-changed-pnpm-lock-").unwrap();
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            &[
                "bazel_dep(name = \"aspect_rules_js\", version = \"1.0\")\n".to_owned(),
                "npm = use_extension(\"@aspect_rules_js//npm:extensions.bzl\", \"npm\")\n"
                    .to_owned(),
                format!(
                    "npm.npm_translate_lock(name = \"npm\", pnpm_lock = \"//:{}\")\n",
                    Ecosystem::Npm.lockfile()
                ),
            ]
            .concat(),
        )
        .unwrap();
        write_registry(
            workspace,
            &[("aspect_rules_js", "module(name = \"aspect_rules_js\")\n")],
        );
        write(workspace, "BUILD.bazel", "").unwrap();
        let lock = |lodash: &str| {
            format!(
                "lockfileVersion: '9.0'\n\npackages:\n\n  left-pad@1.0.0:\n    resolution: {{integrity: sha512-a}}\n\n  lodash@{lodash}:\n    resolution: {{integrity: sha512-b}}\n"
            )
        };
        write(workspace, Ecosystem::Npm.lockfile(), &lock("4.1.0")).unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&lock("4.0.0")),
            graph(&[
                ("//a:lib", &["@@aspect_rules_js++npm+npm//lodash:pkg"]),
                ("//b:lib", &["@@aspect_rules_js++npm+npm//left-pad:pkg"]),
            ]),
        ]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Npm.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("pnpm lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["@@aspect_rules_js++npm+npm//lodash:pkg"])
        );
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: format!("rdeps(//..., {})", hub_label("npm", "//lodash")),
                reason: format!("changed crates in {}", Ecosystem::Npm.lockfile()),
                repos: vec![hub_label("npm", "//lodash")],
            }]
        );
    }

    #[test]
    fn lock_update_nothing_uses_selects_nothing_but_names_its_crates() {
        let scratch = crate_workspace("dx-changed-unused-lock-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&cargo_lock("1.0.1")),
            graph(&[("//b:lib", &["@@rules_rust++crate+crates//:itoa"])]),
        ]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Cargo.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
        )
        .expect("lock narrows");
        assert!(selection.targets.is_empty(), "{:?}", selection.targets);
        assert_eq!(
            selection.widened,
            [WidenedScope {
                scope: format!("rdeps(//..., {})", hub_label("cargo", "//:serde")),
                reason: format!("changed crates in {}", Ecosystem::Cargo.lockfile()),
                repos: vec![hub_label("cargo", "//:serde")],
            }]
        );
    }

    #[test]
    fn cargo_bazel_lock_json_update_selects_only_its_users() {
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
        write(workspace, "cargo-bazel-lock.json", &lock("1.0.2")).unwrap();
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
            Some(base()),
        )
        .expect("lock narrows");
        assert_eq!(
            selection.targets,
            strings(&["@@rules_rust++crate+crates//:serde"])
        );
        assert_eq!(
            selection.widened[0].repos,
            vec![hub_label("cargo", "//:serde")]
        );
    }

    #[test]
    fn unknown_lock_format_still_widens() {
        let scratch = crate_workspace("dx-changed-unknown-lock-");
        let workspace = scratch.path();
        write(workspace, "x.lock", "x\n").unwrap();
        let selection = resolve_affected(
            &[change("x.lock", dx_adopt::ChangeKind::Modified)],
            workspace,
            &NeverQuery,
            &[],
            &[],
            Some(base()),
        )
        .expect("unknown lock widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(selection.widened[0].reason, "workspace input x.lock");
        assert_eq!(selection.widened[0].repos, [] as [String; 0]);
    }

    #[test]
    fn added_deleted_or_unparsable_cargo_lock_widens() {
        let scratch = crate_workspace("dx-changed-added-lock-");
        let workspace = scratch.path();
        let modified = [change(
            Ecosystem::Cargo.lockfile(),
            dx_adopt::ChangeKind::Modified,
        )];
        let added = FakeQuery::new(vec![FakeQuery::failed(&format!(
            "fatal: path '{}' exists on disk, but not in 'base'\n",
            Ecosystem::Cargo.lockfile()
        ))]);
        let selection = resolve_affected(&modified, workspace, &added, &[], &[], Some(base()))
            .expect("added widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            format!("workspace input {}", Ecosystem::Cargo.lockfile())
        );

        let unparsable = FakeQuery::new(vec![FakeQuery::ok("not toml [[[")]);
        let selection = resolve_affected(&modified, workspace, &unparsable, &[], &[], Some(base()))
            .expect("unparsable widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert!(
            selection.widened[0].reason.starts_with(&format!(
                "cannot read {}: invalid lock",
                Ecosystem::Cargo.lockfile()
            )),
            "{:?}",
            selection.widened
        );

        let broken = FakeQuery::new(vec![FakeQuery::failed("fatal: bad object base\n")]);
        let selection = resolve_affected(&modified, workspace, &broken, &[], &[], Some(base()))
            .expect("git widens");
        assert_eq!(
            selection.widened[0].reason,
            format!(
                "cannot read {}: git show failed: fatal: bad object base (exit 2)",
                Ecosystem::Cargo.lockfile()
            )
        );

        let selection = resolve_affected(&modified, workspace, &NeverQuery, &[], &[], None)
            .expect("no base widens");
        assert_eq!(
            selection.widened[0].reason,
            format!("workspace input {}", Ecosystem::Cargo.lockfile())
        );

        std::fs::remove_file(workspace.join(Ecosystem::Cargo.lockfile())).expect("remove");
        let selection = resolve_affected(
            &[change(
                Ecosystem::Cargo.lockfile(),
                dx_adopt::ChangeKind::Deleted,
            )],
            workspace,
            &NeverQuery,
            &[],
            &[],
            Some(base()),
        )
        .expect("deleted widens");
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened[0].reason,
            format!("workspace input {}", Ecosystem::Cargo.lockfile())
        );
    }

    #[test]
    fn lock_that_is_a_rule_input_keeps_its_owner() {
        let scratch = crate_workspace("dx-changed-owned-lock-");
        let workspace = scratch.path();
        write(workspace, "BUILD.bazel", "").unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::ok(&cargo_lock("1.0.1")),
            graph(&[
                (
                    "//:vendor",
                    &[&format!("//:{}", Ecosystem::Cargo.lockfile())],
                ),
                ("//a:lib", &["@@rules_rust++crate+crates//:serde"]),
                ("//b:lib", &["//b:b.rs"]),
            ]),
        ]);
        let selection = resolve_affected(
            &[change(
                Ecosystem::Cargo.lockfile(),
                dx_adopt::ChangeKind::Modified,
            )],
            workspace,
            &query,
            &[],
            &[],
            Some(base()),
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
            let scratch = dx_test_scratch::scratch("dx-changed-parity-").unwrap();
            let workspace = scratch.path().to_path_buf();
            write(&workspace, "pkg/BUILD.bazel", "").unwrap();
            write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
            write(&workspace, "pkg/b.py", "x = 1\n").unwrap();
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
            assert_eq!(affected.widened, [] as [WidenedScope; 0]);
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
        let scratch = dx_test_scratch::scratch("dx-changed-deleted-parity-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-control-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/ok.py", "x = 1\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-escape-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-no-package-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "docs/guide.md", "# guide\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-nested-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "outer/BUILD.bazel", "").unwrap();
        write(&workspace, "outer/inner/BUILD.bazel", "").unwrap();
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
        let scratch = dx_test_scratch::scratch(prefix).unwrap();
        let workspace = scratch.path();
        write(workspace, "MODULE.bazel", "module(name = \"demo\")\n").unwrap();
        write(workspace, "tools/BUILD.bazel", "").unwrap();
        write(workspace, "tools/defs.bzl", "x = 1\n").unwrap();
        write(
            workspace,
            "app/BUILD.bazel",
            "load(\"//tools:defs.bzl\", \"x\")\n",
        )
        .unwrap();
        write(workspace, "app/main.py", "x = 1\n").unwrap();
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
        write(workspace, "tools/x.scl", "X = 1\n").unwrap();
        write(workspace, "tools/defs.bzl", "load(\":x.scl\", \"X\")\n").unwrap();
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
        )
        .unwrap();
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
        write(workspace, "app/BUILD.bazel", "").unwrap();
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert!(selection.targets.is_empty(), "{:?}", selection.targets);
        assert_eq!(selection.widened, [] as [WidenedScope; 0]);
    }

    #[test]
    fn bzl_owned_by_a_rule_keeps_its_owner() {
        let scratch = bzl_workspace("dx-changed-bzl-owned-");
        let workspace = scratch.path();
        write(workspace, "app/BUILD.bazel", "").unwrap();
        let query = FakeQuery::new(vec![graph(&[("//tools:defs", &["//tools:defs.bzl"])])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(selection.targets, strings(&["//tools:defs"]));
        assert_eq!(selection.widened, [] as [WidenedScope; 0]);
    }

    #[test]
    fn bzl_used_by_module_extension_selects_its_users() {
        let scratch = bzl_workspace("dx-changed-bzl-extension-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:defs.bzl\", \"x\")\nuse_repo(x, \"xr\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![graph(&[
            ("//app:a", &["@@+x+xr//:lib"]),
            ("//app:b", &["@@+x2+other//:lib", "@@rules_cc+//:cc"]),
            ("//app:c", &["@@+xy+other//:lib"]),
        ])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &[],
        );
        assert_eq!(
            selection.targets,
            strings(&["//app:all", "@@+x+xr//:lib", "@@+x2+other//:lib"])
        );
        assert_eq!(
            selection.widened,
            [
                WidenedScope {
                    scope: "//app:all".to_owned(),
                    reason: "loads tools/defs.bzl".to_owned(),
                    repos: Vec::new(),
                },
                WidenedScope {
                    scope: "rdeps(//..., @@+x+*)".to_owned(),
                    reason: "changed module extension x in tools/defs.bzl".to_owned(),
                    repos: strings(&["@@+x+*"]),
                }
            ]
        );
    }

    #[test]
    fn extension_helper_bzl_widens_when_its_repos_register_toolchains() {
        let scratch = bzl_workspace("dx-changed-bzl-helper-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\nx = use_extension(\"//tools:ext.bzl\", \"x\")\nuse_repo(x, \"xr\")\nregister_toolchains(\"@xr//:all\")\n",
        )
        .unwrap();
        write(workspace, "tools/ext.bzl", "load(\":defs.bzl\", \"x\")\n").unwrap();
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
            "core/quality/aspects.bzl",
            "load(\"//tools:defs.bzl\", \"x\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![graph(&[])]);
        let selection = bzl_resolve(
            workspace,
            &[change("tools/defs.bzl", dx_adopt::ChangeKind::Modified)],
            &query,
            &["@demo//core/quality:aspects.bzl%lint_aspect"],
        );
        assert_eq!(selection.targets, strings(&["//..."]));
        assert_eq!(
            selection.widened,
            widened(&[("//...", "aspect input tools/defs.bzl")])
        );
    }

    fn implicit_workspace(prefix: &str) -> tempfile::TempDir {
        let scratch = bzl_workspace(prefix);
        let workspace = scratch.path();
        write(
            workspace,
            "core/quality/aspects.bzl",
            "lint_aspect = aspect(attrs = {\"_runner\": attr.label(default = \"@demo//runner\")})\n",
        )
        .unwrap();
        write(workspace, "runner/BUILD.bazel", "").unwrap();
        write(workspace, "runner/main.rs", "fn main() {}\n").unwrap();
        write(workspace, "cfg/BUILD.bazel", "").unwrap();
        write(workspace, "cfg/typos.toml", "\n").unwrap();
        write(
            workspace,
            ".bazelrc",
            "common --//cfg:workspace=//cfg:policy\n",
        )
        .unwrap();
        scratch
    }

    fn implicit_graph() -> QueryResult {
        graph(&[
            ("//runner:runner", &["//runner:main.rs"]),
            ("//cfg:typos", &["//cfg:typos.toml"]),
            ("//cfg:policy", &["//cfg:typos"]),
            ("//app:lib", &["//app:main.py"]),
        ])
    }

    #[test]
    fn changes_that_reach_an_aspect_attribute_default_widen() {
        let scratch = implicit_workspace("dx-changed-implicit-aspect-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![implicit_graph()]);
        let selection = bzl_resolve(
            workspace,
            &[change("runner/main.rs", dx_adopt::ChangeKind::Modified)],
            &query,
            &["@demo//core/quality:aspects.bzl%lint_aspect"],
        );
        assert!(selection.targets.contains(&"//...".to_owned()));
        assert_eq!(
            selection.widened,
            widened(&[(
                "//...",
                "changed files reach //runner:runner, which aspects or bazelrc flags use"
            )])
        );
    }

    #[test]
    fn changes_that_reach_a_bazelrc_flag_value_widen() {
        let scratch = implicit_workspace("dx-changed-implicit-flag-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![implicit_graph()]);
        let selection = bzl_resolve(
            workspace,
            &[change("cfg/typos.toml", dx_adopt::ChangeKind::Modified)],
            &query,
            &["@demo//core/quality:aspects.bzl%lint_aspect"],
        );
        assert!(selection.targets.contains(&"//...".to_owned()));
        assert_eq!(
            selection.widened,
            widened(&[(
                "//...",
                "changed files reach //cfg:policy, which aspects or bazelrc flags use"
            )])
        );
    }

    #[test]
    fn changes_no_aspect_or_flag_uses_keep_their_owners() {
        let scratch = implicit_workspace("dx-changed-implicit-none-");
        let workspace = scratch.path();
        let query = FakeQuery::new(vec![implicit_graph()]);
        let selection = bzl_resolve(
            workspace,
            &[change("app/main.py", dx_adopt::ChangeKind::Modified)],
            &query,
            &["@demo//core/quality:aspects.bzl%lint_aspect"],
        );
        assert_eq!(selection.targets, strings(&["//app:lib"]));
        assert!(selection.widened.is_empty(), "{:?}", selection.widened);
    }

    #[test]
    fn toolchain_input_bzl_widens() {
        let scratch = bzl_workspace("dx-changed-bzl-toolchain-");
        let workspace = scratch.path();
        write(
            workspace,
            "MODULE.bazel",
            "module(name = \"demo\")\nregister_toolchains(\"//tools/tc:all\")\n",
        )
        .unwrap();
        write(
            workspace,
            "tools/tc/BUILD.bazel",
            "load(\"//tools:defs.bzl\", \"x\")\n",
        )
        .unwrap();
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
        write(workspace, "MODULE.bazel", "module(name = \n").unwrap();
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
        write(workspace, "app/BUILD.bazel", "load(\"//tools:defs.bzl\"\n").unwrap();
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
        assert_eq!(selection.widened, [] as [WidenedScope; 0]);
        assert_eq!(query.calls(), [] as [(Vec<String>, std::path::PathBuf); 0]);
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
        write(workspace, "tools/more.bzl", "y = 1\n").unwrap();
        write(workspace, "BUILD", "load(\"//tools:more.bzl\", \"y\")\n").unwrap();
        write(
            workspace,
            "lib/BUILD.bazel",
            "load(\"//tools:defs.bzl\", \"x\")\nload(\"//tools:more.bzl\", \"y\")\n",
        )
        .unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-unrelated-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/notes.txt", "x\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-spaced-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/space name.py", "x = 1\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-owner-fail-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/a.py", "x = 1\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-partial-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(&workspace, "pkg/BUILD.bazel", "").unwrap();
        write(&workspace, "pkg/owned.py", "x = 1\n").unwrap();
        write(&workspace, "pkg/orphan.py", "x = 1\n").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-worktree-").unwrap();
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/b.py\0".to_vec()),
            FakeQuery::raw(b"pkg/c.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        let mut paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        paths.sort_unstable();
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
        let scratch = dx_test_scratch::scratch("dx-changed-worktree-dedupe-").unwrap();
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
    fn untracked_dx_config_is_a_change() {
        let scratch = dx_test_scratch::scratch("dx-changed-untracked-config-").unwrap();
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(Vec::new()),
            FakeQuery::raw(Vec::new()),
            FakeQuery::raw(b".dx/config/ruff/base.toml\0.dx/local/current\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        let paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(paths, vec![".dx/config/ruff/base.toml"]);
    }

    #[test]
    fn untracked_dx_state_is_not_a_change() {
        let scratch = dx_test_scratch::scratch("dx-changed-untracked-state-").unwrap();
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(Vec::new()),
            FakeQuery::raw(Vec::new()),
            FakeQuery::raw(b".dx/local/leases/process-groups/1-7.lock\0pkg/c.py\0".to_vec()),
        ]);
        let changes =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect("collect");
        let paths: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
        assert_eq!(paths, vec!["pkg/c.py"]);
    }

    #[test]
    fn untracked_names_must_be_utf8() {
        let scratch = dx_test_scratch::scratch("dx-changed-untracked-utf8-").unwrap();
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(b"M\0pkg/a.py\0".to_vec()),
            FakeQuery::raw(vec![0xff, 0x00]),
        ]);
        let err =
            collect_working_tree(Path::new("/hermetic/git"), &workspace, &query).expect_err("utf8");
        assert!(err.message().contains("UTF-8"), "{err}");
    }

    #[test]
    fn range_diffs_base_against_head() {
        let scratch = dx_test_scratch::scratch("dx-changed-range-").unwrap();
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
        let scratch = dx_test_scratch::scratch("dx-changed-merge-base-").unwrap();
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
            let scratch = dx_test_scratch::scratch("dx-changed-merge-base-bad-").unwrap();
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
            assert!(err.message().contains("merge-base"), "{name}: {err}");
        }
    }

    #[test]
    fn shallow_clone_turns_diff_failure_into_actionable_error() {
        let scratch = dx_test_scratch::scratch("dx-changed-shallow-").unwrap();
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
        assert!(err.message().contains("shallow"), "{err}");
        assert!(err.message().contains("fetch --unshallow"), "{err}");
        assert_eq!(query.calls().len(), 1, "shallow needs no extra process");
    }

    #[test]
    fn worktree_shallow_marker_is_found_through_the_gitdir_pointer() {
        let scratch = dx_test_scratch::scratch("dx-changed-shallow-wt-").unwrap();
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
        assert!(err.message().contains("shallow"), "{err}");
    }

    #[test]
    fn non_shallow_diff_failure_stays_plain() {
        let scratch = dx_test_scratch::scratch("dx-changed-diff-fail-").unwrap();
        let workspace = scratch.path().to_path_buf();
        let query = FakeQuery::new(vec![FakeQuery::failed("fatal: bad revision")]);
        let err = diff_name_status(
            Path::new("/hermetic/git"),
            &workspace,
            &query,
            &strings(&["diff", "--name-status", "-z", "base", "head"]),
        )
        .expect_err("plain failure");
        assert!(err.message().contains("git diff failed"), "{err}");
        assert!(err.message().contains("bad revision"), "{err}");
    }

    fn git_failure_message(code: Option<i32>, stderr: &str) -> String {
        let scratch = dx_test_scratch::scratch("dx-changed-git-exit-").unwrap();
        let query = FakeQuery::new(vec![QueryResult {
            code,
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }]);
        merge_base(
            Path::new("/hermetic/git"),
            scratch.path(),
            &query,
            "origin/main",
            "HEAD",
        )
        .expect_err("git failed")
        .message()
        .to_owned()
    }

    #[test]
    fn a_git_failure_names_the_fatal_line_not_a_leading_warning() {
        let message = git_failure_message(
            Some(128),
            "warning: LF will be replaced by CRLF in a.txt.\r\nfatal: bad revision 'x'\r\n",
        );
        assert_eq!(
            message,
            "git merge-base failed: fatal: bad revision 'x' (exit 128)"
        );
    }

    #[test]
    fn a_git_failure_without_output_keeps_its_exit_status() {
        assert_eq!(
            git_failure_message(Some(128), ""),
            "git merge-base failed: no Git diagnostic (exit 128)"
        );
        assert_eq!(
            git_failure_message(None, ""),
            "git merge-base failed: no Git diagnostic (ended by a signal)"
        );
    }

    #[test]
    fn override_changes_need_no_git_without_outside_overrides() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-none-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"x\", path = \"third_party/x\")\n",
        )
        .unwrap();
        let changes =
            local_override_changes(Path::new("/hermetic/git"), &workspace, &NeverQuery, "base")
                .expect("no overrides");
        assert_eq!(changes, [] as [String; 0]);
    }

    #[test]
    fn override_changes_list_files_under_outside_overrides() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\nlocal_path_override(module_name = \"far\", path = \"../../far\")\n",
        ).unwrap();
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
    fn override_changes_keep_a_newline_inside_a_file_name() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-newline-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"sub/\n".to_vec()),
            FakeQuery::raw(b"lib/a\nb.rs\0".to_vec()),
            FakeQuery::raw(Vec::new()),
        ]);
        let changes =
            local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "base-sha")
                .expect("override changes");
        assert_eq!(changes, strings(&["../lib/a\nb.rs"]));
    }

    #[test]
    fn override_changes_read_backslash_override_paths_like_slash_paths() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-backslash-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"..\\\\lib\")\n",
        )
        .unwrap();
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
        assert!(calls[1].0.ends_with(&[":(top)lib".to_owned()]), "{calls:?}");
    }

    #[test]
    fn override_changes_skip_the_override_modules_dx_state() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-dx-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"sub/\n".to_vec()),
            FakeQuery::raw(b"lib/x.rs\0".to_vec()),
            FakeQuery::raw(b"lib/.dx/local/leases/process-groups/1-7.lock\0lib/a/.dx/y\0".to_vec()),
        ]);
        let changes = local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "b")
            .expect("override changes");
        assert_eq!(changes, strings(&["../lib/a/.dx/y", "../lib/x.rs"]));
    }

    #[test]
    fn override_changes_skip_the_workspace_dx_state_under_an_ancestor_override() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-anc-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"up\", path = \"../../a\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![
            FakeQuery::raw(b"a/b/\n".to_vec()),
            FakeQuery::raw(b"a/x.rs\0".to_vec()),
            FakeQuery::raw(b"a/b/.dx/local/leases/process-groups/1-7.lock\0".to_vec()),
        ]);
        let changes = local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "b")
            .expect("override changes");
        assert_eq!(changes, strings(&["../../a/x.rs"]));
    }

    #[test]
    fn override_changes_skip_overrides_outside_the_repository() {
        let scratch = dx_test_scratch::scratch("dx-changed-override-outside-").unwrap();
        let workspace = scratch.path().to_path_buf();
        write(
            &workspace,
            "MODULE.bazel",
            "local_path_override(module_name = \"lib\", path = \"../lib\")\n",
        )
        .unwrap();
        let query = FakeQuery::new(vec![FakeQuery::raw(b"\n".to_vec())]);
        let changes =
            local_override_changes(Path::new("/hermetic/git"), &workspace, &query, "base")
                .expect("outside repo");
        assert_eq!(changes, [] as [String; 0]);
        assert_eq!(query.calls().len(), 1);
    }
}
