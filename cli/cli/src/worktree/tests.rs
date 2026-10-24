use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use super::host::{Host, Server};
use super::ops::{self, AddRequest, State};
use super::pool::{Pool, Who};

const DAY: u64 = 24 * 60 * 60;

struct FakeHost {
    clock: AtomicU64,
    alive: Mutex<BTreeSet<u32>>,
    shutdowns: Mutex<Vec<PathBuf>>,
    free_percent: Mutex<Option<f64>>,
    roots: Mutex<Vec<PathBuf>>,
    servers: Mutex<Vec<Server>>,
}

impl FakeHost {
    fn new() -> FakeHost {
        FakeHost {
            clock: AtomicU64::new(1_000_000),
            alive: Mutex::new(BTreeSet::new()),
            shutdowns: Mutex::new(Vec::new()),
            free_percent: Mutex::new(None),
            roots: Mutex::new(Vec::new()),
            servers: Mutex::new(Vec::new()),
        }
    }

    fn advance(&self, seconds: u64) {
        self.clock.fetch_add(seconds, Ordering::SeqCst);
    }

    fn live(&self, pid: u32) {
        self.alive.lock().expect("alive").insert(pid);
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

    fn free_disk_percent(&self, _path: &Path) -> Option<f64> {
        *self.free_percent.lock().expect("free")
    }

    fn servers(&self) -> Vec<Server> {
        self.servers.lock().expect("servers").clone()
    }

    fn output_roots(&self) -> Vec<PathBuf> {
        self.roots.lock().expect("roots").clone()
    }

    fn bazel_shutdown(&self, workspace: &Path) {
        self.shutdowns
            .lock()
            .expect("shutdowns")
            .push(workspace.to_path_buf());
    }

    fn disk_bytes(&self, _path: &Path) -> u64 {
        0
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

    fn pool(&self) -> Pool {
        Pool::open(&self.main, &self.cache).expect("pool opens")
    }
}

fn who(session: &str, pid: u32) -> Who {
    Who {
        session: Some(session.to_owned()),
        pid,
    }
}

fn create(branch: &str) -> AddRequest {
    AddRequest {
        branch: branch.to_owned(),
        create: Some("origin/main".to_owned()),
    }
}

fn existing(branch: &str) -> AddRequest {
    AddRequest {
        branch: branch.to_owned(),
        create: None,
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
    assert!(added.path.starts_with(&fixture.cache));
    assert!(!added.path.starts_with(&fixture.main));
    assert_eq!(git(&added.path, &["branch", "--show-current"]), "feature/a");
    assert!(added.path.join("a.txt").is_file());
    let listing = ops::list(&pool, &host).expect("list");
    let row = listing
        .rows
        .iter()
        .find(|row| row.slot == Some(1))
        .expect("slot row");
    assert_eq!(row.state, State::Held);
    assert_eq!(row.branch.as_deref(), Some("feature/a"));
    assert_eq!(ops::path_of(&pool, "feature/a").expect("path"), added.path);
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
    let row = ops::list(&pool, &host)
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
    assert_eq!(again.path, added.path);
    let refused = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect_err("held");
    assert_eq!(refused.code, "branch_held");
    assert!(refused.message.contains("slot 1"), "{}", refused.message);
    assert!(refused.message.contains("s1"), "{}", refused.message);
}

#[test]
fn an_existing_branch_is_needed_without_create() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let refused =
        ops::add(&pool, &host, &who("s1", 10), &existing("nope")).expect_err("unknown branch");
    assert!(refused.usage);
    assert_eq!(refused.code, "unknown_branch");
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
    let listing = ops::list(&pool, &host).expect("list");
    assert_eq!(slot_one(&listing).state, State::Reclaimable);
    let second = ops::add(&pool, &host, &who("s2", 20), &existing("feature/a")).expect("second");
    assert_eq!(second.slot, 1);
    assert!(!second.reused);
    assert_eq!(
        git(&second.path, &["branch", "--show-current"]),
        "feature/a"
    );
    assert!(host.shutdown_count() >= 1);
    let after = ops::list(&pool, &host).expect("list");
    assert_eq!(slot_one(&after).state, State::Held);
    assert!(slot_one(&after)
        .holder
        .as_deref()
        .is_some_and(|holder| holder.contains("s2")));
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
    let listing = ops::list(&pool, &host).expect("list");
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
    let listing = ops::list(&pool, &host).expect("list");
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
    let listing = ops::list(&pool, &host).expect("list");
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
    let listing = ops::list(&pool, &host).expect("list");
    let row = listing
        .rows
        .iter()
        .find(|row| row.branch.as_deref() == Some("by-hand"))
        .expect("hand row");
    assert_eq!(row.state, State::External);
    assert_eq!(row.slot, None);
    assert_eq!(ops::path_of(&pool, "by-hand").expect("path"), hand);
    let refused =
        ops::add(&pool, &host, &who("s1", 10), &existing("by-hand")).expect_err("checked out");
    assert_eq!(refused.code, "branch_held");
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
fn low_disk_reuses_the_oldest_free_slot_and_otherwise_the_newest() {
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
    *host.free_percent.lock().expect("free") = Some(5.0);
    let oldest = ops::add(&pool, &host, &me, &create("feature/d")).expect("d");
    assert_eq!(oldest.slot, a.slot, "oldest first when disk is low");
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
    let plan = ops::prune_plan(&pool, &host);
    assert_eq!(plan.bases.len(), 1);
    assert_eq!(plan.bases[0].dir, orphan);
    ops::prune_apply(&pool, &host, &plan).expect("apply");
    assert!(!orphan.exists());
    assert!(kept.exists());
    ops::release(&pool, &host, &me, &second.path, true).expect("release b");
    host.advance(31 * DAY);
    let plan = ops::prune_plan(&pool, &host);
    ops::prune_apply(&pool, &host, &plan).expect("apply slots");
    assert!(!second.path.exists());
    assert!(!kept.exists(), "the slot's output base goes with it");
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
    let plan = ops::prune_plan(&pool, &host);
    assert_eq!(plan.bases.len(), 1);
    assert_eq!(plan.bases[0].dir, roots.join("gone"));
    assert_eq!(plan.bases[0].workspace, gone);
    ops::prune_apply(&pool, &host, &plan).expect("apply");
    assert!(!roots.join("gone").exists());
    assert!(roots.join("live").exists());
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
    let plan = ops::prune_plan(&pool, &host);
    assert_eq!(plan.slots.len(), 1);
    let _held = pool.lock().expect("lock");
    let refused = ops::prune_apply(&pool, &host, &plan);
    assert_eq!(refused.expect_err("busy").code, "pool_busy");
    assert!(first.path.exists());
}

#[test]
fn prune_lists_old_free_slots_and_apply_deletes_them() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let me = who("s1", 10);
    let a = ops::add(&pool, &host, &me, &create("feature/a")).expect("a");
    ops::release(&pool, &host, &me, &a.path, true).expect("release");
    assert!(ops::prune_plan(&pool, &host).is_empty());
    host.advance(31 * DAY);
    let plan = ops::prune_plan(&pool, &host);
    assert_eq!(plan.slots.len(), 1);
    assert_eq!(plan.slots[0].idle_days, 31);
    assert!(a.path.exists());
    ops::prune_apply(&pool, &host, &plan).expect("apply");
    assert!(!a.path.exists());
    assert!(pool.records().is_empty());
    assert!(ops::prune_plan(&pool, &host).is_empty());
}

#[test]
fn new_branches_start_from_the_default_branch_of_origin() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let origin_clone = fixture.cache.parent().expect("parent").join("other");
    git(
        fixture.cache.parent().expect("parent"),
        &[
            "clone",
            "--quiet",
            &fixture
                .cache
                .parent()
                .expect("parent")
                .join("origin.git")
                .to_string_lossy(),
            &origin_clone.to_string_lossy(),
        ],
    );
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
            create: Some(String::new()),
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

    #[test]
    fn guard_blocks_git_worktree_add_remove_and_prune_only() {
        let block =
            |command: &str| hook::guard(&serde_json::json!({"tool_input": {"command": command}}));
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
        assert!(hook::guard(&serde_json::json!({})).is_none());
    }

    #[test]
    fn guard_parses_words_and_ignores_heredocs_and_quotes() {
        let block =
            |command: &str| hook::guard(&serde_json::json!({"tool_input": {"command": command}}));
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
        let block =
            |command: &str| hook::guard(&serde_json::json!({"tool_input": {"command": command}}));
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
        let block =
            |command: &str| hook::guard(&serde_json::json!({"tool_input": {"command": command}}));
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
        let path = hook::create(&pool, &host, &me, &input).expect("create");
        assert!(path.starts_with(&fixture.cache));
        assert_eq!(
            git(&path, &["branch", "--show-current"]),
            "worktree-agent-one"
        );
        let removal = serde_json::json!({"worktree_path": path});
        let freed = hook::remove(&pool, &host, &me, &removal).expect("remove");
        assert_eq!(freed, Some(path.clone()));
        let again = hook::create(&pool, &host, &me, &input).expect("create again");
        assert_eq!(again, path, "the freed slot is reused");
        let elsewhere = serde_json::json!({"worktree_path": fixture.main});
        assert_eq!(
            hook::remove(&pool, &host, &me, &elsewhere).expect("ignored"),
            None
        );
    }

    #[test]
    fn ten_agents_in_a_row_without_remove_hooks_use_at_most_two_slots() {
        let fixture = Fixture::new();
        let pool = fixture.pool();
        let host = FakeHost::new();
        let me = who("s1", 10);
        for n in 0..10u32 {
            let input = serde_json::json!({"name": format!("agent-{n}"), "cwd": fixture.main});
            let path = hook::create(&pool, &host, &me, &input).expect("create");
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
        let one = hook::create(&pool, &host, &me, &first).expect("one");
        let second = serde_json::json!({"name": "two", "cwd": fixture.main});
        let two = hook::create(&pool, &host, &me, &second).expect("two");
        assert_ne!(one, two, "a young lease is not recycled");
        host.advance(120);
        std::fs::write(two.join("work.txt"), "dirty").expect("dirty");
        let third = serde_json::json!({"name": "three", "cwd": fixture.main});
        let three = hook::create(&pool, &host, &me, &third).expect("three");
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
        let four = hook::create(&pool, &host, &me, &fourth).expect("four");
        assert_eq!(four, one, "a later create reuses the freed slot");
        assert_eq!(pool.records().len(), 3);
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

#[test]
fn release_removes_a_clean_pushed_external_worktree() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    let hand = hand_checkout(&fixture, "by-hand");
    git(&hand, &["push", "--quiet", "-u", "origin", "by-hand"]);
    let released =
        ops::release(&pool, &host, &who("s1", 10), &hand, false).expect("release external");
    assert!(released.removed);
    assert_eq!(released.slot, None);
    assert_eq!(released.branch.as_deref(), Some("by-hand"));
    assert!(!hand.exists());
    assert_eq!(host.shutdown_count(), 1);
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
    assert!(fixture.main.exists());
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
    let plan = ops::prune_plan(&pool, &host);
    let paths: Vec<&PathBuf> = plan.externals.iter().map(|item| &item.path).collect();
    assert_eq!(paths, vec![&merged]);
    assert!(merged.exists());
    ops::prune_apply(&pool, &host, &plan).expect("apply");
    assert!(!merged.exists());
    assert!(!base.exists());
    assert!(open.exists());
    assert!(dirty.exists());
    assert!(fresh.exists());
    assert!(locked.exists());
    assert!(serving.exists());
    assert!(ops::prune_plan(&pool, &host).externals.is_empty());
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
    assert!(ops::prune_plan(&pool, &host).externals.is_empty());
    git(
        &fixture.main,
        &["push", "--quiet", "origin", "--delete", "topic"],
    );
    git(&fixture.main, &["fetch", "--quiet", "--prune", "origin"]);
    let plan = ops::prune_plan(&pool, &host);
    assert_eq!(plan.externals.len(), 1);
    assert_eq!(plan.externals[0].path, hand);
}
