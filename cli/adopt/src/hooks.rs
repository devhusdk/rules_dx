use std::collections::BTreeMap;
use std::path::Path;

use super::AdoptError;

pub const HOOK_BUDGET_SECS: u64 = 120;

pub const HOOK_GIT_ENV_VAR: &str = "DX_GIT_BIN";

pub const HOOK_BASELINE_REL: &str = "dx.hooks.toml";

pub const HOOK_OVERLAY_REL: &str = "dx.local.toml";

pub const HOOK_TIMINGS_REL: &str = ".dx/hooks-timings.toml";

pub fn hook_git_is_hermetic(uses_hermetic_git: bool, uses_ambient_git: bool) -> bool {
    uses_hermetic_git && !uses_ambient_git
}

pub fn hook_git_path_is_hermetic(path: &Path) -> bool {
    path.is_absolute()
}

pub fn is_hook_trigger(trigger: &str) -> bool {
    HOOK_TRIGGERS.contains(&trigger)
}

pub fn hook_verb_pipe() -> String {
    HOOK_VERBS.join("|")
}

pub fn hook_trigger_pipe() -> String {
    HOOK_TRIGGERS.join("|")
}

pub fn hook_status_shows_merged(
    shows_baseline: bool,
    shows_overlay: bool,
    shows_timings: bool,
) -> bool {
    shows_baseline && shows_overlay && shows_timings
}

pub const HOOK_MANAGED_MARKER: &str = "# managed by dx hooks";

pub const HOOK_VERBS: &[&str] = &["install", "uninstall", "status", "run"];

pub const HOOK_TRIGGERS: &[&str] = &["pre-commit", "pre-push"];

pub const LOCAL_OVERLAY_COMMENT: &str = "# Local-only overrides (gitignored).";

/// Empty tree object Git diffs against when a push creates a new remote ref.
pub const EMPTY_TREE_SHA: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

/// All-zero object id Git sends for a created or deleted side of a push ref update.
pub const ZERO_SHA: &str = "0000000000000000000000000000000000000000";

/// Filenames Git reports for one workspace change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Copied,
    Deleted,
    Modified,
    Renamed,
    TypeChanged,
    Unmerged,
}

/// One path Git names, with its rename source when Git reports one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitChange {
    pub path: String,
    pub from: Option<String>,
    pub kind: ChangeKind,
}

/// Where a hook run selected its changed paths from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangeSource {
    Staged,
    Pushed,
}

/// One `local_ref local_sha remote_ref remote_sha` line from pre-push stdin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushRef {
    pub local_ref: String,
    pub local_sha: String,
    pub remote_ref: String,
    pub remote_sha: String,
}

/// Reports whether a push ref side names the all-zero object id.
pub fn is_zero_sha(sha: &str) -> bool {
    sha.len() == ZERO_SHA.len() && sha.bytes().all(|byte| byte == b'0')
}

/// Reports whether a pushed ref deletes the remote ref, so no new commits need checks.
pub fn push_ref_is_deletion(push_ref: &PushRef) -> bool {
    is_zero_sha(&push_ref.local_sha)
}

/// Selects the diff base for a pushed ref: none for deletions, the empty tree for new remotes.
pub fn push_diff_base(push_ref: &PushRef) -> Option<String> {
    if push_ref_is_deletion(push_ref) {
        return None;
    }
    if is_zero_sha(&push_ref.remote_sha) {
        return Some(EMPTY_TREE_SHA.to_owned());
    }
    Some(push_ref.remote_sha.clone())
}

/// Names a change source the way hook run summaries spell it.
pub fn change_source_name(source: &ChangeSource) -> &'static str {
    match source {
        ChangeSource::Staged => "staged",
        ChangeSource::Pushed => "pushed",
    }
}

fn change_kind_from_status(status: &str) -> Result<(ChangeKind, bool), String> {
    let kind = match status.as_bytes().first() {
        Some(b'A') => ChangeKind::Added,
        Some(b'C') => ChangeKind::Copied,
        Some(b'D') => ChangeKind::Deleted,
        Some(b'M') => ChangeKind::Modified,
        Some(b'R') => ChangeKind::Renamed,
        Some(b'T') => ChangeKind::TypeChanged,
        Some(b'U') => ChangeKind::Unmerged,
        _ => {
            return Err(format!(
                "hook git change status {status:?} is not supported"
            ))
        }
    };
    let renamed = matches!(kind, ChangeKind::Renamed | ChangeKind::Copied);
    Ok((kind, renamed))
}

