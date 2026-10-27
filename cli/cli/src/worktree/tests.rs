use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use super::host::{same_path, DiskSpace, Host, Server};
use super::ops::{self, AddRequest, State};
use super::pool::{Pool, Who};

const DAY: u64 = 24 * 60 * 60;
const GIB: u64 = 1024 * 1024 * 1024;
const LOW: DiskSpace = DiskSpace {
    free: 5 * GIB,
    total: 400 * GIB,
};
const OK: DiskSpace = DiskSpace {
    free: 50 * GIB,
    total: 400 * GIB,
};

struct FakeHost {
    clock: AtomicU64,
    alive: Mutex<BTreeSet<u32>>,
    shutdowns: Mutex<Vec<PathBuf>>,
    space: Mutex<Vec<DiskSpace>>,
    disks: Mutex<Vec<(PathBuf, DiskSpace)>>,
    roots: Mutex<Vec<PathBuf>>,
    servers: Mutex<Vec<Server>>,
    disk_calls: Mutex<Vec<PathBuf>>,
    root_scans: AtomicU64,
    disk_cache: Mutex<Option<PathBuf>>,
    stop_after: Mutex<Option<(usize, i32)>>,
    stubborn: Mutex<BTreeSet<u32>>,
    stuck: Mutex<BTreeSet<PathBuf>>,
    nested: Mutex<Option<PathBuf>>,
}

impl FakeHost {
    fn new() -> FakeHost {
        FakeHost {
            clock: AtomicU64::new(1_000_000),
            alive: Mutex::new(BTreeSet::new()),
            shutdowns: Mutex::new(Vec::new()),
            space: Mutex::new(Vec::new()),
            disks: Mutex::new(Vec::new()),
            roots: Mutex::new(Vec::new()),
            servers: Mutex::new(Vec::new()),
            disk_calls: Mutex::new(Vec::new()),
            root_scans: AtomicU64::new(0),
            disk_cache: Mutex::new(None),
            stop_after: Mutex::new(None),
            stubborn: Mutex::new(BTreeSet::new()),
            stuck: Mutex::new(BTreeSet::new()),
            nested: Mutex::new(None),
        }
    }

    fn serve(&self, pid: u32, workspace: &Path) {
        self.servers.lock().expect("servers").push(Server {
            pid,
            rss_bytes: 0,
            workspace: Some(workspace.to_path_buf()),
            output_base: None,
            output_user_root: None,
        });
    }

    fn stubborn(&self, pid: u32) {
        self.stubborn.lock().expect("stubborn").insert(pid);
    }

    fn shutdown_paths(&self) -> Vec<PathBuf> {
        self.shutdowns.lock().expect("shutdowns").clone()
    }

    /// Makes `remove_tree` leave `path` on disk and report failure.
    fn stick(&self, path: &Path) {
        self.stuck.lock().expect("stuck").insert(path.to_path_buf());
    }

    /// Lets `remove_tree` delete every path again.
    fn unstick(&self) {
        self.stuck.lock().expect("stuck").clear();
    }

    fn stop_after(&self, count: usize, signo: i32) {
        *self.stop_after.lock().expect("stop") = Some((count, signo));
    }

    fn advance(&self, seconds: u64) {
        self.clock.fetch_add(seconds, Ordering::SeqCst);
    }

    fn live(&self, pid: u32) {
        self.alive.lock().expect("alive").insert(pid);
    }

    fn disk_paths(&self) -> Vec<PathBuf> {
        self.disk_calls.lock().expect("disk").clone()
    }

    fn script_space(&self, space: &[DiskSpace]) {
        *self.space.lock().expect("space") = space.to_vec();
    }

    fn script_disk(&self, prefix: &Path, space: DiskSpace) {
        self.disks
            .lock()
            .expect("disks")
            .push((prefix.to_path_buf(), space));
    }

    fn shutdown_count(&self) -> usize {
        self.shutdowns.lock().expect("shutdowns").len()
    }
}

impl Host for FakeHost {
    fn now(&self) -> u64 {
        self.clock.load(Ordering::SeqCst)
    }

    fn process_alive(&self, pid: u32) -> bool {
        self.alive.lock().expect("alive").contains(&pid)
    }

    fn disk_space(&self, path: &Path) -> Option<DiskSpace> {
        let scripted = self
            .disks
            .lock()
            .expect("disks")
            .iter()
            .filter(|(prefix, _)| path.starts_with(prefix))
            .max_by_key(|(prefix, _)| prefix.components().count())
            .map(|(_, space)| *space);
        if scripted.is_some() {
            return scripted;
        }
        let mut space = self.space.lock().expect("space");
        let first = space.first().copied();
        if space.len() > 1 {
            space.remove(0);
        }
        first
    }

    fn servers(&self) -> Vec<Server> {
        self.servers.lock().expect("servers").clone()
    }

    fn output_roots(&self) -> Vec<PathBuf> {
        self.root_scans.fetch_add(1, Ordering::SeqCst);
        self.roots.lock().expect("roots").clone()
    }

    fn bazel_shutdown(&self, workspace: &Path) {
        self.shutdowns
            .lock()
            .expect("shutdowns")
            .push(workspace.to_path_buf());
        let stubborn = self.stubborn.lock().expect("stubborn").clone();
        self.servers.lock().expect("servers").retain(|server| {
            stubborn.contains(&server.pid)
                || !server
                    .workspace
                    .as_deref()
                    .is_some_and(|dir| same_path(dir, workspace))
        });
    }

    fn disk_bytes(&self, path: &Path) -> u64 {
        self.disk_calls
            .lock()
            .expect("disk")
            .push(path.to_path_buf());
        4096
    }

    fn disk_cache(&self) -> Option<PathBuf> {
        self.disk_cache.lock().expect("disk cache").clone()
    }

    fn remove_tree(&self, path: &Path) -> bool {
        if self.stuck.lock().expect("stuck").contains(path) {
            // `git worktree remove` may already have deleted it; a stuck tree is still on disk.
            std::fs::create_dir_all(path).expect("stuck path");
            return false;
        }
        let _ = std::fs::remove_dir_all(path);
        !path.exists()
    }

    fn nested_root(&self) -> Option<PathBuf> {
        self.nested.lock().expect("nested").clone()
    }

    fn interrupted(&self) -> Option<i32> {
        let stop = *self.stop_after.lock().expect("stop");
        stop.and_then(|(count, signo)| (self.disk_paths().len() >= count).then_some(signo))
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .current_dir(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Fixture {
    _scratch: tempfile::TempDir,
    main: PathBuf,
    cache: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let scratch = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(scratch.path()).expect("canonical");
        let origin = root.join("origin.git");
        let main = root.join("repo");
        let cache = root.join("cache");
        std::fs::create_dir_all(&origin).expect("origin dir");
        git(
            &origin,
            &["init", "--bare", "--quiet", "--initial-branch=main"],
        );
        std::fs::create_dir_all(&main).expect("main dir");
        git(&main, &["init", "--quiet", "--initial-branch=main"]);
        std::fs::write(main.join("a.txt"), "one\n").expect("write");
        std::fs::write(main.join("MODULE.bazel.lock"), "{}\n").expect("lock");
        git(&main, &["add", "."]);
        git(&main, &["commit", "--quiet", "-m", "first"]);
        git(
            &main,
            &["remote", "add", "origin", &origin.to_string_lossy()],
        );
        git(&main, &["push", "--quiet", "-u", "origin", "main"]);
        git(&main, &["remote", "set-head", "origin", "main"]);
        Fixture {
            _scratch: scratch,
            main,
            cache,
        }
    }

    fn submodule() -> Fixture {
        let fixture = Fixture::new();
        let root = fixture.cache.parent().expect("root").to_path_buf();
        let outer = root.join("super");
        std::fs::create_dir_all(&outer).expect("super dir");
        git(&outer, &["init", "--quiet", "--initial-branch=main"]);
        git(
            &outer,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "--quiet",
                "add",
                &root.join("origin.git").to_string_lossy(),
                "mod",
            ],
        );
        git(&outer, &["commit", "--quiet", "-m", "add mod"]);
        let main = std::fs::canonicalize(outer.join("mod")).expect("canonical mod");
        git(&main, &["checkout", "--quiet", "main"]);
        git(&main, &["remote", "set-head", "origin", "main"]);
        Fixture { main, ..fixture }
    }

    fn separate() -> Fixture {
        let fixture = Fixture::new();
        let root = fixture.cache.parent().expect("root").to_path_buf();
        let sep = root.join("sep");
        git(
            &root,
            &[
                "clone",
                "--quiet",
                &format!("--separate-git-dir={}", root.join("store.git").display()),
                &root.join("origin.git").to_string_lossy(),
                &sep.to_string_lossy(),
            ],
        );
        let main = std::fs::canonicalize(&sep).expect("canonical sep");
        git(&main, &["remote", "set-head", "origin", "main"]);
        Fixture { main, ..fixture }
    }

    #[cfg(unix)]
    fn linked() -> Fixture {
        let fixture = Fixture::new();
        let real = fixture.cache.with_file_name("real-cache");
        std::fs::create_dir_all(&real).expect("real cache");
        std::os::unix::fs::symlink(&real, &fixture.cache).expect("cache link");
        fixture
    }

    fn pool(&self) -> Pool {
        Pool::open(&self.main, &self.cache).expect("pool opens")
    }

    fn clone_origin(&self) -> PathBuf {
        let root = self.cache.parent().expect("parent");
        let clone = root.join("other");
        git(
            root,
            &[
                "clone",
                "--quiet",
                &root.join("origin.git").to_string_lossy(),
                &clone.to_string_lossy(),
            ],
        );
        clone
    }
}

fn who(session: &str, pid: u32) -> Who {
    Who {
        session: Some(session.to_owned()),
        agent: None,
        pid,
    }
}

fn agent(session: &str, agent: &str, pid: u32) -> Who {
    Who {
        agent: Some(agent.to_owned()),
        ..who(session, pid)
    }
}

fn create(branch: &str) -> AddRequest {
    AddRequest {
        branch: branch.to_owned(),
        create: Some("origin/main".to_owned()),
        force: false,
        unclaimed: false,
    }
}

fn existing(branch: &str) -> AddRequest {
    AddRequest {
        branch: branch.to_owned(),
        create: None,
        force: false,
        unclaimed: false,
    }
}

fn forced(branch: &str) -> AddRequest {
    AddRequest {
        force: true,
        ..existing(branch)
    }
}

#[test]
fn add_leases_a_slot_outside_the_repo_and_checks_out_the_branch() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    assert_eq!(added.slot, 1);
    assert!(!added.reused);
    assert_eq!(added.base.as_deref(), Some("origin/main"));
    assert!(added.path.starts_with(&fixture.cache));
    assert!(!added.path.starts_with(&fixture.main));
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "feature/a");
    assert!(added.path.join("a.txt").is_file());
    let listing = ops::list(&pool, &host, false).expect("list");
    let row = listing
        .rows
        .iter()
        .find(|row| row.slot == Some(1))
        .expect("slot row");
    assert_eq!(row.state, State::Held);
    assert_eq!(row.branch.as_deref(), Some("feature/a"));
    assert_eq!(ops::path_of(&pool, "feature/a").expect("path"), added.path);
}

fn assert_leases_a_slot(fixture: &Fixture) {
    let pool = fixture.pool();
    assert_eq!(pool.main, fixture.main);
    let host = FakeHost::new();
    host.live(10);
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    assert_eq!(added.slot, 1);
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "feature/a");
    let listing = ops::list(&pool, &host, false).expect("list");
    assert!(
        listing.rows.iter().all(|row| row.state != State::External),
        "{:?}",
        listing.rows
    );
    assert_eq!(
        ops::path_of(&pool, "main").expect("main path"),
        fixture.main
    );
    let reopened = Pool::open(&added.path, &fixture.cache).expect("open from slot");
    assert_eq!(reopened.main, pool.main);
    assert_eq!(reopened.base, pool.base);
}

#[test]
fn add_leases_a_slot_in_a_submodule_checkout() {
    assert_leases_a_slot(&Fixture::submodule());
}

#[test]
fn add_leases_a_slot_in_a_separate_git_dir_clone() {
    assert_leases_a_slot(&Fixture::separate());
}

#[test]
fn open_refuses_a_bare_repository() {
    let fixture = Fixture::new();
    let origin = fixture.main.with_file_name("origin.git");
    let refused = Pool::open(&origin, &fixture.cache).expect_err("bare");
    assert_eq!(refused.code, "bare_repository");
    assert!(
        refused.message.contains("without --bare"),
        "{}",
        refused.message
    );
}

#[test]
fn hook_parse_refusal_names_stdin() {
    let refused = super::hook::parse("").expect_err("empty");
    assert_eq!(refused.code, "bad_hook_input");
    assert!(refused.message.contains("on stdin"), "{}", refused.message);
}

#[test]
fn hook_event_names_are_stable() {
    let events = super::hook::HookKind::ALL.map(super::hook::HookKind::event);
    assert_eq!(
        events,
        [
            "WorktreeCreate",
            "WorktreeRemove",
            "SubagentStop",
            "PreToolUse"
        ]
    );
}

#[test]
fn open_refuses_a_linked_worktree_of_a_bare_repository() {
    let fixture = Fixture::new();
    let origin = fixture.main.with_file_name("origin.git");
    let linked = fixture.main.with_file_name("bare-linked");
    git(
        &origin,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "feature/bare",
            &linked.to_string_lossy(),
            "main",
        ],
    );
    let refused = Pool::open(&linked, &fixture.cache).expect_err("bare linked");
    assert_eq!(refused.code, "bare_repository");
    assert!(
        refused.message.contains("without --bare"),
        "{}",
        refused.message
    );
}

#[test]
fn open_refuses_a_directory_outside_git() {
    let outside = tempfile::tempdir().expect("tempdir");
    let cache = outside.path().join("cache");
    let refused = Pool::open(outside.path(), &cache).expect_err("not a repo");
    assert_eq!(refused.code, "not_a_repository");
    assert!(refused.usage);
    assert!(
        refused
            .message
            .ends_with("; run dx worktree from a git checkout"),
        "{}",
        refused.message
    );
}

#[test]
fn parse_checkouts_marks_bare_entries() {
    let found = super::git::parse_checkouts(
        "worktree /r/origin.git\nbare\n\nworktree /r/slot\nHEAD abc\nbranch refs/heads/a\n",
    );
    assert_eq!(found.len(), 2);
    assert!(found[0].bare);
    assert!(!found[1].bare);
    assert_eq!(found[1].branch.as_deref(), Some("a"));
}

#[test]
fn add_warns_when_origin_cannot_be_fetched() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let missing = fixture.main.parent().expect("root").join("missing.git");
    git(
        &fixture.main,
        &["remote", "set-url", "origin", &missing.to_string_lossy()],
    );
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    assert_eq!(added.warnings.len(), 1, "{:?}", added.warnings);
    assert_eq!(added.warnings[0].code, "fetch_failed");
    assert!(
        added.warnings[0]
            .message
            .starts_with("cannot fetch origin, using local refs: "),
        "{:?}",
        added.warnings
    );
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "feature/a");
}

#[test]
fn add_has_no_warnings_when_origin_fetches() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    assert!(added.warnings.is_empty(), "{:?}", added.warnings);
}

#[test]
fn re_adding_a_held_branch_does_not_fetch_origin() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    let missing = fixture.main.parent().expect("root").join("missing.git");
    git(
        &fixture.main,
        &["remote", "set-url", "origin", &missing.to_string_lossy()],
    );
    let again = ops::add(&pool, &host, &who("s1", 10), &existing("feature/a")).expect("re-add");
    assert!(again.reused);
    assert_eq!(again.path, first.path);
    assert!(again.base.is_none());
    assert!(again.warnings.is_empty(), "{:?}", again.warnings);
    let other = ops::add(&pool, &host, &who("s1", 10), &create("feature/b")).expect("add b");
    assert_eq!(other.warnings.len(), 1, "{:?}", other.warnings);
    assert_eq!(other.warnings[0].code, "fetch_failed");
}

#[test]
fn release_frees_the_slot_and_the_next_add_reuses_it_without_leftovers() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("add");
    std::fs::write(first.path.join("scratch.log"), "x").expect("scratch");
    git(
        &first.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    let released = ops::release(&pool, &host, &me, &first.path, true).expect("release");
    assert_eq!(released.slot, Some(1));
    assert_eq!(git(&first.path, &["branch", "--show-current"]), "");
    let row = ops::list(&pool, &host, false)
        .expect("list")
        .rows
        .into_iter()
        .find(|row| row.slot == Some(1))
        .expect("row");
    assert_eq!(row.state, State::Free);
    let second = ops::add(&pool, &host, &me, &create("feature/b")).expect("add b");
    assert_eq!(second.slot, 1);
    assert_eq!(second.path, first.path);
    assert!(!second.path.join("scratch.log").exists());
    assert_eq!(
        git(&second.path, &["branch", "--show-current"]),
        "feature/b"
    );
}

