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
        checks_for_trigger, default_hooks_config, hook_check_timed_out, hook_git_is_hermetic,
        hook_git_path_is_hermetic, hook_status_shows_merged, install_hooks, is_hook_trigger,
        load_hook_timings, load_hooks_config, render_hook_timings, render_hooks_status_merged,
        render_local_overlay, uninstall_hooks, HOOK_BUDGET_SECS, HOOK_MANAGED_MARKER,
        HOOK_TRIGGERS, LOCAL_OVERLAY_COMMENT,
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
}