fn utf8_path(bytes: &[u8]) -> Result<String, String> {
    std::str::from_utf8(bytes)
        .map(ToOwned::to_owned)
        .map_err(|_| "hook git path is not UTF-8: rename the file to a UTF-8 name".to_owned())
}

/// Parses `git diff --name-status -z` output into typed changes with rename identities kept.
pub fn parse_name_status_nul(output: &[u8]) -> Result<Vec<GitChange>, String> {
    let mut fields: Vec<&[u8]> = output.split(|byte| *byte == 0).collect();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    let mut changes = Vec::new();
    let mut index = 0;
    while index < fields.len() {
        let status = utf8_path(fields[index])?;
        if status.is_empty() {
            return Err("hook git change list has an empty status field".to_owned());
        }
        let (kind, renamed) = change_kind_from_status(&status)?;
        index += 1;
        let first = fields
            .get(index)
            .ok_or_else(|| "hook git change list ends before its path".to_owned())
            .and_then(|field| utf8_path(field))?;
        if first.is_empty() {
            return Err("hook git change list has an empty path".to_owned());
        }
        index += 1;
        let (path, from) = if renamed {
            let second = fields
                .get(index)
                .ok_or_else(|| "hook git rename ends before its target path".to_owned())
                .and_then(|field| utf8_path(field))?;
            if second.is_empty() {
                return Err("hook git rename has an empty target path".to_owned());
            }
            index += 1;
            (second, Some(first))
        } else {
            (first, None)
        };
        changes.push(GitChange { path, from, kind });
    }
    Ok(changes)
}

fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Parses pre-push stdin into one record per `local_ref local_sha remote_ref remote_sha` line.
pub fn parse_push_refs(stdin: &[u8]) -> Result<Vec<PushRef>, String> {
    let text =
        std::str::from_utf8(stdin).map_err(|_| "hook pre-push stdin is not UTF-8".to_owned())?;
    let mut refs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 4 {
            return Err(format!(
                "hook pre-push stdin line needs four fields, got {}: {line:?}",
                fields.len()
            ));
        }
        for sha in [fields[1], fields[3]] {
            if !valid_sha(sha) {
                return Err(format!(
                    "hook pre-push object id is not a 40-hex sha: {sha:?}"
                ));
            }
        }
        refs.push(PushRef {
            local_ref: fields[0].to_owned(),
            local_sha: fields[1].to_owned(),
            remote_ref: fields[2].to_owned(),
            remote_sha: fields[3].to_owned(),
        });
    }
    Ok(refs)
}

/// Sorts changes by path and drops duplicate paths, keeping rename sources.
pub fn dedupe_changes(changes: Vec<GitChange>) -> Vec<GitChange> {
    let mut sorted = changes;
    sorted.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.from.cmp(&right.from))
    });
    sorted.dedup_by(|next, current| next.path == current.path);
    sorted
}

fn is_package_dir(workspace: &Path, dir: &str) -> bool {
    let base = if dir.is_empty() {
        workspace.to_path_buf()
    } else {
        workspace.join(dir)
    };
    if !base.is_dir() {
        return false;
    }
    ["BUILD.bazel", "BUILD"]
        .iter()
        .any(|marker| std::fs::symlink_metadata(base.join(marker)).is_ok())
}

/// Maps a missing path to the nearest enclosing Bazel package pattern for hook fallback.
pub fn nearest_package_pattern(workspace: &Path, rel: &str) -> String {
    let mut dir = match rel.rfind('/') {
        Some(index) => &rel[..index],
        None => "",
    };
    loop {
        if is_package_dir(workspace, dir) {
            if dir.is_empty() {
                return "//...".to_owned();
            }
            return format!("//{dir}/...");
        }
        if dir.is_empty() {
            return "//...".to_owned();
        }
        dir = match dir.rfind('/') {
            Some(index) => &dir[..index],
            None => "",
        };
    }
}

/// Renders the one-line selection summary hook runs print before their checks.
pub fn render_selection_line(
    trigger: &str,
    source: &ChangeSource,
    files: usize,
    targets: usize,
) -> String {
    format!(
        "ran {trigger}: {} {} file(s) as {} target(s); checks read worktree files",
        change_source_name(source),
        files,
        targets
    )
}

pub fn render_local_overlay() -> Result<String, AdoptError> {
    let mut root = toml::Table::new();
    root.insert("hooks".to_owned(), toml::Value::Table(toml::Table::new()));
    let body = toml::to_string(&root).map_err(|e| AdoptError::RenderOverlay {
        detail: e.to_string(),
    })?;
    Ok(format!("{LOCAL_OVERLAY_COMMENT}\n{body}"))
}