#[test]
fn release_refuses_uncommitted_changes_unless_forced() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let added = ops::add(&pool, &host, &me, &create("feature/a")).expect("add");
    std::fs::write(added.path.join("a.txt"), "changed\n").expect("edit");
    let refused = ops::release(&pool, &host, &me, &added.path, false).expect_err("dirty");
    assert_eq!(refused.code, "dirty_slot");
    assert!(!refused.usage);
    let forced = ops::release(&pool, &host, &me, &added.path, true).expect("forced");
    assert!(!forced.already_free);
}

#[test]
fn release_refuses_commits_that_are_on_no_remote_unless_forced() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let added = ops::add(&pool, &host, &me, &create("feature/a")).expect("add");
    std::fs::write(added.path.join("b.txt"), "new\n").expect("write");
    git(&added.path, &["add", "."]);
    git(&added.path, &["commit", "--quiet", "-m", "local"]);
    let refused = ops::release(&pool, &host, &me, &added.path, false).expect_err("unpushed");
    assert_eq!(refused.code, "unpushed_commits");
    git(
        &added.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    ops::release(&pool, &host, &me, &added.path, false).expect("pushed work releases");
}

#[test]
fn release_of_a_slot_leased_to_another_live_session_is_a_usage_error() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    let refused =
        ops::release(&pool, &host, &who("s2", 20), &added.path, false).expect_err("not yours");
    assert!(refused.usage);
    assert_eq!(refused.code, "slot_leased");
    assert!(refused.message.contains("s1"), "{}", refused.message);
    ops::release(&pool, &host, &who("s2", 20), &added.path, true).expect("force overrides");
}

#[test]
fn a_stale_lease_does_not_release_the_slot_leased_again_after_it() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("worktree-a")).expect("a");
    let snapshot = pool.record(first.slot).expect("record");
    host.advance(120);
    ops::release(&pool, &host, &me, &first.path, false).expect("release a");
    let again = ops::add(&pool, &host, &me, &create("worktree-c")).expect("c");
    assert_eq!(again.slot, first.slot);
    let shutdowns = host.shutdown_count();
    let refused = ops::release_lease(&pool, &host, &me, &snapshot).expect_err("stale");
    assert_eq!(refused.code, "lease_changed");
    assert!(!refused.usage);
    assert!(
        refused.message.contains("dx worktree list"),
        "{}",
        refused.message
    );
    let record = pool.record(first.slot).expect("record");
    assert!(record.held);
    assert_eq!(record.branch.as_deref(), Some("worktree-c"));
    assert_eq!(
        git(&again.path, &["branch", "--show-current"]),
        "worktree-c"
    );
    assert_eq!(host.shutdown_count(), shutdowns);
}

#[test]
fn another_live_agent_in_the_same_session_cannot_release_or_reuse_the_slot_without_force() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let first = agent("s1", "a1", 10);
    let second = agent("s1", "a2", 10);
    let added = ops::add(&pool, &host, &first, &create("feature/a")).expect("add");
    git(
        &added.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    let mine = ops::add(&pool, &host, &first, &existing("feature/a")).expect("mine");
    assert!(mine.reused);
    let refused = ops::release(&pool, &host, &second, &added.path, false).expect_err("not yours");
    assert_eq!(refused.code, "slot_leased");
    assert!(refused.message.contains("agent a1"), "{}", refused.message);
    let taken = ops::add(&pool, &host, &second, &existing("feature/a")).expect_err("not yours");
    assert_eq!(taken.code, "branch_held");
    assert!(taken.message.contains("--force"), "{}", taken.message);
    let main_thread = who("s1", 10);
    assert_eq!(
        ops::release(&pool, &host, &main_thread, &added.path, false)
            .expect_err("not the main thread's")
            .code,
        "slot_leased"
    );
    let other = ops::add(&pool, &host, &second, &create("feature/b")).expect("own slot");
    assert_ne!(other.slot, added.slot);
    let holders: Vec<Option<String>> = ops::list(&pool, &host, false)
        .expect("list")
        .rows
        .into_iter()
        .map(|row| row.holder)
        .collect();
    assert_eq!(
        holders,
        vec![
            Some("session s1 agent a1 pid 10".to_owned()),
            Some("session s1 agent a2 pid 10".to_owned()),
        ]
    );
    let adopted = ops::add(&pool, &host, &second, &forced("feature/a")).expect("forced");
    assert!(adopted.reused);
    assert_eq!(adopted.slot, added.slot);
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "feature/a");
    assert!(pool.record(added.slot).expect("record").held_by(&second));
    ops::release(&pool, &host, &first, &added.path, true).expect("force releases");
}

#[test]
fn a_gone_holder_does_not_let_another_session_release_before_it_is_reclaimable() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    std::fs::write(added.path.join("a.txt"), "dirty\n").expect("dirty");
    let refused =
        ops::release(&pool, &host, &who("s2", 20), &added.path, false).expect_err("not yours");
    assert_eq!(refused.code, "slot_leased");
    assert!(refused.message.contains("(gone)"), "{}", refused.message);
    git(&added.path, &["checkout", "--quiet", "--", "a.txt"]);
    ops::release(&pool, &host, &who("s2", 20), &added.path, false).expect("reclaimable");
}

#[test]
fn a_failed_switch_still_deletes_the_stale_slots_it_took_the_records_of() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let second = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    ops::release(&pool, &host, &me, &first.path, true).expect("release a");
    host.advance(31 * DAY);
    ops::release(&pool, &host, &me, &second.path, true).expect("release b");
    std::fs::remove_file(second.path.join(".git")).expect("break slot 2");
    let broken = ops::add(&pool, &host, &me, &create("feature/c"));
    assert!(broken.is_err());
    assert!(!first.path.exists(), "the stale checkout is deleted");
    assert!(pool.record(first.slot).is_none());
}

#[test]
fn a_branch_held_in_another_slot_is_an_error_but_the_same_session_gets_it_back() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    let again = ops::add(&pool, &host, &who("s1", 10), &existing("feature/a")).expect("again");
    assert!(again.reused);
    assert_eq!(again.base, None);
    assert_eq!(again.path, added.path);
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("held");
    assert_eq!(refused.code, "branch_held");
    assert!(refused.message.contains("slot 1"), "{}", refused.message);
    assert!(refused.message.contains("s1"), "{}", refused.message);
    assert!(
        refused.message.contains("pick another branch"),
        "{}",
        refused.message
    );
    assert!(refused.message.contains("--force"), "{}", refused.message);
}

#[test]
fn a_missing_branch_starts_from_the_default_base() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &existing("nope")).expect("started");
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "nope");
    assert_eq!(added.base.as_deref(), Some("origin/main"));
    assert_eq!(
        git(&added.path, &["rev-parse", "HEAD"]),
        git(&fixture.main, &["rev-parse", "origin/main"])
    );
    let unknown = AddRequest {
        create: Some("origin/nope".to_owned()),
        ..existing("fresh")
    };
    let refused = ops::add(&pool, &host, &who("s2", 20), &unknown).expect_err("unknown base");
    assert!(refused.usage);
    assert_eq!(refused.code, "unknown_ref");
    git(&fixture.main, &["branch", "other"]);
    let taken = ops::add(&pool, &host, &who("s1", 10), &create("other")).expect_err("exists");
    assert_eq!(taken.code, "branch_exists");
}

fn slot_one(listing: &ops::Listing) -> &ops::Row {
    listing
        .rows
        .iter()
        .find(|row| row.slot == Some(1))
        .expect("slot 1 is listed")
}

#[test]
fn a_dead_holder_with_a_clean_pushed_tree_is_reclaimed_at_once() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(20);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    git(
        &first.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    let listing = ops::list(&pool, &host, false).expect("list");
    assert_eq!(slot_one(&listing).state, State::Reclaimable);
    assert_eq!(slot_one(&listing).reason, None);
    assert!(slot_one(&listing)
        .holder
        .as_deref()
        .is_some_and(|holder| holder.contains("(gone)")));
    host.serve(70, &first.path);
    let second = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect("second");
    assert_eq!(second.slot, 1);
    assert!(!second.reused);
    assert_eq!(second.base, None);
    assert_eq!(
        git(&second.path, &["branch", "--show-current"]),
        "feature/a"
    );
    assert_eq!(host.shutdown_paths(), vec![first.path.clone()]);
    let after = ops::list(&pool, &host, false).expect("list");
    assert_eq!(slot_one(&after).state, State::Held);
    assert!(slot_one(&after)
        .holder
        .as_deref()
        .is_some_and(|holder| holder.contains("s2")));
}

fn run_dx_in(pool: &Pool, slot: u32) -> dx_atomic_fs::lease::SharedLease {
    dx_atomic_fs::lease::acquire_shared_at(
        &pool.busy_path(slot),
        std::time::Duration::from_secs(10),
    )
    .expect("busy lock")
}

#[test]
fn a_slot_with_a_running_dx_command_is_not_reclaimed_or_released() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(20);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    git(
        &first.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    let running = run_dx_in(&pool, 1);
    let listing = ops::list(&pool, &host, false).expect("list");
    assert_eq!(slot_one(&listing).state, State::Held);
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("busy");
    assert_eq!(refused.code, "branch_held");
    let mut forced = existing("feature/a");
    forced.force = true;
    let refused = ops::add(&pool, &host, &who("s2", 20), &forced).expect_err("busy");
    assert_eq!(refused.code, "slot_busy");
    let second = ops::add(&pool, &host, &who("s2", 20), &create("feature/b")).expect("second");
    assert_eq!(second.slot, 2);
    for force in [false, true] {
        let refused =
            ops::release(&pool, &host, &who("s2", 20), &first.path, force).expect_err("busy");
        assert_eq!(refused.code, "slot_busy");
        assert!(
            refused.message.contains("running a dx command"),
            "{}",
            refused.message
        );
    }
    assert_eq!(git(&first.path, &["branch", "--show-current"]), "feature/a");
    drop(running);
    let third = ops::add(&pool, &host, &who("s2", 20), &create("feature/c")).expect("third");
    assert_eq!(third.slot, 1);
    assert_eq!(git(&third.path, &["branch", "--show-current"]), "feature/c");
}

#[test]
fn add_skips_a_free_slot_with_a_running_dx_command() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("add");
    ops::release(&pool, &host, &me, &first.path, false).expect("release");
    let running = run_dx_in(&pool, 1);
    let second = ops::add(&pool, &host, &me, &create("feature/b")).expect("second");
    assert_eq!(second.slot, 2);
    drop(running);
    let third = ops::add(&pool, &host, &me, &create("feature/c")).expect("third");
    assert_eq!(third.slot, 1);
}

#[test]
fn a_dead_holder_with_a_dirty_tree_keeps_its_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(20);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    git(
        &first.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    std::fs::write(first.path.join("a.txt"), "dirty\n").expect("dirty");
    let listing = ops::list(&pool, &host, false).expect("list");
    assert_eq!(slot_one(&listing).state, State::Held);
    assert!(slot_one(&listing)
        .reason
        .is_some_and(|reason| reason.contains("uncommitted")));
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("held");
    assert_eq!(refused.code, "branch_held");
    assert!(
        refused.message.contains("uncommitted"),
        "{}",
        refused.message
    );
    let second = ops::add(&pool, &host, &who("s2", 20), &create("feature/b")).expect("second");
    assert_eq!(second.slot, 2);
}

#[test]
fn a_dead_holder_with_unpushed_commits_keeps_its_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(20);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    std::fs::write(first.path.join("a.txt"), "work\n").expect("write");
    git(&first.path, &["add", "a.txt"]);
    git(&first.path, &["commit", "--quiet", "-m", "work"]);
    let listing = ops::list(&pool, &host, false).expect("list");
    assert_eq!(slot_one(&listing).state, State::Held);
    assert!(slot_one(&listing)
        .reason
        .is_some_and(|reason| reason.contains("not pushed")));
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("held");
    assert_eq!(refused.code, "branch_held");
    let second = ops::add(&pool, &host, &who("s2", 20), &create("feature/b")).expect("second");
    assert_eq!(second.slot, 2);
}

#[test]
fn a_dead_holder_with_unpushed_commits_and_a_dirty_tree_reports_the_dirty_tree() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(20);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    std::fs::write(first.path.join("a.txt"), "work\n").expect("write");
    git(&first.path, &["add", "a.txt"]);
    git(&first.path, &["commit", "--quiet", "-m", "work"]);
    std::fs::write(first.path.join("a.txt"), "dirty\n").expect("dirty");
    let listing = ops::list(&pool, &host, false).expect("list");
    assert_eq!(slot_one(&listing).state, State::Held);
    assert_eq!(
        slot_one(&listing).reason,
        Some("holder gone, tree has uncommitted changes")
    );
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("held");
    assert_eq!(refused.code, "branch_held");
    assert!(
        refused
            .message
            .contains("; holder gone, tree has uncommitted changes;"),
        "{}",
        refused.message
    );
    assert!(
        !refused.message.contains("not pushed"),
        "{}",
        refused.message
    );
}

#[test]
fn a_live_holder_keeps_its_slot_after_a_day() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    host.advance(3 * DAY);
    let second = ops::add(&pool, &host, &who("s2", 20), &create("feature/b")).expect("second");
    assert_eq!(second.slot, 2);
}

#[test]
fn two_concurrent_adds_get_different_slots() {
    let fixture = Fixture::new();
    let host = FakeHost::new();
    host.live(100);
    host.live(101);
    let results: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2u32)
            .map(|n| {
                let pool = fixture.pool();
                let host = &host;
                scope.spawn(move || {
                    let who = who(&format!("s{n}"), 100 + n);
                    ops::add(&pool, host, &who, &create(&format!("feature/{n}")))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("joins"))
            .collect()
    });
    let mut slots: Vec<u32> = results
        .into_iter()
        .map(|result| result.expect("add").slot)
        .collect();
    slots.sort_unstable();
    assert_eq!(slots, vec![1, 2]);
}

#[test]
fn a_slot_removed_with_git_worktree_remove_is_forgotten_and_its_number_reused() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("add");
    git(
        &fixture.main,
        &[
            "worktree",
            "remove",
            "--force",
            "--force",
            &first.path.to_string_lossy(),
        ],
    );
    let listing = ops::list(&pool, &host, false).expect("list");
    assert!(listing.rows.iter().all(|row| row.slot.is_none()));
    let second = ops::add(&pool, &host, &me, &create("feature/b")).expect("add b");
    assert_eq!(second.slot, 1);
    assert_eq!(
        git(&second.path, &["branch", "--show-current"]),
        "feature/b"
    );
}

#[test]
fn hand_made_checkouts_are_listed_as_external() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = fixture.cache.parent().expect("parent").join("hand");
    git(
        &fixture.main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "by-hand",
            &hand.to_string_lossy(),
        ],
    );
    let listing = ops::list(&pool, &host, false).expect("list");
    let row = listing
        .rows
        .iter()
        .find(|row| row.branch.as_deref() == Some("by-hand"))
        .expect("hand row");
    assert_eq!(row.state, State::External);
    assert_eq!(row.slot, None);
    let detached = detached_checkout(&fixture, "by-hand-detached", "HEAD");
    let listing = ops::list(&pool, &host, false).expect("list");
    let row = listing
        .rows
        .iter()
        .find(|row| row.path == detached)
        .expect("detached row");
    assert_eq!(row.branch, None);
    assert_eq!(row.state, State::External);
    assert_eq!(row.slot, None);
    assert_eq!(ops::path_of(&pool, "by-hand").expect("path"), hand);
    let refused =
        ops::add(&pool, &host, &who("s1", 10), &existing("by-hand")).expect_err("checked out");
    assert_eq!(refused.code, "branch_held");
    assert!(refused.message.contains("cd "), "{}", refused.message);
    assert!(
        refused.message.ends_with("or pick another branch"),
        "{}",
        refused.message
    );
}

#[test]
fn the_slot_that_last_held_a_branch_is_picked_first() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    for path in [&a.path, &b.path] {
        let branch = git(path, &["branch", "--show-current"]);
        git(path, &["push", "--quiet", "-u", "origin", &branch]);
    }
    ops::release(&pool, &host, &me, &a.path, false).expect("release a");
    host.advance(10);
    ops::release(&pool, &host, &me, &b.path, false).expect("release b");
    let again = ops::add(&pool, &host, &me, &existing("feature/a")).expect("again");
    assert_eq!(
        again.slot, a.slot,
        "a was freed first, but it held the branch"
    );
    let other = ops::add(&pool, &host, &me, &create("feature/c")).expect("c");
    assert_eq!(other.slot, b.slot);
}

