use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::AdoptError;

pub const WATCH_DEBOUNCE_MS: u64 = 200;

/// Quiet time after a change before one wait reports it.
const WATCH_SETTLE: Duration = Duration::from_millis(2 * WATCH_DEBOUNCE_MS);

/// Longest one wait keeps collecting a burst after its first change.
const WATCH_SETTLE_CAP: Duration = Duration::from_millis(10 * WATCH_DEBOUNCE_MS);

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

/// Whether a path relative to the watch root can trigger a rerun.
pub fn should_watch_path(relative: &Path) -> bool {
    for component in relative.components() {
        let text = component.as_os_str().to_string_lossy();
        if text == ".dx" || text == ".git" || text.starts_with("bazel-") {
            return false;
        }
    }
    true
}

/// Sorted, deduplicated paths under one of the roots that can trigger a rerun.
pub fn coalesce_watch_paths(roots: &[&Path], mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.retain(|path| {
        roots
            .iter()
            .any(|root| path.strip_prefix(root).is_ok_and(should_watch_path))
    });
    paths.sort();
    paths.dedup();
    paths
}

/// Whether an event changes the tree rather than only reading it.
pub fn is_change(kind: &notify::EventKind) -> bool {
    use notify::event::{AccessKind, AccessMode, EventKind};
    match kind {
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
        EventKind::Access(_) => false,
        _ => true,
    }
}

/// Whether an event only touches directory timestamps, which Windows reports on reads.
pub fn is_directory_touch(event: &notify::Event) -> bool {
    use notify::event::{EventKind, ModifyKind};
    matches!(event.kind, EventKind::Modify(ModifyKind::Any))
        && !event.paths.is_empty()
        && event.paths.iter().all(|path| path.is_dir())
}

/// The platform watcher with read-only access events dropped.
pub struct ChangeWatcher(notify::RecommendedWatcher);

impl notify::Watcher for ChangeWatcher {
    fn new<F: notify::EventHandler>(
        mut handler: F,
        config: notify::Config,
    ) -> notify::Result<Self> {
        notify::RecommendedWatcher::new(
            move |event: notify::Result<notify::Event>| {
                let relevant = match &event {
                    Ok(event) => {
                        is_change(&event.kind) && !(cfg!(windows) && is_directory_touch(event))
                    }
                    Err(_) => true,
                };
                if relevant {
                    handler.handle_event(event);
                }
            },
            config,
        )
        .map(ChangeWatcher)
    }

    fn watch(&mut self, path: &Path, mode: notify::RecursiveMode) -> notify::Result<()> {
        self.0.watch(path, mode)
    }

    fn unwatch(&mut self, path: &Path) -> notify::Result<()> {
        self.0.unwatch(path)
    }

    fn configure(&mut self, option: notify::Config) -> notify::Result<bool> {
        self.0.configure(option)
    }

    fn kind() -> notify::WatcherKind {
        notify::RecommendedWatcher::kind()
    }
}

fn batch_paths(
    batch: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>,
) -> Result<Vec<PathBuf>, AdoptError> {
    batch
        .map(|events| events.into_iter().map(|event| event.path).collect())
        .map_err(|e| AdoptError::WatchFailed {
            detail: e.to_string(),
        })
}

/// One filesystem observer that stays alive across wrapped executions.
pub struct WatchSession {
    root: PathBuf,
    canonical_root: PathBuf,
    reports: Vec<PathBuf>,
    _debouncer: notify_debouncer_mini::Debouncer<ChangeWatcher>,
    events: std::sync::mpsc::Receiver<
        Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>,
    >,
}