pub fn render_hook_shim(trigger: &str) -> String {
    format!(
        "#!/bin/sh\n{HOOK_MANAGED_MARKER} {trigger}\nexec bazel run @rules_dx//:dx -- hooks run {trigger} -- \"$@\"\n"
    )
}

/// Lists managed hook paths that `install_hooks` would still write, without
/// writing anything. A foreign hook fails exactly like the install does; a
/// managed shim with stale bytes or a missing overlay counts as drift.
pub fn check_hooks_install(root: &Path) -> Result<Vec<String>, AdoptError> {
    let mut drifted = Vec::new();
    for trigger in ["pre-commit", "pre-push"] {
        let dest = root.join(".git/hooks").join(trigger);
        let wanted = render_hook_shim(trigger);
        match std::fs::read_to_string(&dest) {
            Ok(existing) => {
                if !existing.contains(HOOK_MANAGED_MARKER) {
                    return Err(AdoptError::UnmanagedInstall {
                        trigger: trigger.to_owned(),
                    });
                }
                if existing != wanted || !hook_shim_executable(&dest) {
                    drifted.push(format!(".git/hooks/{trigger}"));
                }
            }
            Err(error) => {
                if dest.exists() {
                    return Err(AdoptError::ReadHook {
                        trigger: trigger.to_owned(),
                        detail: error.to_string(),
                    });
                }
                drifted.push(format!(".git/hooks/{trigger}"));
            }
        }
    }
    if !root.join("dx.local.toml").is_file() {
        drifted.push("dx.local.toml".to_owned());
    }
    Ok(drifted)
}

#[cfg(unix)]
fn hook_shim_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn hook_shim_executable(_path: &Path) -> bool {
    true
}

/// Lists managed hook shims that `uninstall_hooks` would still remove,
/// without removing anything. A foreign hook fails exactly like the
/// uninstall does; absent shims count as already uninstalled.
pub fn check_hooks_uninstall(root: &Path) -> Result<Vec<String>, AdoptError> {
    let mut drifted = Vec::new();
    for trigger in ["pre-commit", "pre-push"] {
        let dest = root.join(".git/hooks").join(trigger);
        if !dest.exists() {
            continue;
        }
        let existing = std::fs::read_to_string(&dest).map_err(|e| AdoptError::ReadHook {
            trigger: trigger.to_owned(),
            detail: e.to_string(),
        })?;
        if !existing.contains(HOOK_MANAGED_MARKER) {
            return Err(AdoptError::UnmanagedUninstall {
                trigger: trigger.to_owned(),
            });
        }
        drifted.push(format!(".git/hooks/{trigger}"));
    }
    Ok(drifted)
}

pub fn install_hooks(root: &Path) -> Result<Vec<String>, AdoptError> {
    let hooks_dir = root.join(".git/hooks");
    std::fs::create_dir_all(&hooks_dir).map_err(|e| AdoptError::CreateHooksDir {
        detail: e.to_string(),
    })?;
    let mut installed = Vec::new();
    for trigger in ["pre-commit", "pre-push"] {
        let dest = hooks_dir.join(trigger);
        if dest.exists() {
            let existing = std::fs::read_to_string(&dest).map_err(|e| AdoptError::ReadHook {
                trigger: trigger.to_owned(),
                detail: e.to_string(),
            })?;
            if !existing.contains(HOOK_MANAGED_MARKER) {
                return Err(AdoptError::UnmanagedInstall {
                    trigger: trigger.to_owned(),
                });
            }
        }
        dx_atomic_fs::write_atomic(&dest, render_hook_shim(trigger).as_bytes()).map_err(|e| {
            AdoptError::WriteHook {
                trigger: trigger.to_owned(),
                detail: e.to_string(),
            }
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest)
                .map_err(|e| AdoptError::StatHook {
                    trigger: trigger.to_owned(),
                    detail: e.to_string(),
                })?
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest, perms).map_err(|e| AdoptError::ChmodHook {
                trigger: trigger.to_owned(),
                detail: e.to_string(),
            })?;
        }
        installed.push(format!(".git/hooks/{trigger}"));
    }
    let overlay = root.join("dx.local.toml");
    if !overlay.exists() {
        let content = render_local_overlay()?;
        dx_atomic_fs::write_atomic(&overlay, content.as_bytes()).map_err(|e| {
            AdoptError::WriteOverlay {
                detail: e.to_string(),
            }
        })?;
        installed.push("dx.local.toml".to_owned());
    }
    Ok(installed)
}