#[test]
fn low_disk_still_reuses_the_newest_free_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    host.advance(10);
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    let newest = ops::add(&pool, &host, &me, &create("feature/c")).expect("c");
    assert_eq!(newest.slot, b.slot, "most recently used first");
    ops::release(&pool, &host, &me, &newest.path, true).expect("release c");
    host.script_space(&[DiskSpace {
        free: 19 * GIB,
        total: 400 * GIB,
    }]);
    let warm = ops::add(&pool, &host, &me, &create("feature/d")).expect("d");
    assert_eq!(warm.slot, b.slot, "the warmest slot first, low disk or not");
}

fn output_base(roots: &Path, name: &str, workspace: &Path) -> PathBuf {
    let base = roots.join(name);
    std::fs::create_dir_all(base.join("external")).expect("base");
    std::fs::write(
        base.join("DO_NOT_BUILD_HERE"),
        workspace.to_string_lossy().as_bytes(),
    )
    .expect("marker");
    base
}

#[test]
fn low_disk_add_leases_the_warmest_slot_and_drops_other_free_bases_oldest_first() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let mut slots = Vec::new();
    for branch in ["feature/a", "feature/b", "feature/c", "feature/e"] {
        slots.push(ops::add(&pool, &host, &me, &create(branch)).expect("add"));
    }
    let mut bases = Vec::new();
    for slot in &slots {
        ops::release(&pool, &host, &me, &slot.path, true).expect("release");
        host.advance(10);
        bases.push(output_base(&roots, &format!("s{}", slot.slot), &slot.path));
    }
    let orphan = output_base(&roots, "orphan", &fixture.cache.join("gone"));
    host.script_space(&[LOW, LOW, LOW, LOW, OK, OK]);
    let held = ops::add(&pool, &host, &me, &create("feature/d")).expect("d");
    assert_eq!(
        held.slot, slots[3].slot,
        "most recently used free slot first"
    );
    assert!(!orphan.exists());
    assert!(!bases[0].exists());
    assert!(!bases[1].exists());
    assert!(bases[2].exists(), "reclaim stops once disk recovers");
    assert!(bases[3].exists(), "the leased slot keeps its base");
    let warnings: Vec<&ops::Warning> = held
        .warnings
        .iter()
        .filter(|warning| warning.code == "disk_low")
        .collect();
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].message,
        format!(
            "low disk: 5.0 GiB free; removed 1 orphaned output base(s) and the output bases of free slot(s) {}, {}; 50.0 GiB free now",
            slots[0].slot, slots[1].slot
        )
    );
}

const FULL: DiskSpace = DiskSpace {
    free: 512 * 1024 * 1024,
    total: 400 * GIB,
};

const TINY_FULL: DiskSpace = DiskSpace {
    free: 512 * 1024 * 1024,
    total: 2 * GIB,
};

#[test]
fn add_refuses_disk_full_after_reclaim() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    let records = pool.records().len();
    host.script_disk(&fixture.cache, FULL);
    let error = ops::add(&pool, &host, &me, &create("feature/b")).expect_err("full");
    assert_eq!(error.code, "disk_full");
    assert!(
        error.message.contains("512.0 MiB free"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("prune --apply --bases"),
        "{}",
        error.message
    );
    assert!(!base.exists(), "reclaim ran before the refusal");
    assert_eq!(pool.records().len(), records, "no slot record is left");
    assert!(!pool.slot_path(records as u32 + 1).exists());
}

#[test]
fn low_disk_add_keeps_the_base_of_the_slot_it_leases() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.script_space(&[LOW]);
    let shutdowns = host.shutdown_count();
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    assert_eq!(b.slot, a.slot);
    assert!(base.exists(), "the leased slot keeps its output base");
    assert_eq!(host.shutdown_count(), shutdowns, "its server is left alone");
    let warnings: Vec<&ops::Warning> = b
        .warnings
        .iter()
        .filter(|warning| warning.code == "disk_low")
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", b.warnings);
    assert!(
        warnings[0].message.contains("nothing to reclaim"),
        "{}",
        warnings[0].message
    );
}

#[test]
fn full_disk_add_drops_the_leased_slot_base_as_a_last_resort() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.script_space(&[FULL, FULL, FULL, FULL, OK]);
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    assert_eq!(b.slot, a.slot);
    assert!(!base.exists(), "the reused slot's base made room");
    let warnings: Vec<&ops::Warning> = b
        .warnings
        .iter()
        .filter(|warning| warning.code == "disk_low")
        .collect();
    assert_eq!(warnings.len(), 1, "{:?}", b.warnings);
    assert!(
        warnings[0].message.ends_with(&format!(
            "disk full: 512.0 MiB free; removed the output bases of slot {}, which this lease reuses; 50.0 GiB free now",
            a.slot
        )),
        "{}",
        warnings[0].message
    );
}

#[test]
fn release_stops_no_server_when_none_runs_in_the_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    assert!(host.shutdown_paths().is_empty());
}

#[test]
fn release_stops_the_server_of_its_slot_only() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    host.serve(80, &a.path);
    host.serve(81, &b.path);
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    assert_eq!(host.shutdown_paths(), vec![a.path.clone()]);
    let left: Vec<u32> = host.servers().iter().map(|server| server.pid).collect();
    assert_eq!(left, vec![81]);
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_stops_the_server_of_a_released_slot() {
    let fixture = Fixture::linked();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    host.serve(82, &physical(&a.path));
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    assert_eq!(host.shutdown_paths().len(), 1);
    assert!(host.servers().is_empty());
}

#[test]
fn low_disk_reclaim_keeps_a_free_slot_base_whose_server_will_not_stop() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let base = output_base(&roots, "a", &a.path);
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    host.serve(83, &a.path);
    host.stubborn(83);
    host.script_space(&[LOW]);
    let _ = ops::reclaim_if_low(&pool, &host, None).expect("reclaim");
    assert_eq!(host.shutdown_paths(), vec![a.path.clone()]);
    assert!(base.exists(), "a base with a running server stays");
}

#[test]
fn full_disk_add_keeps_the_reused_slot_base_while_its_server_runs() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.serve(84, &a.path);
    host.stubborn(84);
    host.script_space(&[FULL, FULL, FULL, FULL, OK]);
    let err = ops::add(&pool, &host, &me, &create("feature/b")).expect_err("full");
    assert_eq!(err.code, "disk_full");
    assert!(base.exists());
}

#[test]
fn add_reuses_own_held_branch_on_full_disk() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    host.script_disk(&fixture.cache, FULL);
    let again = ops::add(&pool, &host, &me, &create("feature/a")).expect("reuse");
    assert!(again.reused);
    assert_eq!(again.slot, a.slot);
}

#[test]
fn full_small_disk_reclaims_free_slot_base_before_refusing() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.script_disk(&fixture.cache, TINY_FULL);
    let error = ops::add(&pool, &host, &me, &create("feature/b")).expect_err("full");
    assert_eq!(error.code, "disk_full");
    assert!(!base.exists(), "reclaim ran on a full disk over 10% free");
}

#[test]
fn full_small_disk_with_nothing_to_reclaim_refuses() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    host.script_disk(&fixture.cache, TINY_FULL);
    let error = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect_err("full");
    assert_eq!(error.code, "disk_full");
}

#[test]
fn prune_bases_lists_free_bases_lru_on_plenty_of_disk() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    host.script_disk(&fixture.cache, OK);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    let held = ops::add(&pool, &host, &me, &create("feature/c")).expect("c");
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    host.advance(10);
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    let base_a = output_base(&roots, "a", &a.path);
    let base_b = output_base(&roots, "b", &b.path);
    let base_held = output_base(&roots, "c", &held.path);
    let plain = ops::prune(&pool, &host, false, false).expect("plain");
    assert!(plain.plan.bases.is_empty(), "no bases without --bases");
    let dry = ops::prune(&pool, &host, false, true).expect("dry");
    let dirs: Vec<&PathBuf> = dry.plan.bases.iter().map(|base| &base.dir).collect();
    assert_eq!(dirs, vec![&base_b, &base_a], "least recently used first");
    assert!(
        base_a.exists() && base_b.exists(),
        "a dry run deletes nothing"
    );
    let applied = ops::prune(&pool, &host, true, true).expect("apply");
    assert!(applied.skipped_bases.is_empty());
    assert!(!base_a.exists() && !base_b.exists());
    assert!(base_held.exists(), "a leased slot keeps its base");
}

#[test]
fn prune_bases_apply_skips_busy_and_server_slots() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    host.script_disk(&fixture.cache, OK);
    let me = who("s1", 10);
    let slot = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let base = output_base(&roots, "aaa", &slot.path);
    ops::release(&pool, &host, &me, &slot.path, true).expect("release");
    host.servers.lock().expect("servers").push(Server {
        pid: 42,
        rss_bytes: 0,
        workspace: Some(slot.path.clone()),
        output_base: None,
        output_user_root: None,
    });
    host.stubborn(42);
    let pruned = ops::prune(&pool, &host, true, true).expect("prune");
    assert_eq!(pruned.skipped_bases.len(), 1);
    assert_eq!(pruned.skipped_bases[0].reason, "server");
    assert!(base.exists());
}

#[test]
fn prune_bases_does_not_double_count_stale_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    host.script_disk(&fixture.cache, OK);
    let me = who("s1", 10);
    let slot = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    output_base(&roots, "aaa", &slot.path);
    ops::release(&pool, &host, &me, &slot.path, true).expect("release");
    host.advance(31 * 24 * 60 * 60);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::OnDemand).expect("plan");
    assert_eq!(plan.slots.len(), 1);
    assert!(plan.bases.is_empty(), "{:?}", plan.bases);
}

#[test]
fn an_add_that_fails_before_picking_a_slot_drops_nothing() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.script_space(&[LOW, LOW, OK]);
    let request = AddRequest {
        create: Some("origin/no-such-branch".to_owned()),
        ..create("feature/b")
    };
    assert!(ops::add(&pool, &host, &me, &request).is_err());
    assert!(base.exists(), "no slot was picked, so nothing is reclaimed");
}

#[test]
fn reclaim_keeps_the_workspace_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    host.advance(10);
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    let base_a = output_base(&roots, "a", &a.path);
    let base_b = output_base(&roots, "b", &b.path);
    host.script_space(&[LOW, LOW, LOW, OK]);
    let message =
        super::reclaim_for_build_with(&a.path, &fixture.cache, &host).expect("a reclaim message");
    assert!(base_a.exists(), "the workspace being built keeps its base");
    assert!(!base_b.exists());
    assert_eq!(
        message,
        format!(
            "low disk: 5.0 GiB free; removed the output bases of free slot(s) {}; 50.0 GiB free now",
            b.slot
        )
    );
}

#[test]
fn pool_find_creates_nothing() {
    let fixture = Fixture::new();
    assert!(Pool::find(&fixture.main, &fixture.cache).is_none());
    assert!(!fixture.cache.exists());
    let pool = fixture.pool();
    let found = Pool::find(&fixture.main, &fixture.cache).expect("existing pool");
    assert_eq!(found.base, pool.base);
}

#[test]
fn reclaim_for_build_skips_plenty_or_unknown_disk() {
    let fixture = Fixture::new();
    let _pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let orphan = output_base(&roots, "orphan", &fixture.cache.join("gone"));
    host.script_space(&[OK]);
    assert_eq!(
        super::reclaim_for_build_with(&fixture.main, &fixture.cache, &host),
        None
    );
    host.script_space(&[]);
    assert_eq!(
        super::reclaim_for_build_with(&fixture.main, &fixture.cache, &host),
        None
    );
    assert_eq!(host.root_scans.load(Ordering::SeqCst), 1);
    assert!(orphan.exists());
}

#[test]
fn reclaim_for_build_without_a_pool_creates_none() {
    let fixture = Fixture::new();
    let host = FakeHost::new();
    host.script_space(&[LOW]);
    assert_eq!(
        super::reclaim_for_build_with(&fixture.main, &fixture.cache, &host),
        None
    );
    assert!(!fixture.cache.exists());
    assert_eq!(host.root_scans.load(Ordering::SeqCst), 0);
}

#[test]
fn reclaim_for_build_is_quiet_when_nothing_is_reclaimed() {
    let fixture = Fixture::new();
    let _pool = fixture.pool();
    let host = FakeHost::new();
    host.script_space(&[LOW]);
    assert_eq!(
        super::reclaim_for_build_with(&fixture.main, &fixture.cache, &host),
        None
    );
    assert_eq!(host.root_scans.load(Ordering::SeqCst), 1);
}

#[test]
fn reclaim_for_build_reports_a_removed_free_base() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.script_space(&[LOW, LOW, LOW, OK]);
    assert_eq!(
        super::reclaim_for_build_with(&fixture.main, &fixture.cache, &host),
        Some(format!(
            "low disk: 5.0 GiB free; removed the output bases of free slot(s) {}; 50.0 GiB free now",
            a.slot
        ))
    );
    assert!(!base.exists());
}

#[test]
fn plenty_of_disk_reclaims_nothing_and_takes_no_lock() {
    let fixture = Fixture::new();
    let mut pool = fixture.pool();
    pool.lock_wait = std::time::Duration::from_millis(200);
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let orphan = output_base(&roots, "orphan", &fixture.cache.join("gone"));
    let _held = pool.lock().expect("lock");
    host.script_space(&[OK]);
    assert_eq!(
        ops::reclaim_if_low(&pool, &host, None).expect("plenty"),
        None
    );
    host.script_space(&[]);
    assert_eq!(
        ops::reclaim_if_low(&pool, &host, None).expect("unknown"),
        None,
        "an unknown free space is not low"
    );
    host.script_space(&[DiskSpace {
        free: 10 * GIB,
        total: 50 * GIB,
    }]);
    assert_eq!(
        ops::reclaim_if_low(&pool, &host, None).expect("small disk"),
        None,
        "under 20 GiB but over 10% free is not low"
    );
    assert_eq!(host.root_scans.load(Ordering::SeqCst), 2);
    assert!(orphan.exists());
}

#[test]
fn low_disk_keeps_orphan_bases_a_server_still_uses() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let orphan = output_base(&roots, "orphan", &fixture.cache.join("gone"));
    host.servers.lock().expect("servers").push(Server {
        pid: 42,
        rss_bytes: 0,
        workspace: Some(fixture.cache.join("gone")),
        output_base: Some(orphan.clone()),
        output_user_root: None,
    });
    host.script_space(&[LOW]);
    let reclaimed = ops::reclaim_if_low(&pool, &host, None)
        .expect("reclaim")
        .expect("warning");
    assert!(orphan.exists());
    assert!(!reclaimed.removed);
    assert_eq!(
        reclaimed.warning.message,
        "low disk: 5.0 GiB free; nothing to reclaim; run dx worktree prune --apply"
    );
}

#[test]
fn release_on_low_disk_drops_the_least_recently_used_free_base_first() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    let c = ops::add(&pool, &host, &me, &create("feature/c")).expect("c");
    let base_a = output_base(&roots, "a", &a.path);
    let base_b = output_base(&roots, "b", &b.path);
    let base_c = output_base(&roots, "c", &c.path);
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    host.advance(10);
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    host.advance(10);
    host.serve(71, &b.path);
    host.serve(72, &c.path);
    host.script_space(&[LOW, LOW, OK]);
    let released = ops::release(&pool, &host, &me, &c.path, true).expect("release c");
    assert!(!base_b.exists());
    assert!(base_a.exists());
    assert!(base_c.exists());
    assert_eq!(host.shutdown_paths(), vec![c.path.clone(), b.path.clone()]);
    assert_eq!(released.warnings.len(), 1);
    assert_eq!(released.warnings[0].code, "disk_low");
    assert_eq!(
        released.warnings[0].message,
        format!(
            "low disk: 5.0 GiB free; removed the output bases of free slot(s) {}; 50.0 GiB free now",
            b.slot
        )
    );
}

const ROOT_LOW: DiskSpace = DiskSpace {
    free: 3 * GIB,
    total: 400 * GIB,
};

#[test]
fn low_disk_on_another_output_root_reclaims_bases_on_that_root() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    let orphan = output_base(&roots, "orphan", &fixture.cache.join("gone"));
    host.script_disk(&fixture.cache, OK);
    host.script_disk(&roots, ROOT_LOW);
    let reclaimed = ops::reclaim_if_low(&pool, &host, None)
        .expect("reclaim")
        .expect("warning");
    assert!(!orphan.exists());
    assert!(!base.exists());
    assert!(reclaimed.removed);
    assert_eq!(
        reclaimed.warning.message,
        format!(
            "low disk: 3.0 GiB free; removed 1 orphaned output base(s) and the output bases of free slot(s) {}; 3.0 GiB free now; run dx worktree prune --apply",
            a.slot
        )
    );
}