impl WatchSession {
    pub fn start(watch_root: &Path, reports: &[PathBuf]) -> Result<Self, AdoptError> {
        use notify::RecursiveMode;
        let (tx, events) = std::sync::mpsc::channel();
        let config = notify_debouncer_mini::Config::default()
            .with_timeout(Duration::from_millis(WATCH_DEBOUNCE_MS));
        let mut debouncer = notify_debouncer_mini::new_debouncer_opt::<_, ChangeWatcher>(
            config, tx,
        )
        .map_err(|e| AdoptError::WatchSpawn {
            detail: e.to_string(),
        })?;
        debouncer
            .watcher()
            .watch(watch_root, RecursiveMode::Recursive)
            .map_err(|e| AdoptError::WatchSpawn {
                detail: e.to_string(),
            })?;
        let canonical_root = watch_root
            .canonicalize()
            .unwrap_or_else(|_| watch_root.to_path_buf());
        let reports = reports
            .iter()
            .filter_map(|report| {
                report
                    .strip_prefix(watch_root)
                    .or_else(|_| report.strip_prefix(&canonical_root))
                    .ok()
                    .map(Path::to_path_buf)
            })
            .collect();
        Ok(Self {
            root: watch_root.to_path_buf(),
            canonical_root,
            reports,
            _debouncer: debouncer,
            events,
        })
    }