pub fn uninstall_hooks(root: &Path) -> Result<Vec<String>, AdoptError> {
    let mut removed = Vec::new();
    for trigger in ["pre-commit", "pre-push"] {
        let dest = root.join(".git/hooks").join(trigger);
        if !dest.exists() {
            continue;
        }
        let existing = std::fs::read_to_string(&dest).map_err(|e| AdoptError::ReadHook {
            trigger: trigger.to_owned(),
            detail: e.to_string(),
        })?;
        if !existing.contains(HOOK_MANAGED_MARKER) {
            return Err(AdoptError::UnmanagedUninstall {
                trigger: trigger.to_owned(),
            });
        }
        std::fs::remove_file(&dest).map_err(|e| AdoptError::RemoveHook {
            trigger: trigger.to_owned(),
            detail: e.to_string(),
        })?;
        removed.push(format!(".git/hooks/{trigger}"));
    }
    Ok(removed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HooksConfig {
    pub pre_commit: Vec<String>,
    pub pre_push: Vec<String>,
    pub budget_secs: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
struct HooksTable {
    #[serde(default)]
    pre_commit: Option<Vec<String>>,
    #[serde(default)]
    pre_push: Option<Vec<String>>,
    #[serde(default)]
    budget_secs: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
struct HooksFile {
    #[serde(default)]
    hooks: Option<HooksTable>,
}

pub fn default_hooks_config() -> HooksConfig {
    HooksConfig {
        pre_commit: vec!["format --check".to_owned(), "lint --check".to_owned()],
        pre_push: vec![
            "typecheck --check".to_owned(),
            "generate --check".to_owned(),
        ],
        budget_secs: HOOK_BUDGET_SECS,
    }
}

fn parse_hooks_file(text: &str) -> Result<HooksFile, AdoptError> {
    toml::from_str(text).map_err(|e| AdoptError::InvalidHooks {
        detail: e.to_string(),
    })
}

fn merge_hooks_config(baseline: &HooksFile, overlay: &HooksFile) -> HooksConfig {
    let defaults = default_hooks_config();
    let base = baseline.hooks.as_ref();
    let over = overlay.hooks.as_ref();
    HooksConfig {
        pre_commit: over
            .and_then(|t| t.pre_commit.clone())
            .or_else(|| base.and_then(|t| t.pre_commit.clone()))
            .unwrap_or(defaults.pre_commit),
        pre_push: over
            .and_then(|t| t.pre_push.clone())
            .or_else(|| base.and_then(|t| t.pre_push.clone()))
            .unwrap_or(defaults.pre_push),
        budget_secs: over
            .and_then(|t| t.budget_secs)
            .or_else(|| base.and_then(|t| t.budget_secs))
            .unwrap_or(defaults.budget_secs),
    }
}

pub fn load_hooks_config(
    baseline_text: Option<&str>,
    overlay_text: Option<&str>,
) -> Result<HooksConfig, AdoptError> {
    let baseline = match baseline_text {
        Some(text) => parse_hooks_file(text)?,
        None => HooksFile::default(),
    };
    let overlay = match overlay_text {
        Some(text) => parse_hooks_file(text)?,
        None => HooksFile::default(),
    };
    Ok(merge_hooks_config(&baseline, &overlay))
}

pub fn checks_for_trigger(config: &HooksConfig, trigger: &str) -> Vec<String> {
    match trigger {
        "pre-commit" => config.pre_commit.clone(),
        "pre-push" => config.pre_push.clone(),
        _ => Vec::new(),
    }
}

pub fn hook_check_timed_out(elapsed_secs: f64, budget_secs: u64) -> bool {
    elapsed_secs > budget_secs as f64
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HookTimings {
    pub secs_by_check: BTreeMap<String, f64>,
}

fn parse_timings_file(text: &str) -> Result<HookTimings, AdoptError> {
    #[derive(serde::Deserialize, Default)]
    struct TimingsFile {
        #[serde(default)]
        timings: BTreeMap<String, f64>,
    }
    let parsed: TimingsFile = toml::from_str(text).map_err(|e| AdoptError::InvalidTimings {
        detail: e.to_string(),
    })?;
    for (check, secs) in &parsed.timings {
        if !secs.is_finite() || *secs < 0.0 {
            return Err(AdoptError::InvalidTimings {
                detail: format!("timing for {check:?} is not a finite non-negative number"),
            });
        }
    }
    Ok(HookTimings {
        secs_by_check: parsed.timings,
    })
}

pub fn load_hook_timings(text: Option<&str>) -> Result<HookTimings, AdoptError> {
    match text {
        Some(body) => parse_timings_file(body),
        None => Ok(HookTimings::default()),
    }
}

pub fn render_hook_timings(timings: &HookTimings) -> Result<String, AdoptError> {
    #[derive(serde::Serialize)]
    struct TimingsFile<'a> {
        timings: &'a BTreeMap<String, f64>,
    }
    let body = toml::to_string(&TimingsFile {
        timings: &timings.secs_by_check,
    })
    .map_err(|e| AdoptError::RenderTimings {
        detail: e.to_string(),
    })?;
    Ok(body)
}

pub fn render_hooks_status_merged(
    config: &HooksConfig,
    timings: &HookTimings,
    baseline_src: &str,
    overlay_src: &str,
) -> String {
    let mut view = String::new();
    view.push_str("effective:\n");
    view.push_str(&format!(
        "pre-commit: {}\n",
        if config.pre_commit.is_empty() {
            "(none)".to_owned()
        } else {
            config.pre_commit.join(", ")
        }
    ));
    view.push_str(&format!(
        "pre-push: {}\n",
        if config.pre_push.is_empty() {
            "(none)".to_owned()
        } else {
            config.pre_push.join(", ")
        }
    ));
    view.push_str(&format!("budget_secs: {}\n", config.budget_secs));
    view.push_str("baseline:\n");
    view.push_str(&format!("source: {baseline_src}\n"));
    view.push_str("overlay:\n");
    view.push_str(&format!("source: {overlay_src}\n"));
    view.push_str("timings:\n");
    if timings.secs_by_check.is_empty() {
        view.push_str("(no timings recorded; run hooks to measure)\n");
    } else {
        for (check, secs) in &timings.secs_by_check {
            view.push_str(&format!("{check}: {secs:.2}s (measured)\n"));
        }
    }
    view
}

pub fn render_hooks_status(baseline: &str, overlay: &str, timings: &str) -> String {
    format!("baseline:\n{baseline}\noverlay:\n{overlay}\ntimings:\n{timings}\n")
}

#[cfg(test)]
mod tests {
    use super::super::{
        check_hooks_install, check_hooks_uninstall, checks_for_trigger, default_hooks_config,
        hook_check_timed_out, hook_git_is_hermetic, hook_git_path_is_hermetic,
        hook_status_shows_merged, install_hooks, is_hook_trigger, load_hook_timings,
        load_hooks_config, render_hook_timings, render_hooks_status_merged, render_local_overlay,
        uninstall_hooks, HOOK_BUDGET_SECS, HOOK_MANAGED_MARKER, HOOK_TRIGGERS,
        LOCAL_OVERLAY_COMMENT,
    };
    use super::{render_hook_shim, render_hooks_status};

    #[test]
    fn hooks_reexports_match_local_definitions() {
        assert_eq!(super::HOOK_BUDGET_SECS, HOOK_BUDGET_SECS);
        assert_eq!(super::HOOK_MANAGED_MARKER, HOOK_MANAGED_MARKER);
        assert_eq!(super::LOCAL_OVERLAY_COMMENT, LOCAL_OVERLAY_COMMENT);
        assert!(render_hook_shim("pre-commit").contains(HOOK_MANAGED_MARKER));
        assert!(render_hooks_status("b", "o", "t").contains("baseline:\nb"));
    }

    #[test]
    fn check_hooks_install_and_uninstall_compare_without_writing() {
        let scratch = dx_test_scratch::scratch("dx-adopt-check-hooks-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".git")).expect("git");
        let drifted = check_hooks_install(root).expect("checks");
        assert_eq!(
            drifted,
            vec![
                ".git/hooks/pre-commit".to_owned(),
                ".git/hooks/pre-push".to_owned(),
                "dx.local.toml".to_owned(),
            ],
            "{drifted:?}"
        );
        install_hooks(root).expect("installs");
        let drifted = check_hooks_install(root).expect("rechecks");
        assert!(drifted.is_empty(), "{drifted:?}");
        let drifted = check_hooks_uninstall(root).expect("checks uninstall");
        assert_eq!(
            drifted,
            vec![
                ".git/hooks/pre-commit".to_owned(),
                ".git/hooks/pre-push".to_owned(),
            ],
            "{drifted:?}"
        );
        uninstall_hooks(root).expect("uninstalls");
        assert!(
            check_hooks_uninstall(root).expect("rechecks").is_empty(),
            "absent shims are already uninstalled"
        );
        assert_eq!(
            check_hooks_install(root).expect("rechecks"),
            vec![
                ".git/hooks/pre-commit".to_owned(),
                ".git/hooks/pre-push".to_owned(),
            ],
            "uninstall restores shim drift while the overlay stays"
        );
    }

    #[test]
    fn check_hooks_rejects_foreign_shims_like_install() {
        let scratch = dx_test_scratch::scratch("dx-adopt-check-hooks-foreign-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".git/hooks")).expect("hooks");
        std::fs::write(root.join(".git/hooks/pre-commit"), "foreign hook").expect("foreign");
        assert!(matches!(
            check_hooks_install(root),
            Err(super::super::AdoptError::UnmanagedInstall { .. })
        ));
        assert!(matches!(
            check_hooks_uninstall(root),
            Err(super::super::AdoptError::UnmanagedUninstall { .. })
        ));
        std::fs::write(
            root.join(".git/hooks/pre-commit"),
            format!("{HOOK_MANAGED_MARKER} pre-commit\nstale bytes\n"),
        )
        .expect("stale shim");
        let drifted = check_hooks_install(root).expect("checks");
        assert!(
            drifted.contains(&".git/hooks/pre-commit".to_owned()),
            "stale managed bytes are drift: {drifted:?}"
        );
    }

    #[test]
    fn hook_shim_runs_dx_through_the_public_label() {
        for trigger in HOOK_TRIGGERS {
            let shim = render_hook_shim(trigger);
            assert!(
                shim.contains(&format!(
                    "exec bazel run @rules_dx//:dx -- hooks run {trigger} -- \"$@\"\n"
                )),
                "{shim}"
            );
            assert!(!shim.contains("//cli/cli:dx"), "{shim}");
        }
    }

    #[test]
    fn hook_git_never_falls_back_to_ambient() {
        assert!(hook_git_is_hermetic(true, false));
        assert!(!hook_git_is_hermetic(true, true));
        assert!(!hook_git_is_hermetic(false, false));
        assert!(!hook_git_is_hermetic(false, true));
    }

    #[test]
    fn hook_git_path_needs_absolute() {
        let absolute = if cfg!(windows) {
            std::path::Path::new(r"C:\hermetic\git")
        } else {
            std::path::Path::new("/hermetic/git")
        };
        assert!(hook_git_path_is_hermetic(absolute));
        assert!(!hook_git_path_is_hermetic(std::path::Path::new("git")));
        assert!(!hook_git_path_is_hermetic(std::path::Path::new(
            "tools/git"
        )));
    }

    #[test]
    fn hook_triggers_cover_both() {
        assert!(is_hook_trigger("pre-commit"));
        assert!(is_hook_trigger("pre-push"));
        assert!(!is_hook_trigger("pre-merge"));
        assert!(!is_hook_trigger(""));
    }

    #[test]
    fn hook_status_shows_baseline_overlay_and_timings() {
        assert!(hook_status_shows_merged(true, true, true));
        assert!(!hook_status_shows_merged(false, true, true));
        assert!(!hook_status_shows_merged(true, false, true));
        assert!(!hook_status_shows_merged(true, true, false));
    }

    #[test]
    fn hook_budget_freeze_holds() {
        assert_eq!(HOOK_BUDGET_SECS, 120);
    }

    #[test]
    fn hooks_install_refuses_unmanaged_and_manages_shims() {
        let scratch = dx_test_scratch::scratch("dx-adopt-hook-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".git/hooks")).expect("tmp");
        let installed = install_hooks(&root).expect("install");
        assert!(installed.iter().any(|p| p == ".git/hooks/pre-commit"));
        std::fs::write(root.join(".git/hooks/pre-commit"), "# custom hook\n").expect("unmanaged");
        assert!(install_hooks(&root).is_err());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn hooks_uninstall_removes_only_managed_shims() {
        let scratch = dx_test_scratch::scratch("dx-adopt-hook-uninstall-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".git/hooks")).expect("tmp");
        install_hooks(&root).expect("install");
        let removed = uninstall_hooks(&root).expect("uninstall");
        assert!(removed.iter().any(|p| p == ".git/hooks/pre-commit"));
        assert!(!root.join(".git/hooks/pre-commit").exists());
        let again = uninstall_hooks(&root).expect("uninstall again");
        assert!(again.is_empty());
        std::fs::write(root.join(".git/hooks/pre-commit"), "# custom hook\n").expect("unmanaged");
        assert!(uninstall_hooks(&root).is_err());
        assert!(root.join(".git/hooks/pre-commit").exists());
        scratch.close().expect("cleanup");
    }

    #[test]
    fn local_overlay_stays_byte_identical() {
        assert_eq!(
            render_local_overlay().expect("overlay"),
            "# Local-only overrides (gitignored).\n[hooks]\n"
        );
    }

    #[test]
    fn local_overlay_round_trips_through_toml() {
        let emitted = render_local_overlay().expect("overlay");
        let parsed: toml::Table = emitted.parse().expect("valid TOML");
        assert!(parsed.contains_key("hooks"));
        let hooks = parsed["hooks"].as_table().expect("hooks table");
        assert!(hooks.is_empty());
    }

    #[test]
    fn hooks_install_writes_toml_backed_overlay() {
        let scratch = dx_test_scratch::scratch("dx-adopt-hook-overlay-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join(".git/hooks")).expect("tmp");
        let installed = install_hooks(&root).expect("install");
        assert!(installed.iter().any(|p| p == "dx.local.toml"));
        let written = std::fs::read_to_string(root.join("dx.local.toml")).expect("read overlay");
        assert_eq!(written, render_local_overlay().expect("overlay"));
        let parsed: toml::Table = written.parse().expect("valid TOML");
        assert!(parsed.contains_key("hooks"));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn hooks_config_merges_overlay_over_baseline() {
        let baseline = "[hooks]\npre_commit = [\"format --check\", \"lint --check\"]\npre_push = [\"typecheck --check\", \"generate --check\"]\nbudget_secs = 120\n";
        let merged = load_hooks_config(Some(baseline), None).expect("baseline only");
        assert_eq!(merged, default_hooks_config());
        assert_eq!(
            checks_for_trigger(&merged, "pre-commit"),
            vec!["format --check".to_owned(), "lint --check".to_owned()]
        );
        assert_eq!(
            checks_for_trigger(&merged, "pre-push"),
            vec![
                "typecheck --check".to_owned(),
                "generate --check".to_owned()
            ]
        );
        assert!(checks_for_trigger(&merged, "bogus").is_empty());
        let overlay = "[hooks]\npre_commit = [\"format --check\"]\n";
        let merged = load_hooks_config(Some(baseline), Some(overlay)).expect("merged");
        assert_eq!(merged.pre_commit, vec!["format --check".to_owned()]);
        assert_eq!(merged.pre_push.len(), 2);
        assert_eq!(merged.budget_secs, HOOK_BUDGET_SECS);
        let missing = load_hooks_config(None, None).expect("defaults");
        assert_eq!(missing, default_hooks_config());
        assert!(load_hooks_config(Some("not toml = ["), None).is_err());
        assert!(load_hooks_config(None, Some("[hooks]\nbudget_secs = \"x\"\n")).is_err());
    }

    #[test]
    fn hook_budget_blocks_on_timeout() {
        assert!(!hook_check_timed_out(119.9, 120));
        assert!(!hook_check_timed_out(120.0, 120));
        assert!(hook_check_timed_out(120.01, 120));
    }

    #[test]
    fn hook_timings_round_trip_through_toml() {
        let empty = load_hook_timings(None).expect("missing means no runs");
        assert!(empty.secs_by_check.is_empty());
        let parsed =
            load_hook_timings(Some("[timings]\n\"format --check\" = 1.5\n")).expect("parse");
        assert_eq!(parsed.secs_by_check["format --check"], 1.5);
        let rendered = render_hook_timings(&parsed).expect("render");
        let again = load_hook_timings(Some(&rendered)).expect("reparse");
        assert_eq!(parsed, again);
        assert!(load_hook_timings(Some("not toml = [")).is_err());
        assert!(load_hook_timings(Some("[timings]\n\"x\" = -1.0\n")).is_err());
    }

    #[test]
    fn merged_status_shows_effective_and_measured() {
        let config = default_hooks_config();
        let empty = load_hook_timings(None).expect("empty");
        let view = render_hooks_status_merged(&config, &empty, "dx.hooks.toml", "absent");
        assert!(view.contains("baseline:"));
        assert!(view.contains("overlay:"));
        assert!(view.contains("timings:"));
        assert!(view.contains("effective:"));
        assert!(view.contains("format --check"));
        assert!(view.contains("(no timings recorded; run hooks to measure)"));
        assert!(!view.contains("p95"));
        let timed =
            load_hook_timings(Some("[timings]\n\"format --check\" = 1.23\n")).expect("timed");
        let view = render_hooks_status_merged(&config, &timed, "dx.hooks.toml", "dx.local.toml");
        assert!(view.contains("1.23s (measured)"));
        assert!(!view.contains("p95 12s"));
    }

    #[test]
    fn name_status_nul_keeps_verbatim_paths_and_rename_identities() {
        let output = b"M\x00pkg/a.py\x00A\x00pkg/space name.py\x00D\x00pkg/old.py\x00R100\x00pkg/was.py\x00pkg/now.py\x00";
        let changes = super::parse_name_status_nul(output).expect("parse");
        assert_eq!(changes.len(), 4);
        assert_eq!(changes[0].path, "pkg/a.py");
        assert_eq!(changes[0].kind, super::ChangeKind::Modified);
        assert_eq!(changes[0].from, None);
        assert_eq!(changes[1].path, "pkg/space name.py");
        assert_eq!(changes[1].kind, super::ChangeKind::Added);
        assert_eq!(changes[2].kind, super::ChangeKind::Deleted);
        assert_eq!(changes[3].path, "pkg/now.py");
        assert_eq!(changes[3].from.as_deref(), Some("pkg/was.py"));
        assert_eq!(changes[3].kind, super::ChangeKind::Renamed);
        assert!(super::parse_name_status_nul(b"").expect("empty").is_empty());
    }

    #[test]
    fn name_status_nul_rejects_truncation_and_non_utf8() {
        assert!(super::parse_name_status_nul(b"M\x00").is_err());
        assert!(super::parse_name_status_nul(b"R100\x00a\x00").is_err());
        assert!(super::parse_name_status_nul(b"Z\x00a\x00").is_err());
        assert!(super::parse_name_status_nul(b"M\x00\xff\x00").is_err());
        assert!(super::parse_name_status_nul(b"\x00a\x00").is_err());
    }

    #[test]
    fn push_refs_parse_four_fields_and_classify_empty_sides() {
        let zeros = super::ZERO_SHA;
        let local = "a".repeat(40);
        let remote = "b".repeat(40);
        let stdin = format!("refs/heads/main {local} refs/heads/main {remote}\nrefs/heads/new {local} refs/heads/new {zeros}\nrefs/heads/gone {zeros} refs/heads/gone {remote}\n");
        let refs = super::parse_push_refs(stdin.as_bytes()).expect("parse");
        assert_eq!(refs.len(), 3);
        assert!(!super::push_ref_is_deletion(&refs[0]));
        assert_eq!(
            super::push_diff_base(&refs[0]).as_deref(),
            Some(remote.as_str())
        );
        assert_eq!(
            super::push_diff_base(&refs[1]).as_deref(),
            Some(super::EMPTY_TREE_SHA)
        );
        assert!(super::push_ref_is_deletion(&refs[2]));
        assert_eq!(super::push_diff_base(&refs[2]), None);
        assert!(super::parse_push_refs(b"").expect("empty").is_empty());
        assert!(super::parse_push_refs(b"only three fields here\n").is_err());
        assert!(super::parse_push_refs(b"r s r s extra\n").is_err());
        assert!(super::parse_push_refs(b"\xff").is_err());
    }

    #[test]
    fn change_selection_dedupes_and_falls_back_to_packages() {
        let scratch = dx_test_scratch::scratch("dx-adopt-hook-selection-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("pkg")).expect("pkg");
        std::fs::write(root.join("pkg/BUILD.bazel"), "").expect("build");
        assert_eq!(
            super::nearest_package_pattern(&root, "pkg/gone.py"),
            "//pkg/..."
        );
        assert_eq!(super::nearest_package_pattern(&root, "top.py"), "//...");
        assert_eq!(
            super::nearest_package_pattern(&root, "deep/nested/gone.py"),
            "//..."
        );
        let changes = vec![
            super::GitChange {
                path: "pkg/b.py".to_owned(),
                from: None,
                kind: super::ChangeKind::Added,
            },
            super::GitChange {
                path: "pkg/a.py".to_owned(),
                from: Some("pkg/was.py".to_owned()),
                kind: super::ChangeKind::Renamed,
            },
            super::GitChange {
                path: "pkg/a.py".to_owned(),
                from: None,
                kind: super::ChangeKind::Modified,
            },
        ];
        let deduped = super::dedupe_changes(changes);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].path, "pkg/a.py");
        assert_eq!(deduped[1].path, "pkg/b.py");
        let line = super::render_selection_line("pre-commit", &super::ChangeSource::Staged, 2, 1);
        assert!(line.contains("staged 2 file(s) as 1 target(s)"));
        assert!(line.contains("worktree files"));
        assert_eq!(
            super::change_source_name(&super::ChangeSource::Pushed),
            "pushed"
        );
        scratch.close().expect("cleanup");
    }
}