#[test]
fn low_disk_on_another_output_root_keeps_bases_on_a_root_with_room() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let low = fixture.cache.join("bazel-root");
    let roomy = fixture.cache.join("bazel-other");
    host.roots.lock().expect("roots").push(low.clone());
    host.roots.lock().expect("roots").push(roomy.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    host.advance(10);
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    let base_a = output_base(&roomy, "a", &a.path);
    let base_b = output_base(&low, "b", &b.path);
    let orphan_low = output_base(&low, "orphan", &fixture.cache.join("gone"));
    let orphan_roomy = output_base(&roomy, "orphan", &fixture.cache.join("gone2"));
    host.script_disk(&fixture.cache, OK);
    host.script_disk(&low, ROOT_LOW);
    host.script_disk(&roomy, OK);
    host.serve(73, &b.path);
    let shutdowns = host.shutdown_count();
    let reclaimed = ops::reclaim_if_low(&pool, &host, None)
        .expect("reclaim")
        .expect("warning");
    assert!(!orphan_low.exists());
    assert!(orphan_roomy.exists());
    assert!(!base_b.exists());
    assert!(base_a.exists(), "a base on a disk with room stays");
    assert_eq!(host.shutdown_count(), shutdowns + 1);
    assert_eq!(
        reclaimed.warning.message,
        format!(
            "low disk: 3.0 GiB free; removed 1 orphaned output base(s) and the output bases of free slot(s) {}; 3.0 GiB free now; run dx worktree prune --apply",
            b.slot
        )
    );
}

#[test]
fn low_disk_on_another_output_root_lists_only_its_free_bases_in_prune() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let low = fixture.cache.join("bazel-root");
    let roomy = fixture.cache.join("bazel-other");
    host.roots.lock().expect("roots").push(low.clone());
    host.roots.lock().expect("roots").push(roomy.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    let base_a = output_base(&low, "a", &a.path);
    output_base(&roomy, "b", &b.path);
    host.script_disk(&fixture.cache, OK);
    host.script_disk(&low, ROOT_LOW);
    host.script_disk(&roomy, OK);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.bases.len(), 1, "{:?}", plan.bases);
    assert_eq!(plan.bases[0].dir, base_a);
    assert_eq!(plan.bases[0].owner, ops::BaseOwner::Slot(a.slot));
}

#[test]
fn low_disk_on_another_output_root_reclaims_before_a_build() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    host.script_disk(&fixture.cache, OK);
    host.script_disk(&roots, ROOT_LOW);
    assert_eq!(
        super::reclaim_for_build_with(&fixture.main, &fixture.cache, &host),
        Some(format!(
            "low disk: 3.0 GiB free; removed the output bases of free slot(s) {}; 3.0 GiB free now; run dx worktree prune --apply",
            a.slot
        ))
    );
    assert!(!base.exists());
}

#[test]
fn add_deletes_free_slots_idle_for_thirty_days() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    ops::release(&pool, &host, &me, &a.path, true).expect("release a");
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    host.advance(31 * DAY);
    let c = ops::add(&pool, &host, &me, &create("feature/c")).expect("c");
    let slots: Vec<u32> = pool.records().iter().map(|record| record.slot).collect();
    assert_eq!(slots, vec![c.slot]);
    let leftover = if c.slot == a.slot { &b.path } else { &a.path };
    assert!(!leftover.exists());
}

#[test]
fn a_reused_slot_keeps_its_output_base_and_prune_deletes_orphaned_ones() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    let kept = roots.join("aaa");
    let orphan = roots.join("bbb");
    std::fs::create_dir_all(&kept).expect("kept");
    std::fs::create_dir_all(orphan.join("external")).expect("orphan");
    std::fs::write(orphan.join("external").join("fetched"), "x").expect("fetched");
    std::fs::write(
        orphan.join("DO_NOT_BUILD_HERE"),
        fixture.cache.join("gone").to_string_lossy().as_bytes(),
    )
    .expect("orphan marker");
    host.roots.lock().expect("roots").push(roots);
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    std::fs::write(
        kept.join("DO_NOT_BUILD_HERE"),
        first.path.to_string_lossy().as_bytes(),
    )
    .expect("kept marker");
    ops::release(&pool, &host, &me, &first.path, true).expect("release");
    let second = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    assert_eq!(
        second.path, first.path,
        "the same path keeps the output base"
    );
    assert!(kept.exists());
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.bases.len(), 1);
    assert_eq!(plan.bases[0].dir, orphan);
    assert!(
        host.disk_paths().contains(&orphan),
        "prune measures disk use"
    );
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(!orphan.exists());
    assert!(kept.exists());
    ops::release(&pool, &host, &me, &second.path, true).expect("release b");
    host.advance(31 * DAY);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply slots");
    assert!(!second.path.exists());
    assert!(!kept.exists(), "the slot's output base goes with it");
}

#[test]
fn prune_apply_reports_a_free_slot_base_whose_server_will_not_stop_as_skipped() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let slot = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let base = output_base(&roots, "aaa", &slot.path);
    host.script_space(&[OK]);
    ops::release(&pool, &host, &me, &slot.path, true).expect("release");
    host.script_space(&[LOW]);
    host.servers.lock().expect("servers").push(Server {
        pid: 42,
        rss_bytes: 0,
        workspace: Some(slot.path.clone()),
        output_base: None,
        output_user_root: None,
    });
    host.stubborn(42);
    let pruned = ops::prune(&pool, &host, true, false).expect("prune");
    assert_eq!(pruned.skipped_bases.len(), 1);
    assert_eq!(pruned.skipped_bases[0].dir, base);
    assert_eq!(pruned.skipped_bases[0].reason, "server");
    assert_eq!(
        pruned.skipped_bases[0].owner,
        ops::BaseOwner::Slot(slot.slot)
    );
    assert!(pruned.plan.bases.is_empty());
    assert_eq!(pruned.plan.bytes(), 0);
    assert!(base.exists());
}

#[test]
fn prune_reclaims_output_bases_of_removed_claude_worktree_checkouts() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    let live = fixture.cache.join("repo with space/.claude/worktrees/live");
    let gone = fixture.cache.join("repo with space/.claude/worktrees/gone");
    std::fs::create_dir_all(&live).expect("live");
    for (name, workspace) in [("live", &live), ("gone", &gone)] {
        let base = roots.join(name);
        std::fs::create_dir_all(base.join("external")).expect("base");
        std::fs::write(
            base.join("DO_NOT_BUILD_HERE"),
            format!("{}\r\n", workspace.display()),
        )
        .expect("marker");
    }
    host.roots.lock().expect("roots").push(roots.clone());
    let served = output_base(&roots, "served", &fixture.cache.join("served"));
    host.servers.lock().expect("servers").push(Server {
        pid: 42,
        rss_bytes: 0,
        workspace: None,
        output_base: Some(served.clone()),
        output_user_root: None,
    });
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.bases.len(), 1);
    assert_eq!(plan.bases[0].dir, roots.join("gone"));
    assert_eq!(plan.bases[0].workspace, gone);
    assert_eq!(plan.bases[0].owner, ops::BaseOwner::Orphan);
    assert_eq!(plan.kept_bases.len(), 1);
    assert_eq!(plan.kept_bases[0].dir, served);
    assert_eq!(plan.kept_bases[0].reason, "server");
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(!roots.join("gone").exists());
    assert!(roots.join("live").exists());
    assert!(served.exists());
}

#[test]
fn prune_apply_deletes_nothing_when_the_pool_lock_is_held_elsewhere() {
    let fixture = Fixture::new();
    let mut pool = fixture.pool();
    pool.lock_wait = std::time::Duration::from_millis(200);
    let host = FakeHost::new();
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &first.path, true).expect("release");
    host.advance(31 * DAY);
    assert_eq!(
        ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
            .expect("plan")
            .slots
            .len(),
        1
    );
    let measured = host.disk_paths().len();
    let _held = pool.lock().expect("lock");
    let refused = ops::prune(&pool, &host, true, false);
    assert_eq!(refused.expect_err("busy").code, "pool_busy");
    assert!(first.path.exists());
    assert_eq!(
        host.disk_paths().len(),
        measured,
        "planning waits for the pool lock"
    );
}

#[test]
fn release_of_a_slot_waits_for_the_pool_lock() {
    let fixture = Fixture::new();
    let mut pool = fixture.pool();
    pool.lock_wait = std::time::Duration::from_millis(200);
    let host = FakeHost::new();
    let me = who("s1", 10);
    let added = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let _held = pool.lock().expect("lock");
    let refused = ops::release(&pool, &host, &me, &added.path, true).expect_err("busy");
    assert_eq!(refused.code, "pool_busy");
    assert!(
        refused.message.contains("run the command again"),
        "{}",
        refused.message
    );
    let record = pool.record(added.slot).expect("record");
    assert!(record.held);
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "feature/a");
}

#[test]
fn prune_lists_old_free_slots_and_apply_deletes_them() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    host.live(10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
    let c = ops::add(&pool, &host, &me, &create("feature/c")).expect("c");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .is_empty());
    host.advance(31 * DAY);
    let base_b = output_base(&roots, "b", &b.path);
    let base_c = output_base(&roots, "c", &c.path);
    ops::release(&pool, &host, &me, &b.path, true).expect("release b");
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert!(
        plan.bases.is_empty(),
        "plenty of disk keeps free slots' bases"
    );
    host.script_space(&[LOW]);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.slots.len(), 1);
    assert_eq!(plan.slots[0].idle_days, 31);
    assert_eq!(plan.bases.len(), 1, "{:?}", plan.bases);
    assert_eq!(plan.bases[0].dir, base_b);
    assert_eq!(plan.bases[0].owner, ops::BaseOwner::Slot(b.slot));
    assert!(a.path.exists());
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(!a.path.exists());
    assert!(!base_b.exists());
    assert!(
        b.path.exists(),
        "the free slot stays warm without its output base"
    );
    assert!(base_c.exists(), "a held slot keeps its output base");
    let slots: Vec<u32> = pool.records().iter().map(|record| record.slot).collect();
    assert_eq!(slots, vec![b.slot, c.slot]);
    host.script_space(&[OK]);
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .is_empty());
}

#[test]
fn new_branches_start_from_the_default_branch_of_origin() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let origin_clone = fixture.clone_origin();
    std::fs::write(origin_clone.join("new.txt"), "n\n").expect("write");
    git(&origin_clone, &["add", "."]);
    git(&origin_clone, &["commit", "--quiet", "-m", "newer"]);
    git(&origin_clone, &["push", "--quiet", "origin", "main"]);
    let added = ops::add(
        &pool,
        &host,
        &who("s1", 10),
        &AddRequest {
            branch: "feature/fresh".to_owned(),
            create: None,
            force: false,
            unclaimed: false,
        },
    )
    .expect("add");
    assert!(
        added.path.join("new.txt").is_file(),
        "fetched before branching"
    );
}

#[test]
fn worktreeinclude_files_are_copied_into_the_slot() {
    let fixture = Fixture::new();
    std::fs::write(fixture.main.join(".gitignore"), ".env\n").expect("ignore");
    std::fs::write(fixture.main.join(".worktreeinclude"), ".env\n").expect("include");
    git(&fixture.main, &["add", "."]);
    git(&fixture.main, &["commit", "--quiet", "-m", "include"]);
    git(&fixture.main, &["push", "--quiet", "origin", "main"]);
    std::fs::write(fixture.main.join(".env"), "SECRET=1\n").expect("env");
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    assert_eq!(
        std::fs::read_to_string(added.path.join(".env")).expect("copied"),
        "SECRET=1\n"
    );
}

#[test]
fn the_lease_is_found_from_a_path_inside_the_slot() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    let inner = added.path.join("sub").join("dir");
    std::fs::create_dir_all(&inner).expect("dirs");
    let (slot_path, record) = super::lease_for_workspace(&inner).expect("lease");
    assert_eq!(slot_path, added.path);
    assert_eq!(record.session.as_deref(), Some("s1"));
    assert!(record.held);
    assert!(super::lease_for_workspace(&fixture.main).is_none());
}

mod hooks {
    use super::*;
    use crate::worktree::hook;

    fn blocked(command: &str) -> Option<String> {
        match hook::guard(&serde_json::json!({"tool_input": {"command": command}})) {
            hook::Guard::Block(line) => Some(line),
            _ => None,
        }
    }

    fn guard_for_agent(command: &str) -> hook::Guard {
        hook::guard(&serde_json::json!({
            "agent_id": "a'1",
            "tool_input": {"command": command, "description": "d"},
        }))
    }