    pub fn wait(&self, timeout: Duration) -> Result<Vec<PathBuf>, AdoptError> {
        let deadline = Instant::now() + timeout;
        let mut paths = loop {
            let received = self
                .events
                .recv_timeout(deadline.saturating_duration_since(Instant::now()));
            self.check_root()?;
            let mut paths = match received {
                Ok(batch) => batch_paths(batch)?,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => return Ok(Vec::new()),
                Err(e) => {
                    return Err(AdoptError::WatchFailed {
                        detail: e.to_string(),
                    });
                }
            };
            for batch in self.events.try_iter() {
                paths.extend(batch_paths(batch)?);
            }
            let paths = self.coalesce(paths);
            if !paths.is_empty() {
                break paths;
            }
            if Instant::now() >= deadline {
                return Ok(paths);
            }
        };
        let cap = Instant::now() + WATCH_SETTLE_CAP;
        let mut quiet = Instant::now() + WATCH_SETTLE;
        loop {
            let now = Instant::now();
            let until = quiet.min(cap);
            if now >= until {
                break;
            }
            match self.events.recv_timeout(until - now) {
                Ok(batch) => {
                    let more = self.coalesce(batch_paths(batch)?);
                    if !more.is_empty() {
                        quiet = Instant::now() + WATCH_SETTLE;
                        paths.extend(more);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
                Err(e) => {
                    return Err(AdoptError::WatchFailed {
                        detail: e.to_string(),
                    });
                }
            }
        }
        self.check_root()?;
        Ok(self.coalesce(paths))
    }

    fn check_root(&self) -> Result<(), AdoptError> {
        if self.root.is_dir() {
            Ok(())
        } else {
            Err(AdoptError::WatchFailed {
                detail: format!("watch root removed: {}", self.root.display()),
            })
        }
    }

    pub fn drain(&self) -> Vec<PathBuf> {
        let paths: Vec<PathBuf> = self
            .events
            .try_iter()
            .flatten()
            .flat_map(|events| events.into_iter().map(|event| event.path))
            .collect();
        self.coalesce(paths)
    }

    fn coalesce(&self, paths: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut paths = coalesce_watch_paths(&[&self.root, &self.canonical_root], paths);
        paths.retain(|path| !self.ignores(path));
        paths
    }

    fn ignores(&self, path: &Path) -> bool {
        let Some(rel) = path
            .strip_prefix(&self.root)
            .or_else(|_| path.strip_prefix(&self.canonical_root))
            .ok()
        else {
            return false;
        };
        self.reports
            .iter()
            .any(|report| rel == report || dx_atomic_fs::is_staging_sibling(report, rel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect events until a path ending in `name` shows up (a sentinel
    /// written after the action under test). Kernel events arrive in order,
    /// so anything the filter wrongly kept is already in the result.
    fn wait_for(session: &WatchSession, name: &str) -> Vec<PathBuf> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        loop {
            let now = Instant::now();
            assert!(now < deadline, "sentinel {name} never arrived: {seen:?}");
            let batch = session.wait(deadline - now).expect("wait");
            seen.extend(batch);
            if seen.iter().any(|path| path.ends_with(name)) {
                return seen;
            }
        }
    }

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
            "deploy",
            "docs",
            "env",
            "generate",
            "init",
            "migrate",
            "status",
            "update",
            "version",
            "watch",
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
        assert_eq!(WATCH_DEBOUNCE_MS, 200);
    }

    #[test]
    fn watch_coalesces_bursts_into_a_single_trigger() {
        let first = PathBuf::from("/tmp/ws/src/main.rs");
        let second = PathBuf::from("/tmp/ws/src/lib.rs");
        let trigger = coalesce_watch_paths(
            &[Path::new("/tmp/ws")],
            vec![first.clone(), first.clone(), second.clone(), first.clone()],
        );
        assert_eq!(trigger, vec![second, first]);
    }

    #[test]
    fn watch_ignores_frozen_outputs_and_overlays() {
        assert!(!should_watch_path(Path::new("bazel-bin/a.rs")));
        assert!(!should_watch_path(Path::new(
            "bazel-out/k8-fastbuild/bin/a.rs"
        )));
        assert!(!should_watch_path(Path::new(".dx/current")));
        assert!(!should_watch_path(Path::new(".dx/bin/dx")));
        assert!(!should_watch_path(Path::new(".git/index")));
        assert!(!should_watch_path(Path::new(".git/refs/heads/main")));
        assert!(should_watch_path(Path::new("")));
        assert!(should_watch_path(Path::new("dx.local.toml")));
        assert!(should_watch_path(Path::new("dx.toml")));
        assert!(should_watch_path(Path::new("src/main.rs")));
        assert!(should_watch_path(Path::new("BUILD.bazel")));
        assert!(should_watch_path(Path::new(".github/workflows/ci.yml")));
    }

    #[test]
    fn watch_coalesce_drops_ignored_paths() {
        let trigger = coalesce_watch_paths(
            &[Path::new("/tmp/ws")],
            vec![
                PathBuf::from("/tmp/ws/bazel-bin/a.rs"),
                PathBuf::from("/tmp/ws/.dx/current"),
                PathBuf::from("/tmp/ws/.git/index"),
                PathBuf::from("/tmp/ws/src/main.rs"),
            ],
        );
        assert_eq!(trigger, vec![PathBuf::from("/tmp/ws/src/main.rs")]);
    }

    #[test]
    fn watch_keeps_changes_under_a_bazel_named_parent() {
        let root = Path::new("/tmp/bazel-x/ws");
        let trigger = coalesce_watch_paths(
            &[root],
            vec![
                PathBuf::from("/tmp/bazel-x/ws/src/a.rs"),
                PathBuf::from("/tmp/bazel-x/ws/.git/index"),
                PathBuf::from("/tmp/bazel-x/ws/bazel-out/x"),
                PathBuf::from("/tmp/other/a.rs"),
            ],
        );
        assert_eq!(trigger, vec![PathBuf::from("/tmp/bazel-x/ws/src/a.rs")]);
    }

    #[test]
    fn watch_keeps_changes_under_a_dx_named_parent() {
        let root = Path::new("/home/u/.dx/ws");
        let trigger = coalesce_watch_paths(
            &[root],
            vec![
                PathBuf::from("/home/u/.dx/ws/BUILD.bazel"),
                PathBuf::from("/home/u/.dx/ws/.dx/current"),
            ],
        );
        assert_eq!(trigger, vec![PathBuf::from("/home/u/.dx/ws/BUILD.bazel")]);
    }

    #[test]
    fn watch_matches_events_against_the_canonical_root() {
        let trigger = coalesce_watch_paths(
            &[Path::new("/var/ws"), Path::new("/private/var/ws")],
            vec![
                PathBuf::from("/private/var/ws/src/a.rs"),
                PathBuf::from("/private/var/ws/.git/index"),
            ],
        );
        assert_eq!(trigger, vec![PathBuf::from("/private/var/ws/src/a.rs")]);
    }

    #[test]
    fn session_observes_writes_made_while_it_lives() {
        let scratch = dx_test_scratch::scratch("dx-watch-session-");
        let root = scratch.path().to_path_buf();
        let session = WatchSession::start(&root, &[]).expect("session starts");
        let writer = root.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let _ = std::fs::write(writer.join("during.txt"), "edit");
        });
        std::thread::sleep(Duration::from_millis(600));
        let trigger = session.wait(Duration::from_secs(5)).expect("wait");
        assert!(
            trigger.iter().any(|path| path.ends_with("during.txt")),
            "edit during execution must be seen: {trigger:?}"
        );
    }

    #[test]
    fn session_coalesces_a_burst_into_one_trigger() {
        let scratch = dx_test_scratch::scratch("dx-watch-burst-");
        let root = scratch.path().to_path_buf();
        let session = WatchSession::start(&root, &[]).expect("session starts");
        for name in ["one.txt", "two.txt", "three.txt"] {
            std::fs::write(root.join(name), "burst").expect("write burst file");
        }
        let trigger = session.wait(Duration::from_secs(5)).expect("wait");
        for name in ["one.txt", "two.txt", "three.txt"] {
            assert!(
                trigger.iter().any(|path| path.ends_with(name)),
                "burst file {name} must be reported: {trigger:?}"
            );
        }
        std::fs::write(root.join("sentinel.txt"), "done").expect("sentinel");
        let rest = wait_for(&session, "sentinel.txt");
        for name in ["one.txt", "two.txt", "three.txt"] {
            assert!(
                !rest.iter().any(|path| path.ends_with(name)),
                "one wait must consume the whole burst: {rest:?}"
            );
        }
    }

    #[test]
    fn session_merges_a_burst_split_across_debounce_batches() {
        let scratch = dx_test_scratch::scratch("dx-watch-split-");
        let root = scratch.path().to_path_buf();
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::write(root.join("a.txt"), "first").expect("first write");
        std::thread::sleep(Duration::from_millis(120));
        std::fs::write(root.join("b.txt"), "second").expect("second write");
        let trigger = session.wait(Duration::from_secs(5)).expect("wait");
        for name in ["a.txt", "b.txt"] {
            assert!(
                trigger.iter().any(|path| path.ends_with(name)),
                "{name} must be in the one trigger: {trigger:?}"
            );
        }
        std::fs::write(root.join("sentinel.txt"), "done").expect("sentinel");
        let rest = wait_for(&session, "sentinel.txt");
        for name in ["a.txt", "b.txt"] {
            assert!(
                !rest.iter().any(|path| path.ends_with(name)),
                "the split burst is one trigger: {rest:?}"
            );
        }
    }

    #[test]
    fn session_reports_rename_delete_and_config_changes() {
        let scratch = dx_test_scratch::scratch("dx-watch-mutations-");
        let root = scratch.path().to_path_buf();
        std::fs::write(root.join("before.txt"), "renamed").expect("seed file");
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::rename(root.join("before.txt"), root.join("after.txt")).expect("rename");
        let trigger = session.wait(Duration::from_secs(5)).expect("rename wait");
        assert!(
            trigger.iter().any(|path| path.ends_with("after.txt")),
            "rename target must be reported: {trigger:?}"
        );
        std::fs::remove_file(root.join("after.txt")).expect("delete");
        let trigger = session.wait(Duration::from_secs(5)).expect("delete wait");
        assert!(
            trigger.iter().any(|path| path.ends_with("after.txt")),
            "deleted path must be reported: {trigger:?}"
        );
        std::fs::write(root.join("BUILD.bazel"), "# config").expect("config edit");
        let trigger = session.wait(Duration::from_secs(5)).expect("config wait");
        assert!(
            trigger.iter().any(|path| path.ends_with("BUILD.bazel")),
            "config change must trigger a rerun: {trigger:?}"
        );
    }

    #[test]
    fn reads_never_count_as_changes() {
        use notify::event::{
            AccessKind, AccessMode, CreateKind, EventKind, MetadataKind, ModifyKind, RemoveKind,
            RenameMode,
        };
        for kind in [
            EventKind::Access(AccessKind::Open(AccessMode::Any)),
            EventKind::Access(AccessKind::Close(AccessMode::Read)),
            EventKind::Access(AccessKind::Read),
        ] {
            assert!(!is_change(&kind), "{kind:?}");
        }
        for kind in [
            EventKind::Access(AccessKind::Close(AccessMode::Write)),
            EventKind::Create(CreateKind::File),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
            EventKind::Remove(RemoveKind::File),
        ] {
            assert!(is_change(&kind), "{kind:?}");
        }
    }

    #[test]
    fn only_a_plain_modify_of_directories_is_a_touch() {
        use notify::event::{CreateKind, DataChange, EventKind, ModifyKind};
        let scratch = dx_test_scratch::scratch("dx-watch-touch-");
        let dir = scratch.path().to_path_buf();
        let file = dir.join("main.rs");
        std::fs::write(&file, "fn main() {}").expect("source");
        let event = |kind, path: &Path| notify::Event::new(kind).add_path(path.to_path_buf());
        let modify = EventKind::Modify(ModifyKind::Any);
        assert!(is_directory_touch(&event(modify, &dir)));
        assert!(!is_directory_touch(&event(modify, &file)));
        assert!(!is_directory_touch(&notify::Event::new(modify)));
        assert!(!is_directory_touch(&event(
            EventKind::Create(CreateKind::Folder),
            &dir
        )));
        assert!(!is_directory_touch(&event(
            EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            &dir
        )));
    }

    #[test]
    fn session_ignores_reads_of_watched_files() {
        let scratch = dx_test_scratch::scratch("dx-watch-reads-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("src")).expect("source dir");
        std::fs::write(root.join("src/main.rs"), "fn main() {}").expect("source");
        let _ = std::fs::read(root.join("src/main.rs")).expect("warm read");
        let _ = std::fs::read_dir(root.join("src"))
            .expect("warm list")
            .count();
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::write(root.join("warm.txt"), "warm").expect("warm");
        let _ = wait_for(&session, "warm.txt");
        let _ = std::fs::read(root.join("src/main.rs")).expect("read");
        let _ = std::fs::read_dir(root.join("src")).expect("list").count();
        std::fs::write(root.join("sentinel.txt"), "done").expect("sentinel");
        let changes = wait_for(&session, "sentinel.txt");
        assert!(
            !changes
                .iter()
                .any(|path| path.components().any(|part| part.as_os_str() == "src")),
            "reading is not a change: {changes:?}"
        );
    }

    #[test]
    fn session_ignores_generated_outputs() {
        let scratch = dx_test_scratch::scratch("dx-watch-ignored-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("bazel-bin")).expect("generated dir");
        std::fs::create_dir_all(root.join(".dx")).expect("state dir");
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::write(root.join("bazel-bin/output.rs"), "generated").expect("generated write");
        std::fs::write(root.join(".dx/current"), "state").expect("state write");
        std::fs::write(root.join("sentinel.txt"), "done").expect("sentinel");
        let seen = wait_for(&session, "sentinel.txt");
        assert!(
            !seen.iter().any(|path| path
                .components()
                .any(|part| part.as_os_str() == "bazel-bin" || part.as_os_str() == ".dx")),
            "generated outputs must never trigger a rerun: {seen:?}"
        );
    }

    #[test]
    fn session_watches_a_root_under_a_bazel_named_parent() {
        let scratch = dx_test_scratch::scratch("dx-watch-bazel-parent-");
        let root = scratch.path().join("bazel-x").join("ws");
        std::fs::create_dir_all(root.join("src")).expect("source dir");
        std::fs::create_dir_all(root.join(".git")).expect("git dir");
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::write(root.join(".git/index"), "index").expect("git write");
        std::fs::write(root.join("src/a.rs"), "fn a() {}").expect("source write");
        let trigger = wait_for(&session, "a.rs");
        assert!(
            trigger.iter().any(|path| path.ends_with("src/a.rs")),
            "source edit under a bazel-named parent must be reported: {trigger:?}"
        );
        assert!(
            !trigger.iter().any(|path| path.ends_with(".git/index")),
            "{trigger:?}"
        );
    }

    #[test]
    fn session_fails_once_its_root_is_removed() {
        let scratch = dx_test_scratch::scratch("dx-watch-root-gone-");
        let root = scratch.path().join("ws");
        std::fs::create_dir_all(&root).expect("watch root");
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::remove_dir_all(&root).expect("remove root");
        let error = session
            .wait(Duration::from_secs(5))
            .expect_err("a removed root is an observer failure");
        assert!(
            matches!(&error, AdoptError::WatchFailed { detail } if detail.contains("watch root removed")),
            "{error:?}"
        );
    }

    #[test]
    fn session_ignores_report_files_and_their_staging_siblings() {
        let scratch = dx_test_scratch::scratch("dx-watch-reports-");
        let root = scratch.path().to_path_buf();
        let reports = [root.join("out.sarif"), root.join("reports/lint.sarif")];
        std::fs::create_dir_all(root.join("reports")).expect("reports dir");
        let session = WatchSession::start(&root, &reports).expect("session starts");
        for report in &reports {
            dx_atomic_fs::write_atomic(report, b"report").expect("write report");
        }
        std::fs::write(root.join("src.txt"), "edit").expect("write src");
        let trigger = wait_for(&session, "src.txt");
        for report in &reports {
            let rel = report.strip_prefix(&root).expect("report under root");
            let name = Path::new(rel.file_name().expect("report name"));
            assert!(
                !trigger.iter().any(|path| path.ends_with(rel)
                    || path
                        .file_name()
                        .is_some_and(|file| dx_atomic_fs::is_staging_sibling(
                            name,
                            Path::new(file)
                        ))),
                "{trigger:?}"
            );
        }
        assert!(
            trigger.iter().any(|path| path.ends_with("src.txt")),
            "{trigger:?}"
        );
    }

    #[test]
    fn session_ignores_reports_only_by_workspace_relative_path() {
        let scratch = dx_test_scratch::scratch("dx-watch-reports-rel-");
        let root = scratch.path().to_path_buf();
        std::fs::create_dir_all(root.join("sub")).expect("mkdir");
        let session =
            WatchSession::start(&root, &[root.join("out.sarif")]).expect("session starts");
        std::fs::write(root.join("sub/out.sarif"), "x").expect("write");
        let trigger = session.wait(Duration::from_secs(5)).expect("wait");
        assert!(
            trigger.iter().any(|path| path.ends_with("sub/out.sarif")),
            "{trigger:?}"
        );
        let _ = session.drain();
        std::fs::write(root.join(".tmpabc"), "x").expect("write");
        let trigger = session.wait(Duration::from_secs(5)).expect("wait");
        assert!(
            trigger.iter().any(|path| path.ends_with(".tmpabc")),
            "{trigger:?}"
        );
    }

    #[test]
    fn session_start_on_a_missing_root_fails() {
        let scratch = dx_test_scratch::scratch("dx-watch-missing-");
        let missing = scratch.path().join("missing");
        assert!(
            matches!(
                WatchSession::start(&missing, &[]),
                Err(AdoptError::WatchSpawn { .. })
            ),
            "a missing watch root must fail to start"
        );
    }

    #[test]
    fn session_reports_created_files_and_times_out_when_idle() {
        let scratch = dx_test_scratch::scratch("dx-adopt-watch-");
        let root = scratch.path().to_path_buf();
        let session = WatchSession::start(&root, &[]).expect("session starts");
        std::fs::write(root.join("trigger.txt"), "change").expect("write trigger");
        let trigger = session.wait(Duration::from_secs(5)).expect("watch create");
        assert!(
            trigger.iter().any(|path| path.ends_with("trigger.txt")),
            "created file must trigger a rebuild: {trigger:?}"
        );
        drop(session);
        let idle = WatchSession::start(&root, &[])
            .expect("idle session starts")
            .wait(Duration::from_millis(300))
            .expect("watch idle");
        assert!(idle.is_empty(), "idle watch must time out empty: {idle:?}");
    }
}
