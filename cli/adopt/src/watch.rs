use std::path::{Path, PathBuf};
use std::time::Duration;

use super::AdoptError;

pub const WATCH_DEBOUNCE_MS: u64 = 200;

pub const WATCHABLE_COMMANDS: &[&str] = &[
    "build",
    "test",
    "run",
    "lint",
    "typecheck",
    "format",
    "check",
    "fix",
];

pub fn plan_watch(command: &str, ci: bool) -> Result<String, AdoptError> {
    if ci {
        return Err(AdoptError::WatchRefusesCi);
    }
    if !WATCHABLE_COMMANDS.contains(&command) {
        return Err(AdoptError::NotWatchable {
            command: command.to_owned(),
        });
    }
    Ok(format!("watch:{command}:debounce={WATCH_DEBOUNCE_MS}ms"))
}

pub fn should_watch_path(path: &Path) -> bool {
    if path.file_name().is_some_and(|name| name == "dx.local.toml") {
        return false;
    }
    for component in path.components() {
        let text = component.as_os_str().to_string_lossy();
        if text == ".dx" || text.starts_with("bazel-") {
            return false;
        }
    }
    true
}

pub fn coalesce_watch_paths(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.retain(|path| should_watch_path(path));
    paths.sort();
    paths.dedup();
    paths
}

pub fn watch_for_change(watch_root: &Path, timeout: Duration) -> Result<Vec<PathBuf>, AdoptError> {
    use notify::RecursiveMode;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer =
        notify_debouncer_mini::new_debouncer(Duration::from_millis(WATCH_DEBOUNCE_MS), tx)
            .map_err(|e| AdoptError::WatchSpawn {
                detail: e.to_string(),
            })?;
    debouncer
        .watcher()
        .watch(watch_root, RecursiveMode::Recursive)
        .map_err(|e| AdoptError::WatchSpawn {
            detail: e.to_string(),
        })?;
    match rx.recv_timeout(timeout) {
        Ok(Ok(events)) => {
            let paths: Vec<PathBuf> = events.into_iter().map(|event| event.path).collect();
            Ok(coalesce_watch_paths(paths))
        }
        Ok(Err(e)) => Err(AdoptError::WatchFailed {
            detail: e.to_string(),
        }),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(Vec::new()),
        Err(e) => Err(AdoptError::WatchFailed {
            detail: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_freeze_holds() {
        assert!(plan_watch("test", false).is_ok());
        assert!(plan_watch("docs", false).is_err());
        assert!(plan_watch("test", true).is_err());
    }

    #[test]
    fn watch_execution_gaps_matrix_is_wont_fix() {
        for watchable in [
            "build",
            "test",
            "run",
            "lint",
            "typecheck",
            "format",
            "check",
            "fix",
        ] {
            assert!(
                plan_watch(watchable, false).is_ok(),
                "{watchable} must stay watchable"
            );
            assert!(
                plan_watch(watchable, true).is_err(),
                "{watchable} must still refuse CI"
            );
        }
        for not_watchable in [
            "audit",
            "bazel",
            "bump",
            "clean",
            "codegen",
            "completion",
            "coverage",
            "deps",
            "deploy",
            "docs",
            "env",
            "generate",
            "hooks",
            "init",
            "migrate",
            "owners",
            "setup",
            "status",
            "update",
            "version",
            "watch",
            "why",
        ] {
            let err = plan_watch(not_watchable, false).expect_err("not watchable");
            assert_eq!(
                err,
                AdoptError::NotWatchable {
                    command: not_watchable.to_owned(),
                },
                "{not_watchable} must stay not watchable"
            );
        }
        assert_eq!(WATCHABLE_COMMANDS.len(), 8);
        assert_eq!(WATCH_DEBOUNCE_MS, 200);
    }

    #[test]
    fn watch_coalesces_bursts_into_a_single_trigger() {
        let first = PathBuf::from("/tmp/ws/src/main.rs");
        let second = PathBuf::from("/tmp/ws/src/lib.rs");
        let trigger = coalesce_watch_paths(vec![
            first.clone(),
            first.clone(),
            second.clone(),
            first.clone(),
        ]);
        assert_eq!(trigger, vec![second, first]);
    }

    #[test]
    fn watch_ignores_frozen_outputs_and_overlays() {
        assert!(!should_watch_path(Path::new("/tmp/ws/bazel-bin/a.rs")));
        assert!(!should_watch_path(Path::new(
            "/tmp/ws/bazel-out/k8-fastbuild/bin/a.rs"
        )));
        assert!(!should_watch_path(Path::new("/tmp/ws/.dx/current")));
        assert!(!should_watch_path(Path::new("/tmp/ws/.dx/bin/dx")));
        assert!(!should_watch_path(Path::new("/tmp/ws/dx.local.toml")));
        assert!(!should_watch_path(Path::new("/tmp/ws/sub/dx.local.toml")));
        assert!(should_watch_path(Path::new("/tmp/ws/src/main.rs")));
        assert!(should_watch_path(Path::new("/tmp/ws/BUILD.bazel")));
    }

    #[test]
    fn watch_coalesce_drops_ignored_paths() {
        let trigger = coalesce_watch_paths(vec![
            PathBuf::from("/tmp/ws/bazel-bin/a.rs"),
            PathBuf::from("/tmp/ws/.dx/current"),
            PathBuf::from("/tmp/ws/dx.local.toml"),
            PathBuf::from("/tmp/ws/src/main.rs"),
        ]);
        assert_eq!(trigger, vec![PathBuf::from("/tmp/ws/src/main.rs")]);
    }

    #[test]
    fn watch_reports_created_files_and_times_out_when_idle() {
        let scratch = dx_test_scratch::scratch("dx-adopt-watch-");
        let root = scratch.path().to_path_buf();
        let writer = root.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let _ = std::fs::write(writer.join("trigger.txt"), "change");
        });
        let trigger = watch_for_change(&root, Duration::from_secs(5)).expect("watch create");
        let canonical_root = root.canonicalize().expect("canonical watch root");
        assert!(
            trigger
                .iter()
                .any(|path| path.ends_with("trigger.txt") || path == &canonical_root),
            "created file must trigger a rebuild: {trigger:?}"
        );
        let idle = watch_for_change(&root, Duration::from_millis(300)).expect("watch idle");
        assert!(idle.is_empty(), "idle watch must time out empty: {idle:?}");
    }
}