    #[test]
    fn guard_names_the_subagent_for_every_dx_command() {
        let hook::Guard::Rewrite(output) = guard_for_agent("cd /r && tools/dx worktree add x")
        else {
            panic!("dx worktree is rewritten");
        };
        let updated = &output["hookSpecificOutput"]["updatedInput"];
        assert_eq!(output["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(
            updated["command"],
            "export DX_WORKTREE_HOLDER='a'\\''1'\ncd /r && tools/dx worktree add x"
        );
        assert_eq!(updated["description"], "d");
        assert!(matches!(
            guard_for_agent("bazel run @rules_dx//:dx -- worktree release"),
            hook::Guard::Rewrite(_)
        ));
        for command in [
            "tools/dx test //a:b",
            "cd /r && timeout -s KILL -k 5 600 tools/dx check",
            "nice -n 5 dx build",
            "stdbuf -oL -e 0 tools/dx test",
            "setsid tools/dx test",
            "ionice -c 3 tools/dx test",
            "bash -lc 'cd x && tools/dx check'",
            "sh -c \"timeout 5 tools/dx test\"",
            "bazel run //:dx -- test",
        ] {
            let hook::Guard::Rewrite(output) = guard_for_agent(command) else {
                panic!("{command} is rewritten");
            };
            assert!(
                output["hookSpecificOutput"]
                    .get("permissionDecision")
                    .is_none(),
                "{command}"
            );
            let rewritten = output["hookSpecificOutput"]["updatedInput"]["command"]
                .as_str()
                .expect("command");
            assert!(rewritten.ends_with(command), "{rewritten}");
        }
        for command in [
            "echo dx test",
            "cat tools/dx",
            "timeout 5 sleep 1",
            "bash -c 'echo dx'",
            "echo dx worktree",
            "bash -c 'echo \"unterminated",
        ] {
            assert_eq!(guard_for_agent(command), hook::Guard::Allow, "{command}");
        }
        assert!(matches!(
            guard_for_agent("git worktree add x"),
            hook::Guard::Block(_)
        ));
        assert_eq!(
            hook::guard(&serde_json::json!({"tool_input": {"command": "dx worktree list"}})),
            hook::Guard::Allow
        );
    }

    /// A shell-tool payload in the shape each agent documents.
    fn payload(agent: &str, command: &str) -> serde_json::Value {
        match agent {
            "claude" => serde_json::json!({
                "session_id": "c1",
                "transcript_path": "/t",
                "cwd": "/r",
                "permission_mode": "default",
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {"command": command, "description": "d"},
            }),
            "codex" => serde_json::json!({
                "session_id": "x1",
                "transcript_path": "/t",
                "cwd": "/r",
                "hook_event_name": "PreToolUse",
                "model": "m",
                "turn_id": "t1",
                "tool_name": "Bash",
                "tool_use_id": "u1",
                "permission_mode": "default",
                "tool_input": {"command": command},
            }),
            "cursor" => serde_json::json!({
                "conversation_id": "k'1",
                "generation_id": "g1",
                "model": "m",
                "hook_event_name": "preToolUse",
                "cursor_version": "2.0",
                "workspace_roots": ["/r"],
                "transcript_path": "/t",
                "tool_name": "Shell",
                "tool_use_id": "u1",
                "cwd": "/r",
                "tool_input": {"command": command, "working_directory": "/r/sub"},
            }),
            "gemini" => serde_json::json!({
                "session_id": "m1",
                "transcript_path": "/t",
                "cwd": "/r",
                "hook_event_name": "BeforeTool",
                "timestamp": "2026-10-27T00:00:00Z",
                "tool_name": "run_shell_command",
                "tool_input": {"command": command, "description": "d", "dir_path": "sub"},
            }),
            _ => unreachable!("{agent}"),
        }
    }

    #[test]
    fn dialects_come_from_the_documented_payload_keys() {
        use hook::Dialect;
        for (agent, dialect) in [
            ("claude", Dialect::Claude),
            ("codex", Dialect::Claude),
            ("cursor", Dialect::Cursor),
            ("gemini", Dialect::Gemini),
        ] {
            assert_eq!(Dialect::of(&payload(agent, "ls")), dialect, "{agent}");
        }
        assert_eq!(
            Dialect::of(&serde_json::json!({"conversation_id": "k"})),
            Dialect::Cursor
        );
        assert_eq!(
            Dialect::of(&serde_json::json!({"cursor_version": "2"})),
            Dialect::Cursor
        );
        assert_eq!(Dialect::of(&serde_json::json!({})), Dialect::Claude);
    }

    #[test]
    fn guard_blocks_git_worktree_for_every_agent() {
        for agent in ["claude", "codex", "cursor", "gemini"] {
            let guard = hook::guard(&payload(agent, "git worktree add ../x"));
            assert!(matches!(guard, hook::Guard::Block(_)), "{agent}: {guard:?}");
            let guard = hook::guard(&payload(agent, "ls -la"));
            assert_eq!(guard, hook::Guard::Allow, "{agent}");
        }
    }

    #[test]
    fn guard_names_the_session_only_for_agents_whose_shells_lack_one() {
        for agent in ["claude", "codex"] {
            assert_eq!(
                hook::guard(&payload(agent, "tools/dx test //x")),
                hook::Guard::Allow,
                "{agent}"
            );
        }
        let hook::Guard::Rewrite(cursor) = hook::guard(&payload("cursor", "tools/dx test //x"))
        else {
            panic!("cursor dx commands are rewritten");
        };
        let output = &cursor["hookSpecificOutput"];
        assert_eq!(output["hookEventName"], "PreToolUse");
        assert_eq!(
            output["updatedInput"]["command"],
            "export DX_WORKTREE_SESSION='k'\\''1'\ntools/dx test //x"
        );
        assert_eq!(output["updatedInput"]["working_directory"], "/r/sub");
        assert!(output.get("permissionDecision").is_none());
        let hook::Guard::Rewrite(gemini) =
            hook::guard(&payload("gemini", "cd sub && tools/dx worktree add b"))
        else {
            panic!("gemini dx commands are rewritten");
        };
        let output = &gemini["hookSpecificOutput"];
        assert_eq!(output["hookEventName"], "BeforeTool");
        assert!(output.get("updatedInput").is_none());
        assert_eq!(
            output["tool_input"]["command"],
            "export DX_WORKTREE_SESSION='m1'\ncd sub && tools/dx worktree add b"
        );
        assert_eq!(output["tool_input"]["description"], "d");
        assert_eq!(output["tool_input"]["dir_path"], "sub");
        let mut no_session = payload("gemini", "tools/dx test //x");
        no_session["session_id"] = serde_json::Value::Null;
        assert_eq!(hook::guard(&no_session), hook::Guard::Allow);
    }

    #[test]
    fn guard_exports_the_session_before_the_subagent() {
        let mut input = payload("cursor", "tools/dx worktree release");
        input["agent_id"] = serde_json::json!("a1");
        let hook::Guard::Rewrite(output) = hook::guard(&input) else {
            panic!("rewritten");
        };
        assert_eq!(
            output["hookSpecificOutput"]["updatedInput"]["command"],
            "export DX_WORKTREE_SESSION='k'\\''1'\nexport DX_WORKTREE_HOLDER='a1'\ntools/dx worktree release"
        );
    }

    #[test]
    fn a_cursor_subagent_stop_without_an_agent_id_is_ignored() {
        let cursor = serde_json::json!({"conversation_id": "k1", "subagent_type": "explore"});
        assert!(hook::ignored(hook::HookKind::Stop, &cursor));
        assert!(!hook::ignored(hook::HookKind::Stop, &serde_json::json!({})));
        assert!(!hook::ignored(hook::HookKind::Create, &cursor));
    }

    #[test]
    fn guard_blocks_git_worktree_through_wrappers() {
        for command in [
            "timeout 30 git worktree add x",
            "bash -c 'git worktree remove x'",
            "nice -n 5 git worktree prune",
        ] {
            assert!(
                matches!(guard_for_agent(command), hook::Guard::Block(_)),
                "{command}"
            );
        }
        let mut deep = "git worktree add x".to_owned();
        for _ in 0..6 {
            deep = format!("bash -c '{}'", deep.replace('\'', "'\\''"));
        }
        let _ = guard_for_agent(&deep);
    }

    #[test]
    fn a_stopped_subagent_leaves_its_slot_reclaimable_by_another_agent() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        host.live(10);
        let first = agent("s1", "a1", 10);
        let second = agent("s1", "a2", 10);
        let added = ops::add(&pool, &host, &first, &create("feature/a")).expect("add");
        git(
            &added.path,
            &["push", "--quiet", "-u", "origin", "feature/a"],
        );
        let held = ops::add(&pool, &host, &second, &existing("feature/a")).expect_err("live");
        assert_eq!(held.code, "branch_held");
        let stop = serde_json::json!({"agent_id": "a2"});
        assert!(hook::stop(&pool, &first, &stop).expect("stop").is_empty());
        let stop = serde_json::json!({"agent_id": "a1"});
        assert_eq!(hook::stop(&pool, &first, &stop).expect("stop"), vec![1]);
        let row = ops::list(&pool, &host, false).expect("list").rows.remove(0);
        assert_eq!(row.state, State::Reclaimable);
        assert!(row.holder.expect("holder").ends_with("(gone)"));
        let taken = ops::add(&pool, &host, &second, &existing("feature/a")).expect("taken");
        assert_eq!(taken.slot, added.slot);
        assert!(!taken.reused);
        assert!(hook::stop(&pool, &first, &serde_json::json!({})).is_err());
    }

    #[test]
    fn guard_blocks_git_worktree_add_remove_and_prune_only() {
        let block = blocked;
        assert!(block("git worktree add ../x -b y")
            .expect("add")
            .contains("worktree add"));
        assert!(block("cd /r && git -C /r worktree remove x")
            .expect("remove")
            .contains("release"));
        assert!(block("git worktree prune").is_some());
        assert!(block("/usr/bin/git worktree add x").is_some());
        assert!(block("git worktree list").is_none());
        assert!(block("git status").is_none());
        assert!(block("echo git worktree add").is_none());
        assert!(block("dx worktree add feature/x").is_none());
        assert_eq!(hook::guard(&serde_json::json!({})), hook::Guard::Allow);
    }

    #[test]
    fn guard_parses_words_and_ignores_heredocs_and_quotes() {
        let block = blocked;
        assert!(block("python3 - <<EOF\ngit worktree prune\nEOF").is_none());
        assert!(block("cat <<'EOF' > f\ngit worktree remove x\nEOF\nls").is_none());
        assert!(block("cat <<-EOF\n\tgit worktree add x\n\tEOF\n").is_none());
        assert!(block("cat <<EOF\nx\nEOF\ngit worktree prune").is_some());
        assert!(block("echo 'git worktree add x'").is_none());
        assert!(block("echo \"a; git worktree prune\"").is_none());
        assert!(block("grep -r \"git worktree remove\" docs").is_none());
        assert!(block("git -C dir worktree add x").is_some());
        assert!(block("git -C 'my dir' worktree add x").is_some());
        assert!(block("cd x && git worktree prune").is_some());
        assert!(block("\"git\" worktree remove x").is_some());
        assert!(block("FOO=1 git worktree add x").is_some());
    }

    #[test]
    fn guard_blocks_substitutions_here_strings_and_shifts() {
        let block = blocked;
        assert!(block("out=\"$(git worktree add ../x b)\"").is_some());
        assert!(block("echo \"$(git worktree prune)\"").is_some());
        assert!(block("echo \"`git worktree prune`\"").is_some());
        assert!(block("echo $(git worktree prune)").is_some());
        assert!(block("cat <<< hi\ngit worktree prune").is_some());
        assert!(block("echo $((1<<2))\ngit worktree prune").is_some());
        assert!(block("echo \"$(echo hi)\"").is_none());
        assert!(block("echo $((1<<2))").is_none());
    }

    #[test]
    fn guard_blocks_git_after_keywords_wrappers_and_valued_options() {
        let block = blocked;
        for command in [
            "for b in x y; do git worktree add ../$b $b; done",
            "if git worktree add ../x x; then echo ok; fi",
            "if true; then git worktree prune; fi",
            "if false; then :; else git worktree remove x; fi",
            "if false; then :; elif git worktree prune; then :; fi",
            "while git worktree prune; do :; done",
            "until git worktree prune; do :; done",
            "{ git worktree add x; }",
            "! git worktree prune",
            "if ! git worktree add x; then :; fi",
            "time git worktree prune",
            "time -p git worktree prune",
            "env git worktree add x",
            "env -i FOO=1 -u BAR git worktree add x",
            "/usr/bin/env -- git worktree prune",
            "command git worktree add x",
            "command -p git worktree remove x",
            "exec git worktree prune",
            "exec -a name git worktree prune",
            "nohup git worktree add x",
            "sudo git worktree prune",
            "sudo -u root -E FOO=1 git worktree prune",
            "echo x | xargs git worktree remove",
            "ls | xargs -n 1 -I{} git worktree add {}",
            "sudo env nohup git worktree prune",
            "git --git-dir .git worktree add x",
            "git --work-tree . worktree prune",
            "git --namespace n worktree prune",
            "git --exec-path p worktree prune",
            "git --config-env a=b worktree prune",
            "git --super-prefix p worktree prune",
            "git --git-dir=.git worktree add x",
        ] {
            assert!(block(command).is_some(), "{command}");
        }
        for command in [
            "env",
            "sudo -l",
            "xargs echo git worktree add",
            "time make",
            "if true; then echo git worktree add; fi",
            "env FOO=1 git worktree list",
            "git --git-dir .git worktree list",
            "command -v git",
        ] {
            assert!(block(command).is_none(), "{command}");
        }
    }

    #[test]
    fn create_prints_a_slot_path_and_remove_frees_it() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let me = who("s1", 10);
        let input = serde_json::json!({"name": "agent one", "cwd": fixture.main});
        let created = hook::create(&pool, &host, &me, &input).expect("create");
        assert!(created.warnings.is_empty(), "{:?}", created.warnings);
        let path = created.path;
        assert!(path.starts_with(&fixture.cache));
        assert_eq!(
            git(&path, &["branch", "--show-current"]),
            "worktree-agent-one"
        );
        let removal = serde_json::json!({"worktree_path": path});
        let freed = hook::remove(&pool, &host, &me, &removal).expect("remove");
        assert_eq!(freed, Some(path.clone()));
        let again = hook::create(&pool, &host, &me, &input)
            .expect("create again")
            .path;
        assert_eq!(again, path, "the freed slot is reused");
        let elsewhere = serde_json::json!({"worktree_path": fixture.main});
        assert_eq!(
            hook::remove(&pool, &host, &me, &elsewhere).expect("ignored"),
            None
        );
    }

    #[test]
    fn create_returns_fetch_failed_when_origin_cannot_be_fetched() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let missing = fixture.main.parent().expect("root").join("missing.git");
        git(
            &fixture.main,
            &["remote", "set-url", "origin", &missing.to_string_lossy()],
        );
        let input = serde_json::json!({"name": "agent one", "cwd": fixture.main});
        let created = hook::create(&pool, &host, &who("s1", 10), &input).expect("create");
        assert_eq!(created.warnings.len(), 1, "{:?}", created.warnings);
        assert_eq!(created.warnings[0].code, "fetch_failed");
        assert!(
            created.warnings[0]
                .message
                .starts_with("cannot fetch origin, using local refs: "),
            "{:?}",
            created.warnings
        );
        assert_eq!(
            git(&created.path, &["branch", "--show-current"]),
            "worktree-agent-one"
        );
    }

    #[test]
    fn hook_create_on_low_disk_keeps_the_leased_base_and_drops_other_free_bases() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        host.live(10);
        let roots = fixture.cache.join("bazel-root");
        host.roots.lock().expect("roots").push(roots.clone());
        let me = who("s1", 10);
        let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
        let b = ops::add(&pool, &host, &me, &create("feature/b")).expect("b");
        ops::release(&pool, &host, &me, &a.path, true).expect("release a");
        host.advance(10);
        ops::release(&pool, &host, &me, &b.path, true).expect("release b");
        let base_a = output_base(&roots, "a", &a.path);
        let base_b = output_base(&roots, "b", &b.path);
        host.script_space(&[LOW, LOW, OK]);
        let input = serde_json::json!({"name": "agent one", "cwd": fixture.main});
        let created = hook::create(&pool, &host, &me, &input).expect("create");
        assert_eq!(created.slot, b.slot, "the warmest free slot");
        assert!(base_b.exists(), "the leased slot keeps its base");
        assert!(!base_a.exists());
    }

    #[test]
    fn hook_create_on_full_disk_refuses() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        host.live(10);
        host.script_disk(&fixture.cache, super::FULL);
        let input = serde_json::json!({"name": "agent one", "cwd": fixture.main});
        let error = hook::create(&pool, &host, &who("s1", 10), &input).expect_err("full");
        assert_eq!(error.code, "disk_full");
    }

    #[test]
    fn a_hook_created_slot_belongs_to_the_first_agent_that_works_in_it() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        host.live(10);
        let input = serde_json::json!({"name": "agent one", "cwd": fixture.main});
        let path = hook::create(&pool, &host, &who("s1", 10), &input)
            .expect("create")
            .path;
        let slot = pool.slot_of(&path).expect("slot");
        let first = agent("s1", "a1", 10);
        let sibling = agent("s1", "a2", 10);
        assert_eq!(
            ops::claim(&pool, &sibling, &fixture.main).expect("claim"),
            None
        );
        let refused = ops::release(&pool, &host, &sibling, &path, false).expect_err("not claimed");
        assert_eq!(refused.code, "slot_leased");
        let inside = path.join("sub");
        std::fs::create_dir_all(&inside).expect("mkdir");
        assert_eq!(
            ops::claim(&pool, &first, &inside).expect("claim"),
            Some(slot)
        );
        assert_eq!(ops::claim(&pool, &sibling, &path).expect("claim"), None);
        assert_eq!(
            ops::claim(&pool, &agent("s2", "a1", 20), &path).expect("claim"),
            None
        );
        let record = pool.record(slot).expect("record");
        assert!(record.held_by(&first));
        assert!(!record.unclaimed);
        let reused =
            ops::add(&pool, &host, &first, &existing("worktree-agent-one")).expect("reuse");
        assert!(reused.reused);
        assert_eq!(reused.slot, slot);
        let held =
            ops::add(&pool, &host, &sibling, &existing("worktree-agent-one")).expect_err("held");
        assert_eq!(held.code, "branch_held");
        let stop = serde_json::json!({"agent_id": "a1"});
        assert_eq!(hook::stop(&pool, &first, &stop).expect("stop"), vec![slot]);
        let row = ops::list(&pool, &host, false).expect("list").rows.remove(0);
        assert!(row.holder.expect("holder").ends_with("(gone)"));
        let released = ops::release(&pool, &host, &first, &path, false).expect("release");
        assert_eq!(released.slot, Some(slot));
        assert!(!pool.record(slot).expect("record").held);
    }

    #[test]
    fn a_main_session_claims_its_own_hook_created_slot() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        host.live(10);
        let me = who("s1", 10);
        let input = serde_json::json!({"name": "main", "cwd": fixture.main});
        let path = hook::create(&pool, &host, &me, &input)
            .expect("create")
            .path;
        let slot = pool.slot_of(&path).expect("slot");
        assert!(ops::release(&pool, &host, &me, &path, false).is_err());
        assert_eq!(ops::claim(&pool, &me, &path).expect("claim"), Some(slot));
        assert!(pool.record(slot).expect("record").held_by(&me));
        let released = ops::release(&pool, &host, &me, &path, false).expect("release");
        assert_eq!(released.slot, Some(slot));
        assert!(!pool.record(slot).expect("record").held);
    }

    #[test]
    fn create_skips_an_agent_branch_that_exists_only_on_origin() {
        let fixture = Fixture::new();
        let clone = fixture.clone_origin();
        git(&clone, &["checkout", "--quiet", "-b", "worktree-agent-two"]);
        std::fs::write(clone.join("agent.txt"), "a\n").expect("write");
        git(&clone, &["add", "."]);
        git(&clone, &["commit", "--quiet", "-m", "agent work"]);
        git(&clone, &["push", "--quiet", "origin", "worktree-agent-two"]);
        let pushed = git(&clone, &["rev-parse", "HEAD"]);
        let pool = fixture.pool();
        let host = FakeHost::new();
        let me = who("s1", 10);
        let input = serde_json::json!({"name": "agent two", "cwd": fixture.main});
        let path = hook::create(&pool, &host, &me, &input)
            .expect("create")
            .path;
        let base = git(&fixture.main, &["rev-parse", "origin/main"]);
        assert_ne!(base, pushed);
        assert_eq!(git(&path, &["rev-parse", "HEAD"]), base);
        assert_eq!(
            git(&path, &["branch", "--show-current"]),
            "worktree-agent-two-2"
        );
        assert_eq!(
            git(
                &path,
                &[
                    "for-each-ref",
                    "--format=%(upstream)",
                    "refs/heads/worktree-agent-two-2"
                ]
            ),
            ""
        );
        let removal = serde_json::json!({"worktree_path": path});
        hook::remove(&pool, &host, &me, &removal).expect("remove");
        let again = hook::create(&pool, &host, &me, &input)
            .expect("create again")
            .path;
        assert_eq!(
            git(&again, &["branch", "--show-current"]),
            "worktree-agent-two-2"
        );
    }

    #[test]
    fn a_local_agent_branch_wins_over_origin() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let me = who("s1", 10);
        let input = serde_json::json!({"name": "agent one", "cwd": fixture.main});
        let path = hook::create(&pool, &host, &me, &input)
            .expect("create")
            .path;
        let local = git(&path, &["rev-parse", "HEAD"]);
        let removal = serde_json::json!({"worktree_path": path});
        hook::remove(&pool, &host, &me, &removal).expect("remove");
        let clone = fixture.clone_origin();
        git(&clone, &["checkout", "--quiet", "-b", "worktree-agent-one"]);
        std::fs::write(clone.join("agent.txt"), "a\n").expect("write");
        git(&clone, &["add", "."]);
        git(&clone, &["commit", "--quiet", "-m", "agent work"]);
        git(&clone, &["push", "--quiet", "origin", "worktree-agent-one"]);
        let again = hook::create(&pool, &host, &me, &input)
            .expect("create again")
            .path;
        assert_eq!(
            git(&again, &["branch", "--show-current"]),
            "worktree-agent-one"
        );
        assert_eq!(git(&again, &["rev-parse", "HEAD"]), local);
    }

    #[test]
    fn ten_agents_in_a_row_without_remove_hooks_use_at_most_two_slots() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let me = who("s1", 10);
        for n in 0..10u32 {
            let input = serde_json::json!({"name": format!("agent-{n}"), "cwd": fixture.main});
            let path = hook::create(&pool, &host, &me, &input)
                .expect("create")
                .path;
            std::fs::write(path.join("scratch.txt"), "x").expect("scratch");
            std::fs::remove_file(path.join("scratch.txt")).expect("clean again");
            host.advance(120);
        }
        assert!(pool.records().len() <= 2, "{:?}", pool.records());
    }

    #[test]
    fn a_running_or_unfinished_agent_keeps_its_slot() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        host.live(10);
        let me = who("s1", 10);
        let first = serde_json::json!({"name": "one", "cwd": fixture.main});
        let one = hook::create(&pool, &host, &me, &first).expect("one").path;
        let second = serde_json::json!({"name": "two", "cwd": fixture.main});
        let two = hook::create(&pool, &host, &me, &second).expect("two").path;
        assert_ne!(one, two, "a young lease is not recycled");
        host.advance(120);
        std::fs::write(two.join("work.txt"), "dirty").expect("dirty");
        let third = serde_json::json!({"name": "three", "cwd": fixture.main});
        let three = hook::create(&pool, &host, &me, &third).expect("three").path;
        assert_ne!(
            three, one,
            "the slot freed by this create is not handed back"
        );
        assert_ne!(three, two, "a dirty agent slot is kept");
        let freed = pool.slot_of(&one).and_then(|slot| pool.record(slot));
        assert!(
            freed.is_some_and(|record| !record.held),
            "the clean old agent slot is freed"
        );
        let fourth = serde_json::json!({"name": "four", "cwd": fixture.main});
        let four = hook::create(&pool, &host, &me, &fourth).expect("four").path;
        assert_eq!(four, one, "a later create reuses the freed slot");
        assert_eq!(pool.records().len(), 3);
    }

    #[test]
    fn hook_create_does_not_recycle_a_busy_agent_slot() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let me = who("s1", 10);
        let first = serde_json::json!({"name": "one", "cwd": fixture.main});
        let one = hook::create(&pool, &host, &me, &first).expect("one").path;
        let slot = pool.slot_of(&one).expect("slot");
        host.advance(120);
        let running = run_dx_in(&pool, slot);
        let second = serde_json::json!({"name": "two", "cwd": fixture.main});
        let two = hook::create(&pool, &host, &me, &second).expect("two").path;
        assert_ne!(two, one);
        assert!(pool.record(slot).is_some_and(|record| record.held));
        drop(running);
        host.advance(120);
        let third = serde_json::json!({"name": "three", "cwd": fixture.main});
        hook::create(&pool, &host, &me, &third).expect("three");
        assert!(pool.record(slot).is_some_and(|record| !record.held));
    }

    #[test]
    fn hook_input_must_be_json_with_a_name() {
        assert!(hook::parse("not json").is_err());
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let refused =
            hook::create(&pool, &host, &who("s", 1), &serde_json::json!({})).expect_err("no name");
        assert!(refused.usage);
    }

    #[test]
    fn branch_names_are_made_from_the_agent_name() {
        assert_eq!(hook::branch_for("agent-1"), "worktree-agent-1");
        assert_eq!(hook::branch_for("a b/c"), "worktree-a-b-c");
    }
}

fn hand_checkout(fixture: &Fixture, name: &str) -> PathBuf {
    let hand = fixture.cache.parent().expect("parent").join(name);
    git(
        &fixture.main,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            name,
            &hand.to_string_lossy(),
        ],
    );
    hand
}

fn detached_checkout(fixture: &Fixture, name: &str, rev: &str) -> PathBuf {
    let hand = fixture.cache.parent().expect("parent").join(name);
    git(
        &fixture.main,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            &hand.to_string_lossy(),
            rev,
        ],
    );
    hand
}

/// Sets the times of the files git updates for `checkout` to `secs`.
fn age_checkout(checkout: &Path, secs: u64) {
    let admin = PathBuf::from(git(checkout, &["rev-parse", "--absolute-git-dir"]));
    for name in ["HEAD", "index", "logs/HEAD"] {
        let file = admin.join(name);
        if file.exists() {
            set_time(&file, secs);
        }
    }
}

fn kept_reason(plan: &ops::PrunePlan, path: &Path) -> Option<&'static str> {
    plan.kept_externals
        .iter()
        .find(|item| item.path == path)
        .map(|item| item.reason)
}

#[test]
fn release_removes_a_clean_pushed_external_worktree() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = hand_checkout(&fixture, "by-hand");
    git(&hand, &["push", "--quiet", "-u", "origin", "by-hand"]);
    host.serve(74, &hand);
    let released =
        ops::release(&pool, &host, &who("s1", 10), &hand, false).expect("release external");
    assert!(released.removed);
    assert_eq!(released.slot, None);
    assert_eq!(released.branch.as_deref(), Some("by-hand"));
    assert!(!hand.exists());
    assert_eq!(host.shutdown_count(), 1);
    assert!(host.servers().is_empty());
    assert!(!git(&fixture.main, &["worktree", "list"]).contains("by-hand"));
    assert!(git(&fixture.main, &["branch", "--list", "by-hand"]).contains("by-hand"));
}

#[test]
fn release_refuses_a_dirty_or_unpushed_external_worktree_until_forced() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let hand = hand_checkout(&fixture, "by-hand");
    git(&hand, &["push", "--quiet", "-u", "origin", "by-hand"]);
    std::fs::write(hand.join("wip.txt"), "x").expect("wip");
    let dirty = ops::release(&pool, &host, &me, &hand, false).expect_err("dirty");
    assert_eq!(dirty.code, "dirty_slot");
    assert!(hand.exists());
    std::fs::remove_file(hand.join("wip.txt")).expect("clean");
    std::fs::write(hand.join("b.txt"), "y").expect("b");
    git(&hand, &["add", "."]);
    git(&hand, &["commit", "--quiet", "-m", "local"]);
    let ahead = ops::release(&pool, &host, &me, &hand, false).expect_err("unpushed");
    assert_eq!(ahead.code, "unpushed_commits");
    assert!(hand.exists());
    std::fs::write(hand.join("wip.txt"), "x").expect("wip");
    ops::release(&pool, &host, &me, &hand, true).expect("forced");
    assert!(!hand.exists());
}

#[test]
fn release_rejects_paths_that_are_not_worktrees() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let refused =
        ops::release(&pool, &host, &who("s1", 10), &fixture.main, true).expect_err("main checkout");
    assert_eq!(refused.code, "not_a_slot");
    assert!(
        refused.message.contains("dx worktree list"),
        "{}",
        refused.message
    );
    assert!(fixture.main.exists());
}

#[test]
fn refusals_name_the_next_step() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let invalid = ops::add(&pool, &host, &me, &existing("x y")).expect_err("invalid");
    assert_eq!(invalid.code, "invalid_branch");
    assert!(
        invalid.message.contains("check-ref-format"),
        "{}",
        invalid.message
    );
    let unknown = AddRequest {
        create: Some("nope".to_owned()),
        ..existing("feature/n")
    };
    let unknown = ops::add(&pool, &host, &me, &unknown).expect_err("unknown");
    assert_eq!(unknown.code, "unknown_ref");
    assert!(
        unknown.message.contains("--create=origin/"),
        "{}",
        unknown.message
    );
    let empty = pool.slot_path(9);
    std::fs::create_dir_all(&empty).expect("slot dir");
    let missing = ops::release(&pool, &host, &me, &empty, false).expect_err("no lease");
    assert_eq!(missing.code, "not_a_slot");
    assert!(missing.message.contains("slot 9"), "{}", missing.message);
    assert!(
        missing.message.contains("dx worktree list"),
        "{}",
        missing.message
    );
    let unheld = ops::path_of(&pool, "feature/none").expect_err("unheld");
    assert_eq!(unheld.code, "branch_not_held");
    assert!(
        unheld.message.contains("dx worktree add feature/none"),
        "{}",
        unheld.message
    );
    git(&fixture.main, &["remote", "remove", "origin"]);
    let no_default = ops::add(&pool, &host, &me, &existing("feature/fresh")).expect_err("no base");
    assert_eq!(no_default.code, "no_default_branch");
    assert!(
        no_default.message.ends_with("pass --create=<ref>"),
        "{}",
        no_default.message
    );
}

#[test]
fn prune_removes_clean_external_worktrees_on_merged_branches_only() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let merged = hand_checkout(&fixture, "merged");
    std::fs::write(merged.join("m.txt"), "m").expect("m");
    git(&merged, &["add", "."]);
    git(&merged, &["commit", "--quiet", "-m", "merged work"]);
    git(&merged, &["push", "--quiet", "-u", "origin", "merged"]);
    git(&fixture.main, &["fetch", "--quiet", "origin"]);
    git(
        &fixture.main,
        &["merge", "--quiet", "--ff-only", "origin/merged"],
    );
    git(&fixture.main, &["push", "--quiet", "origin", "HEAD"]);
    let fresh = hand_checkout(&fixture, "fresh");
    let locked = hand_checkout(&fixture, "locked-merged");
    let serving = hand_checkout(&fixture, "serving-merged");
    for hand in [&locked, &serving] {
        git(hand, &["push", "--quiet", "-u", "origin", "HEAD"]);
    }
    git(
        &fixture.main,
        &["worktree", "lock", &locked.to_string_lossy()],
    );
    host.servers.lock().expect("servers").push(Server {
        pid: 7,
        rss_bytes: 1,
        workspace: Some(serving.clone()),
        output_base: None,
        output_user_root: None,
    });
    let open = hand_checkout(&fixture, "open");
    let dirty = hand_checkout(&fixture, "dirty-merged");
    std::fs::write(open.join("c.txt"), "z").expect("c");
    git(&open, &["add", "."]);
    git(&open, &["commit", "--quiet", "-m", "open work"]);
    git(&open, &["push", "--quiet", "-u", "origin", "open"]);
    std::fs::write(dirty.join("wip.txt"), "x").expect("wip");
    let roots = fixture.cache.join("bazel-root");
    let base = roots.join("merged-base");
    std::fs::create_dir_all(&base).expect("base");
    std::fs::write(
        base.join("DO_NOT_BUILD_HERE"),
        merged.to_string_lossy().as_bytes(),
    )
    .expect("marker");
    host.roots.lock().expect("roots").push(roots);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    let paths: Vec<&PathBuf> = plan.externals.iter().map(|item| &item.path).collect();
    assert_eq!(paths, vec![&merged]);
    assert!(merged.exists());
    for (path, reason) in [
        (&fresh, "local"),
        (&open, "unmerged"),
        (&dirty, "dirty"),
        (&locked, "locked"),
        (&serving, "server"),
    ] {
        assert_eq!(kept_reason(&plan, path), Some(reason), "{}", path.display());
    }
    assert_eq!(plan.kept_externals.len(), 5);
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(!merged.exists());
    assert!(!base.exists());
    assert!(open.exists());
    assert!(dirty.exists());
    assert!(fresh.exists());
    assert!(locked.exists());
    assert!(serving.exists());
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .externals
        .is_empty());
}

#[test]
fn prune_removes_external_worktrees_whose_upstream_branch_was_deleted() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = hand_checkout(&fixture, "topic");
    std::fs::write(hand.join("t.txt"), "t").expect("t");
    git(&hand, &["add", "."]);
    git(&hand, &["commit", "--quiet", "-m", "topic"]);
    git(&hand, &["push", "--quiet", "-u", "origin", "topic"]);
    git(&hand, &["push", "--quiet", "origin", "topic:kept"]);
    let follower = tracking_checkout(&fixture, "follower", "origin/topic");
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .externals
        .is_empty());
    git(
        &fixture.main,
        &["push", "--quiet", "origin", "--delete", "topic"],
    );
    git(&fixture.main, &["fetch", "--quiet", "--prune", "origin"]);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.externals.len(), 1);
    assert_eq!(plan.externals[0].path, hand);
    assert!(follower.exists());
}

fn tracking_checkout(fixture: &Fixture, name: &str, upstream: &str) -> PathBuf {
    let hand = fixture.cache.parent().expect("parent").join(name);
    git(
        &fixture.main,
        &[
            "worktree",
            "add",
            "--quiet",
            "--track",
            "-b",
            name,
            &hand.to_string_lossy(),
            upstream,
        ],
    );
    hand
}

#[test]
fn prune_keeps_unpushed_branch_created_from_the_default_branch() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let fresh = tracking_checkout(&fixture, "fresh", "origin/main");
    assert_eq!(
        git(
            &fixture.main,
            &["for-each-ref", "--format=%(upstream)", "refs/heads/fresh"]
        ),
        "refs/remotes/origin/main"
    );
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert!(plan.externals.is_empty());
    assert_eq!(kept_reason(&plan, &fresh), Some("local"));
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(fresh.exists());
}

#[test]
fn prune_lists_detached_external_worktrees_as_removable_or_kept() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let old = detached_checkout(&fixture, "old", "origin/main");
    let fresh = detached_checkout(&fixture, "fresh", "origin/main");
    let ahead = detached_checkout(&fixture, "ahead", "origin/main");
    std::fs::write(ahead.join("a2.txt"), "a").expect("a2");
    git(&ahead, &["add", "."]);
    git(&ahead, &["commit", "--quiet", "-m", "local"]);
    let side = detached_checkout(&fixture, "side", "origin/main");
    std::fs::write(side.join("s.txt"), "s").expect("s");
    git(&side, &["add", "."]);
    git(&side, &["commit", "--quiet", "-m", "side"]);
    git(
        &side,
        &["push", "--quiet", "origin", "HEAD:refs/heads/side"],
    );
    let dirty = detached_checkout(&fixture, "dirty", "origin/main");
    std::fs::write(dirty.join("wip.txt"), "x").expect("wip");
    let roots = fixture.cache.join("bazel-root");
    let base = roots.join("old-base");
    std::fs::create_dir_all(&base).expect("base");
    std::fs::write(
        base.join("DO_NOT_BUILD_HERE"),
        old.to_string_lossy().as_bytes(),
    )
    .expect("marker");
    host.roots.lock().expect("roots").push(roots);
    let two_days_ago = host.now() - 2 * 24 * 60 * 60;
    for checkout in [&old, &ahead, &side, &dirty] {
        age_checkout(checkout, two_days_ago);
    }
    age_checkout(&fresh, host.now() - 60 * 60);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.externals.len(), 1, "{plan:?}");
    assert_eq!(plan.externals[0].path, old);
    assert_eq!(plan.externals[0].branch, None);
    for (path, reason) in [
        (&fresh, "recent"),
        (&ahead, "unpushed"),
        (&side, "unmerged"),
        (&dirty, "dirty"),
    ] {
        assert_eq!(kept_reason(&plan, path), Some(reason), "{}", path.display());
    }
    assert!(plan.kept_externals.iter().all(|item| item.branch.is_none()));
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(!old.exists());
    assert!(!base.exists());
    for path in [&fresh, &ahead, &side, &dirty] {
        assert!(path.exists(), "{}", path.display());
    }
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .externals
        .is_empty());
}

fn upstream_deleted_checkout(fixture: &Fixture) -> PathBuf {
    let hand = hand_checkout(fixture, "topic");
    std::fs::write(hand.join("t.txt"), "t").expect("t");
    git(&hand, &["add", "."]);
    git(&hand, &["commit", "--quiet", "-m", "topic"]);
    git(&hand, &["push", "--quiet", "-u", "origin", "topic"]);
    git(&hand, &["push", "--quiet", "origin", "topic:kept"]);
    git(
        &fixture.main,
        &["push", "--quiet", "origin", "--delete", "topic"],
    );
    git(&fixture.main, &["fetch", "--quiet", "--prune", "origin"]);
    hand
}

#[test]
fn prune_apply_keeps_external_worktree_that_turns_dirty_after_plan() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = upstream_deleted_checkout(&fixture);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.externals.len(), 1);
    std::fs::write(hand.join("wip.txt"), "x").expect("wip");
    let kept = ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock"))
        .expect("apply")
        .kept;
    assert!(hand.join("wip.txt").exists());
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].path, hand);
    assert_eq!(kept[0].branch.as_deref(), Some("topic"));
    assert_eq!(kept[0].reason, "dirty");
}

#[test]
fn prune_apply_keeps_external_worktree_that_gains_a_commit_after_plan() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = upstream_deleted_checkout(&fixture);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.externals.len(), 1);
    std::fs::write(hand.join("late.txt"), "l").expect("late");
    git(&hand, &["add", "."]);
    git(&hand, &["commit", "--quiet", "-m", "late"]);
    let kept = ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock"))
        .expect("apply")
        .kept;
    assert!(hand.join("late.txt").exists());
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].reason, "unpushed");
}

#[test]
fn prune_apply_keeps_external_worktree_that_starts_a_server_after_plan() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = upstream_deleted_checkout(&fixture);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.externals.len(), 1);
    host.servers.lock().expect("servers").push(Server {
        pid: 7,
        rss_bytes: 1,
        workspace: Some(hand.clone()),
        output_base: None,
        output_user_root: None,
    });
    let kept = ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock"))
        .expect("apply")
        .kept;
    assert!(hand.exists());
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].reason, "server");
}

#[test]
fn prune_apply_removes_external_worktree_still_clean_at_apply() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = upstream_deleted_checkout(&fixture);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    let kept = ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock"))
        .expect("apply")
        .kept;
    assert!(kept.is_empty());
    assert!(!hand.exists());
}

/// A free slot idle for 31 days with a base of its own, ready for `prune --apply`.
fn idle_slot_with_base(
    fixture: &Fixture,
    host: &FakeHost,
    pool: &Pool,
) -> (ops::Added, PathBuf, PathBuf) {
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    let me = who("s1", 10);
    let slot = ops::add(pool, host, &me, &create("feature/a")).expect("a");
    let base = output_base(&roots, "aaa", &slot.path);
    ops::release(pool, host, &me, &slot.path, true).expect("release");
    host.advance(31 * DAY);
    let path = slot.path.clone();
    (slot, path, base)
}

#[test]
fn prune_apply_skips_a_slot_that_cannot_be_deleted_as_busy() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let (slot, path, _base) = idle_slot_with_base(&fixture, &host, &pool);
    host.stick(&path);
    let pruned = ops::prune(&pool, &host, true, false).expect("prune");
    assert_eq!(pruned.skipped_slots.len(), 1);
    assert_eq!(pruned.skipped_slots[0].slot, slot.slot);
    assert_eq!(pruned.skipped_slots[0].reason, "busy");
    assert!(pruned.plan.slots.is_empty(), "a skipped slot is not pruned");
    assert_eq!(pruned.plan.bytes(), 0);
    let record = pool.record(slot.slot).expect("the record stays");
    assert!(record.broken && !record.held);
    let next = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(next.slots.len(), 1, "the next prune finds it again");
    host.unstick();
    let pruned = ops::prune(&pool, &host, true, false).expect("prune again");
    assert!(pruned.skipped_slots.is_empty());
    assert_eq!(pruned.plan.slots.len(), 1);
    assert!(!path.exists());
    assert!(pool.record(slot.slot).is_none());
}

#[test]
fn add_never_leases_a_slot_that_failed_to_delete() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let (slot, path, _base) = idle_slot_with_base(&fixture, &host, &pool);
    host.stick(&path);
    ops::prune(&pool, &host, true, false).expect("prune");
    assert!(pool.record(slot.slot).expect("record").broken);
    let me = who("s2", 11);
    let again = ops::add(&pool, &host, &me, &create("feature/a2")).expect("add");
    assert_ne!(again.slot, slot.slot, "the broken slot is not leased");
    assert!(pool.record(slot.slot).expect("record").broken);
    host.unstick();
    let other = ops::add(&pool, &host, &me, &create("feature/b")).expect("add after");
    assert_ne!(other.slot, slot.slot);
    assert!(!path.exists(), "add deletes the broken slot once it can");
    assert!(pool.record(slot.slot).is_none());
}

#[test]
fn prune_apply_prunes_a_slot_and_skips_its_stuck_output_base_as_orphaned() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let (slot, path, base) = idle_slot_with_base(&fixture, &host, &pool);
    host.stick(&base);
    let pruned = ops::prune(&pool, &host, true, false).expect("prune");
    assert!(pruned.skipped_slots.is_empty());
    assert_eq!(pruned.plan.slots.len(), 1);
    assert_eq!(pruned.plan.bytes(), 4096, "the stuck base is not counted");
    assert_eq!(pruned.skipped_bases.len(), 1);
    assert_eq!(pruned.skipped_bases[0].dir, base);
    assert_eq!(pruned.skipped_bases[0].owner, ops::BaseOwner::Orphan);
    assert_eq!(pruned.skipped_bases[0].reason, "busy");
    assert!(!path.exists());
    assert!(pool.record(slot.slot).is_none());
    assert!(base.exists());
    let next = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(next.bases.len(), 1, "the base is an orphan now");
    assert_eq!(next.bases[0].dir, base);
}

#[test]
fn prune_apply_skips_stuck_bases_as_busy() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    host.script_disk(&fixture.cache, OK);
    let me = who("s1", 10);
    let slot = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let owned = output_base(&roots, "aaa", &slot.path);
    let orphan = output_base(&roots, "bbb", &fixture.cache.join("gone"));
    ops::release(&pool, &host, &me, &slot.path, true).expect("release");
    host.stick(&owned);
    host.stick(&orphan);
    let pruned = ops::prune(&pool, &host, true, true).expect("prune");
    assert_eq!(pruned.skipped_bases.len(), 2);
    assert!(pruned
        .skipped_bases
        .iter()
        .all(|base| base.reason == "busy"));
    assert!(pruned
        .skipped_bases
        .iter()
        .any(|base| base.owner == ops::BaseOwner::Orphan));
    assert!(pruned.plan.bases.is_empty());
    assert_eq!(pruned.plan.bytes(), 0);
    assert!(owned.exists() && orphan.exists());
}

#[test]
fn prune_apply_keeps_a_stuck_external_worktree_as_busy() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = upstream_deleted_checkout(&fixture);
    host.stick(&hand);
    let pruned = ops::prune(&pool, &host, true, false).expect("prune");
    assert_eq!(pruned.kept.len(), 1);
    assert_eq!(pruned.kept[0].reason, "busy");
    assert!(pruned.plan.externals.is_empty());
    assert_eq!(pruned.plan.bytes(), 0);
}

#[test]
fn prune_apply_skips_a_slot_taken_after_the_plan_as_held() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let (slot, path, _base) = idle_slot_with_base(&fixture, &host, &pool);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.slots.len(), 1);
    let taken = ops::add(&pool, &host, &who("s2", 11), &create("feature/b")).expect("take");
    assert_eq!(taken.slot, slot.slot);
    let applied =
        ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert_eq!(applied.skipped_slots.len(), 1);
    assert_eq!(applied.skipped_slots[0].reason, "held");
    assert!(path.exists());
}

#[test]
fn list_skips_disk_unless_asked() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let added = ops::add(&pool, &host, &me, &create("feature/a")).expect("add");
    let roots = fixture.cache.join("bazel-root");
    let base = roots.join("aaa");
    std::fs::create_dir_all(&base).expect("base");
    std::fs::write(
        base.join("DO_NOT_BUILD_HERE"),
        added.path.to_string_lossy().as_bytes(),
    )
    .expect("marker");
    host.roots.lock().expect("roots").push(roots);
    let hand = hand_checkout(&fixture, "by-hand");
    host.disk_calls.lock().expect("disk").clear();
    let scans = host.root_scans.load(Ordering::SeqCst);

    let quick = ops::list(&pool, &host, false).expect("list");
    assert!(host.disk_paths().is_empty(), "list runs no du by default");
    assert_eq!(host.root_scans.load(Ordering::SeqCst), scans);
    assert_eq!(quick.rows.len(), 2);
    assert!(quick.rows.iter().all(|row| row.disk_bytes.is_none()));
    assert_eq!(quick.disk_bytes, None);

    let measured = ops::list(&pool, &host, true).expect("list --disk");
    let paths = host.disk_paths();
    assert!(paths.contains(&added.path));
    assert!(paths.contains(&base));
    assert!(paths.contains(&hand));
    let slot = measured
        .rows
        .iter()
        .find(|row| row.slot == Some(added.slot))
        .expect("slot row");
    assert_eq!(slot.disk_bytes, Some(8192), "slot plus its output base");
    let external = measured
        .rows
        .iter()
        .find(|row| row.state == State::External)
        .expect("external row");
    assert_eq!(external.disk_bytes, Some(4096));
    assert_eq!(measured.disk_bytes, Some(8192));
}

#[cfg(unix)]
fn physical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).expect("canonical")
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_still_finds_the_slot_output_base_and_prune_deletes_it() {
    let fixture = Fixture::linked();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    let kept = roots.join("aaa");
    std::fs::create_dir_all(&kept).expect("kept");
    host.roots.lock().expect("roots").push(roots);
    let me = who("s1", 10);
    let first = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    let real = physical(&first.path);
    assert_ne!(real, first.path);
    std::fs::write(
        kept.join("DO_NOT_BUILD_HERE"),
        real.to_string_lossy().as_bytes(),
    )
    .expect("kept marker");
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert!(plan.bases.is_empty(), "{:?}", plan.bases);
    ops::release(&pool, &host, &me, &first.path, true).expect("release");
    host.advance(31 * DAY);
    host.disk_calls.lock().expect("disk").clear();
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.slots.len(), 1);
    assert_eq!(plan.slots[0].bytes, 8192, "slot plus its output base");
    assert!(host.disk_paths().contains(&kept));
    ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert!(!first.path.exists());
    assert!(!real.exists());
    assert!(!kept.exists(), "the slot's output base goes with it");
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_counts_the_output_base_in_list_disk() {
    let fixture = Fixture::linked();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    let roots = fixture.cache.join("bazel-root");
    let base = roots.join("aaa");
    std::fs::create_dir_all(&base).expect("base");
    std::fs::write(
        base.join("DO_NOT_BUILD_HERE"),
        physical(&added.path).to_string_lossy().as_bytes(),
    )
    .expect("marker");
    host.roots.lock().expect("roots").push(roots);
    let listing = ops::list(&pool, &host, true).expect("list --disk");
    let slot = listing
        .rows
        .iter()
        .find(|row| row.slot == Some(added.slot))
        .expect("slot row");
    assert_eq!(slot.disk_bytes, Some(8192), "slot plus its output base");
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_gives_the_same_session_its_branch_back() {
    let fixture = Fixture::linked();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(10);
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    let again = ops::add(&pool, &host, &who("s1", 10), &existing("feature/a")).expect("again");
    assert!(again.reused);
    assert_eq!(again.path, added.path);
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("held");
    assert_eq!(refused.code, "branch_held");
    assert!(refused.message.contains("slot 1"), "{}", refused.message);
    host.alive.lock().expect("alive").clear();
    let taken = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect("takeover");
    assert_eq!(taken.path, added.path);
}

#[cfg(unix)]
#[test]
fn a_symlinked_cache_sees_the_server_of_a_slot() {
    let fixture = Fixture::linked();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let added = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    host.servers.lock().expect("servers").push(Server {
        pid: 7,
        rss_bytes: 1,
        workspace: Some(physical(&added.path)),
        output_base: None,
        output_user_root: None,
    });
    let listing = ops::list(&pool, &host, false).expect("list");
    let slot = listing
        .rows
        .iter()
        .find(|row| row.slot == Some(added.slot))
        .expect("slot row");
    assert!(slot.server);
}

#[test]
fn list_reports_the_shared_disk_cache_size_with_disk() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let cache = fixture.cache.join("dx").join("disk-cache");
    *host.disk_cache.lock().expect("disk cache") = Some(cache.clone());
    assert_eq!(
        ops::list(&pool, &host, false)
            .expect("list")
            .disk_cache_bytes,
        None
    );
    assert_eq!(
        ops::list(&pool, &host, true)
            .expect("list")
            .disk_cache_bytes,
        Some(0)
    );
    std::fs::create_dir_all(&cache).expect("cache dir");
    host.disk_calls.lock().expect("disk").clear();
    assert_eq!(
        ops::list(&pool, &host, true)
            .expect("list")
            .disk_cache_bytes,
        Some(4096)
    );
    assert!(host.disk_paths().contains(&cache));
    *host.disk_cache.lock().expect("disk cache") = None;
    assert_eq!(
        ops::list(&pool, &host, true)
            .expect("list")
            .disk_cache_bytes,
        Some(0)
    );
}

#[test]
fn disk_cache_bytes_is_zero_for_a_missing_directory_and_measured_otherwise() {
    let fixture = Fixture::new();
    let host = FakeHost::new();
    let cache = fixture.cache.join("dx").join("disk-cache");
    *host.disk_cache.lock().expect("disk cache") = Some(cache.clone());
    assert_eq!(ops::disk_cache_bytes(&host).expect("bytes"), 0);
    std::fs::create_dir_all(&cache).expect("cache dir");
    assert_eq!(ops::disk_cache_bytes(&host).expect("bytes"), 4096);
}

fn two_stale_slots(pool: &Pool, host: &FakeHost) -> Vec<PathBuf> {
    host.live(10);
    host.live(11);
    let holders = [who("s1", 10), who("s2", 11)];
    let mut paths = Vec::new();
    for (holder, branch) in holders.iter().zip(["feature/a", "feature/b"]) {
        let added = ops::add(pool, host, holder, &create(branch)).expect("add");
        paths.push(added.path);
    }
    assert_ne!(paths[0], paths[1]);
    for (holder, path) in holders.iter().zip(&paths) {
        ops::release(pool, host, holder, path, true).expect("release");
    }
    host.advance(31 * DAY);
    paths
}

#[test]
fn list_disk_stops_at_the_next_measure_after_a_signal() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    two_stale_slots(&pool, &host);
    host.stop_after(1, 15);
    let error = ops::list(&pool, &host, true).expect_err("stopped");
    assert_eq!(error.signal, Some(15));
    assert_eq!(error.code, "cancelled");
    assert_eq!(error.exit(), 143);
    assert_eq!(host.disk_paths().len(), 1);
}

#[test]
fn list_disk_measures_nothing_when_a_signal_came_first() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    two_stale_slots(&pool, &host);
    host.stop_after(0, 15);
    let error = ops::list(&pool, &host, true).expect_err("stopped");
    assert_eq!(error.signal, Some(15));
    assert!(host.disk_paths().is_empty());
}

#[test]
fn prune_plan_stops_measuring_after_a_signal() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    two_stale_slots(&pool, &host);
    host.stop_after(1, 15);
    let error = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect_err("stopped");
    assert_eq!(error.signal, Some(15));
    assert_eq!(error.exit(), 143);
    assert_eq!(host.disk_paths().len(), 1);
    host.disk_calls.lock().expect("disk").clear();
    host.stop_after(0, 2);
    let error = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect_err("stopped");
    assert_eq!(error.signal, Some(2));
    assert!(host.disk_paths().is_empty());
}

#[test]
fn prune_apply_deletes_nothing_after_a_signal() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let paths = two_stale_slots(&pool, &host);
    let roots = fixture.cache.join("bazel-root");
    let base = output_base(&roots, "slot-base", &paths[0]);
    host.roots.lock().expect("roots").push(roots);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.slots.len(), 2);
    host.stop_after(0, 15);
    let error =
        ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect_err("stopped");
    assert_eq!(error.signal, Some(15));
    assert!(paths.iter().all(|path| path.exists()));
    assert!(base.exists());
    drop(pool.lock().expect("lock is free again"));
}

fn set_time(path: &Path, secs: u64) {
    std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open")
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs))
        .expect("set time");
}

fn downloads_of(roots: &Path) -> PathBuf {
    roots
        .join("cache")
        .join("repos")
        .join("v1")
        .join("content_addressable")
}

fn download(roots: &Path, key: &str, files: &[(&str, u64)]) -> PathBuf {
    let dir = downloads_of(roots).join("sha256").join(key);
    std::fs::create_dir_all(&dir).expect("entry");
    for (name, secs) in files {
        std::fs::write(dir.join(name), vec![0u8; 10]).expect("file");
        set_time(&dir.join(name), *secs);
    }
    dir
}

/// A host whose clock is 40 days past the time `STALE` files carry.
fn download_host(fixture: &Fixture) -> (FakeHost, PathBuf) {
    let host = FakeHost::new();
    let roots = fixture.cache.join("bazel-root");
    host.roots.lock().expect("roots").push(roots.clone());
    host.advance(40 * DAY);
    (host, roots)
}

const STALE: u64 = 1_000_000;

fn entry_dirs(plan: &ops::PrunePlan) -> Vec<PathBuf> {
    plan.downloads
        .iter()
        .flat_map(|downloads| downloads.entries.iter().map(|entry| entry.dir.clone()))
        .collect()
}

#[test]
fn prune_lists_downloads_unused_for_30_days_and_apply_deletes_them() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, roots) = download_host(&fixture);
    let old = download(&roots, "old", &[("file", STALE)]);
    let recent = download(&roots, "recent", &[("file", host.now() - 29 * DAY)]);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(plan.downloads.len(), 1);
    assert_eq!(plan.downloads[0].cache, downloads_of(&roots));
    assert_eq!(plan.downloads[0].bytes, 10);
    assert_eq!(entry_dirs(&plan), vec![old.clone()]);
    assert_eq!(plan.bytes(), 10);
    assert!(!plan.is_empty());
    let pruned = ops::prune(&pool, &host, true, false).expect("apply");
    assert_eq!(entry_dirs(&pruned.plan), vec![old.clone()]);
    assert!(!old.exists());
    assert!(recent.exists(), "a download used within 30 days stays");
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .is_empty());
}

#[test]
fn prune_apply_keeps_download_touched_after_plan() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, roots) = download_host(&fixture);
    let old = download(&roots, "old", &[("file", STALE)]);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(entry_dirs(&plan), vec![old.clone()]);
    set_time(&old.join("file"), host.now());
    let applied =
        ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect("apply");
    assert_eq!(applied.used_downloads, vec![old.clone()]);
    assert!(old.join("file").exists());
}

#[test]
fn download_with_a_fresh_tmp_file_is_not_stale() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, roots) = download_host(&fixture);
    let now = host.now();
    let writing = download(&roots, "writing", &[("file", STALE), ("tmp-1", now)]);
    let reading = download(&roots, "reading", &[("file", STALE), ("id-x", now)]);
    std::fs::create_dir_all(downloads_of(&roots).join("sha256").join("empty")).expect("empty");
    assert!(ops::prune_plan(&pool, &host, ops::Trigger::LowDisk)
        .expect("plan")
        .is_empty());
    ops::prune(&pool, &host, true, false).expect("apply");
    assert!(writing.join("file").exists());
    assert!(reading.join("file").exists());
    assert!(downloads_of(&roots).join("sha256").join("empty").exists());
}

#[test]
fn reclaim_drops_stale_downloads_before_free_slot_bases() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, roots) = download_host(&fixture);
    host.live(10);
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    let base = output_base(&roots, "a", &a.path);
    let oldest = download(&roots, "zzz", &[("file", STALE)]);
    let older = download(&roots, "aaa", &[("file", STALE + DAY)]);
    let recent = download(&roots, "new", &[("file", host.now())]);
    host.script_space(&[LOW, LOW, OK]);
    let reclaimed = ops::reclaim_if_low(&pool, &host, None)
        .expect("reclaim")
        .expect("low");
    assert!(reclaimed.removed);
    assert!(
        !oldest.exists(),
        "the least recently used download goes first"
    );
    assert!(older.exists(), "reclaim stops once disk recovers");
    assert!(recent.exists());
    assert!(base.exists(), "the free slot keeps its output base");
    assert_eq!(
        reclaimed.warning.message,
        "low disk: 5.0 GiB free; removed 1 repository cache download(s); 50.0 GiB free now"
    );
}

#[test]
fn prune_never_touches_repo_contents_cache() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, roots) = download_host(&fixture);
    let contents = roots
        .join("cache")
        .join("repos")
        .join("v1")
        .join("contents")
        .join("abc");
    std::fs::create_dir_all(&contents).expect("contents");
    std::fs::write(contents.join("file"), "x").expect("file");
    set_time(&contents.join("file"), STALE);
    let old = download(&roots, "old", &[("file", STALE)]);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(entry_dirs(&plan), vec![old.clone()]);
    host.script_space(&[LOW]);
    ops::reclaim_if_low(&pool, &host, None).expect("reclaim");
    assert!(!old.exists());
    assert!(contents.join("file").exists());
    host.script_space(&[OK]);
    download(&roots, "again", &[("file", STALE)]);
    ops::prune(&pool, &host, true, false).expect("apply");
    assert!(!downloads_of(&roots).join("sha256").join("again").exists());
    assert!(contents.join("file").exists());
}

#[test]
fn prune_apply_deletes_no_downloads_after_a_signal() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, roots) = download_host(&fixture);
    let old = download(&roots, "old", &[("file", STALE)]);
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect("plan");
    assert_eq!(entry_dirs(&plan), vec![old.clone()]);
    host.stop_after(0, 15);
    let error =
        ops::prune_apply(&pool, &host, &plan, &pool.lock().expect("lock")).expect_err("stopped");
    assert_eq!(error.signal, Some(15));
    let error = ops::prune_plan(&pool, &host, ops::Trigger::LowDisk).expect_err("stopped");
    assert_eq!(error.signal, Some(15));
    assert!(old.exists());
}

/// A host whose nested-Bazel root is under the fixture, with the clock far enough on to age files by days.
fn nested_host(fixture: &Fixture) -> (FakeHost, PathBuf) {
    let host = FakeHost::new();
    host.advance(400 * DAY);
    let root = fixture.cache.with_file_name("dxnb");
    std::fs::create_dir_all(&root).expect("nested root");
    *host.nested.lock().expect("nested") = Some(root.clone());
    (host, root)
}

/// A nested-Bazel slot directory under `root`, with the lock file the harness creates.
fn nested_slot(root: &Path, slot: &str) -> PathBuf {
    let dir = root.join(slot);
    std::fs::create_dir_all(&dir).expect("slot");
    std::fs::File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("lock"))
        .expect("lock file");
    dir
}

/// One test case of a nested slot: its workspace under `w` and its output base under `u`.
fn nested_case(slot: &Path, case: &str, base: &str) -> (PathBuf, PathBuf) {
    let workspace = slot.join("w").join(case);
    std::fs::create_dir_all(&workspace).expect("workspace");
    std::fs::write(workspace.join("MODULE.bazel"), "").expect("module");
    let dir = output_base(&slot.join("u"), base, &workspace);
    (dir, workspace)
}

/// Sets a workspace's last change to `days` before the fake clock.
fn age(host: &FakeHost, workspace: &Path, days: u64) {
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(host.now() - days * DAY);
    std::fs::File::open(workspace)
        .expect("open workspace")
        .set_modified(when)
        .expect("set mtime");
}

/// Waits for, then holds, a nested slot's lock the way a running test does.
fn hold_nested(slot: &Path) -> std::fs::File {
    crate::test_support::eventually(
        "nested slot lock",
        std::time::Duration::from_secs(30),
        || {
            let file = std::fs::File::options()
                .write(true)
                .open(slot.join("lock"))
                .expect("lock");
            file.try_lock().ok().map(|()| file)
        },
    )
}

#[test]
fn hold_nested_waits_for_a_released_lock_still_held_by_a_dup() {
    let fixture = Fixture::new();
    let (_host, root) = nested_host(&fixture);
    let slot = nested_slot(&root, "0");
    let held = hold_nested(&slot);
    let dup = held.try_clone().expect("dup");
    drop(held);
    let probe = std::fs::File::options()
        .write(true)
        .open(slot.join("lock"))
        .expect("lock");
    assert!(probe.try_lock().is_err(), "the dup keeps the lock");
    drop(probe);
    let releaser = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        drop(dup);
    });
    drop(hold_nested(&slot));
    releaser.join().expect("releaser");
}

fn nested_dirs(plan: &ops::PrunePlan) -> Vec<PathBuf> {
    plan.bases
        .iter()
        .filter(|base| matches!(base.owner, ops::BaseOwner::Nested { .. }))
        .map(|base| base.dir.clone())
        .collect()
}

#[test]
fn prune_lists_nested_cases_idle_for_30_days_and_apply_deletes_them() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let slot = nested_slot(&root, "0");
    let (old, old_ws) = nested_case(&slot, "old", "aaa");
    let (fresh, fresh_ws) = nested_case(&slot, "fresh", "bbb");
    age(&host, &old_ws, 31);
    age(&host, &fresh_ws, 29);
    // A forked child can briefly keep the lock.
    let dry = crate::test_support::eventually(
        "nested slot idle for planning",
        std::time::Duration::from_secs(30),
        || {
            let dry = ops::prune(&pool, &host, false, false).expect("dry");
            (!dry.plan.bases.is_empty()).then_some(dry)
        },
    );
    assert_eq!(dry.plan.bases.len(), 1, "{:?}", dry.plan.bases);
    assert_eq!(dry.plan.bases[0].dir, old);
    assert_eq!(
        dry.plan.bases[0].owner,
        ops::BaseOwner::Nested {
            slot: slot.clone(),
            case: "old".to_owned(),
        }
    );
    assert_eq!(dry.plan.bases[0].bytes, 2 * 4096, "base and workspace");
    assert!(old.exists(), "a dry run deletes nothing");
    let applied = crate::test_support::eventually(
        "nested slot idle for apply",
        std::time::Duration::from_secs(30),
        || {
            // A busy slot at planning time drops the base from the plan, so
            // an empty skip list alone does not mean the base was deleted.
            let applied = ops::prune(&pool, &host, true, false).expect("apply");
            (!applied.plan.bases.is_empty() && applied.skipped_bases.is_empty()).then_some(applied)
        },
    );
    assert!(applied.skipped_bases.is_empty());
    assert!(!old.exists() && !old_ws.exists());
    assert!(fresh.exists() && fresh_ws.exists());
    assert!(slot.join("lock").exists(), "the slot keeps its lock file");
    drop(hold_nested(&slot));
}

#[test]
fn prune_skips_a_nested_slot_a_test_holds() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let slot = nested_slot(&root, "0");
    let (base, workspace) = nested_case(&slot, "old", "aaa");
    age(&host, &workspace, 90);
    let _test = hold_nested(&slot);
    let pruned = ops::prune(&pool, &host, true, true).expect("prune");
    assert!(pruned.plan.bases.is_empty(), "{:?}", pruned.plan.bases);
    assert!(pruned.skipped_bases.is_empty());
    assert!(base.exists() && workspace.exists());
}

#[test]
fn prune_bases_lists_every_nested_case_of_free_slots_oldest_first() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let zero = nested_slot(&root, "0");
    let one = nested_slot(&root, "1");
    let (newer, newer_ws) = nested_case(&zero, "newer", "aaa");
    let (older, older_ws) = nested_case(&one, "older", "bbb");
    let (middle, middle_ws) = nested_case(&zero, "middle", "ccc");
    age(&host, &newer_ws, 1);
    age(&host, &older_ws, 3);
    age(&host, &middle_ws, 2);
    let plain = ops::prune(&pool, &host, false, false).expect("plain");
    assert!(nested_dirs(&plain.plan).is_empty(), "fresh cases stay");
    let dry = ops::prune(&pool, &host, false, true).expect("dry");
    assert_eq!(
        nested_dirs(&dry.plan),
        vec![older.clone(), middle.clone(), newer.clone()]
    );
    ops::prune(&pool, &host, true, true).expect("apply");
    for path in [&older, &middle, &newer, &older_ws, &middle_ws, &newer_ws] {
        assert!(!path.exists(), "{}", path.display());
    }
}

#[test]
fn low_disk_on_the_nested_root_alone_lists_its_cases() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    host.script_disk(&fixture.cache, OK);
    let slot = nested_slot(&root, "0");
    let (base, _) = nested_case(&slot, "fresh", "aaa");
    let plain = ops::prune(&pool, &host, false, false).expect("plenty");
    assert!(nested_dirs(&plain.plan).is_empty());
    host.script_disk(&root, LOW);
    let low = ops::prune(&pool, &host, false, false).expect("low");
    assert_eq!(nested_dirs(&low.plan), vec![base]);
}

#[test]
fn prune_apply_skips_a_nested_slot_locked_after_the_plan() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let slot = nested_slot(&root, "0");
    let (base, workspace) = nested_case(&slot, "case", "aaa");
    let plan = ops::prune_plan(&pool, &host, ops::Trigger::OnDemand).expect("plan");
    assert_eq!(nested_dirs(&plan), vec![base.clone()]);
    let _test = hold_nested(&slot);
    let lock = pool.lock().expect("pool lock");
    let applied = ops::prune_apply(&pool, &host, &plan, &lock).expect("apply");
    assert_eq!(applied.skipped_bases.len(), 1);
    assert_eq!(applied.skipped_bases[0].reason, "busy");
    assert_eq!(applied.skipped_bases[0].dir, base);
    assert!(base.exists() && workspace.exists());
}

#[test]
fn prune_keeps_a_nested_base_whose_server_still_runs() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let slot = nested_slot(&root, "0");
    let (base, workspace) = nested_case(&slot, "case", "aaa");
    host.servers.lock().expect("servers").push(Server {
        pid: 42,
        rss_bytes: 0,
        workspace: Some(workspace.clone()),
        output_base: Some(base.clone()),
        output_user_root: Some(slot.join("u")),
    });
    let pruned = ops::prune(&pool, &host, true, true).expect("prune");
    assert!(pruned.plan.bases.is_empty(), "{:?}", pruned.plan.bases);
    assert_eq!(pruned.plan.kept_bases.len(), 1);
    assert_eq!(pruned.plan.kept_bases[0].reason, "server");
    assert_eq!(pruned.plan.kept_bases[0].dir, base);
    assert!(base.exists() && workspace.exists());
    assert_eq!(host.shutdown_count(), 0, "nested servers stop on their own");
}

#[test]
fn nested_bases_are_never_listed_as_orphans() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let slot = nested_slot(&root, "0");
    let (base, workspace) = nested_case(&slot, "case", "aaa");
    std::fs::remove_dir_all(&workspace).expect("drop workspace");
    host.roots.lock().expect("roots").push(slot.join("u"));
    let test = hold_nested(&slot);
    let held = ops::prune(&pool, &host, true, false).expect("held");
    assert!(held.plan.bases.is_empty(), "{:?}", held.plan.bases);
    assert!(base.exists(), "a held slot's base is not an orphan");
    drop(test);
    drop(hold_nested(&slot));
    let free = ops::prune(&pool, &host, false, false).expect("free");
    assert_eq!(free.plan.bases.len(), 1, "{:?}", free.plan.bases);
    assert!(matches!(
        free.plan.bases[0].owner,
        ops::BaseOwner::Nested { .. }
    ));
}

#[test]
fn prune_creates_no_lock_file_in_a_nested_slot_without_one() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    let slot = root.join("0");
    let (base, workspace) = nested_case(&slot, "case", "aaa");
    age(&host, &workspace, 90);
    let pruned = ops::prune(&pool, &host, true, true).expect("prune");
    assert!(pruned.plan.bases.is_empty(), "{:?}", pruned.plan.bases);
    assert!(!slot.join("lock").exists());
    assert!(base.exists());
}

#[test]
fn reclaim_drops_nested_cases_when_the_nested_root_disk_is_low() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let (host, root) = nested_host(&fixture);
    host.script_disk(&fixture.cache, OK);
    host.script_disk(&root, ROOT_LOW);
    let zero = nested_slot(&root, "0");
    let one = nested_slot(&root, "1");
    let (free, free_ws) = nested_case(&zero, "case", "aaa");
    let (busy, _) = nested_case(&one, "case", "bbb");
    let _test = hold_nested(&one);
    let reclaimed = ops::reclaim_if_low(&pool, &host, None)
        .expect("reclaim")
        .expect("low");
    assert!(reclaimed.removed);
    assert!(!free.exists() && !free_ws.exists());
    assert!(busy.exists(), "a held nested slot keeps its cases");
    assert_eq!(
        reclaimed.warning.message,
        "low disk: 3.0 GiB free; removed 1 nested Bazel output base(s); 3.0 GiB free now; run dx worktree prune --apply"
    );
}
