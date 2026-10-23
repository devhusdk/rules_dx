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
}

impl FakeHost {
    fn new() -> FakeHost {
        FakeHost {
            clock: AtomicU64::new(1_000_000),
            alive: Mutex::new(BTreeSet::new()),
            shutdowns: Mutex::new(Vec::new()),
            free_percent: Mutex::new(None),
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
        Vec::new()
    }

    fn output_roots(&self) -> Vec<PathBuf> {
        Vec::new()
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
    assert_eq!(released.slot, 1);
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

#[test]
fn a_crashed_holder_is_reclaimed_only_after_a_day_and_only_when_clean() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    host.live(20);
    host.live(30);
    let first = ops::add(&pool, &host, &who("s1", 10), &create("feature/a")).expect("add");
    git(
        &first.path,
        &["push", "--quiet", "-u", "origin", "feature/a"],
    );
    let soon = ops::add(&pool, &host, &who("s2", 20), &create("feature/b")).expect("second");
    assert_eq!(soon.slot, 2, "a recent lease is not reclaimed");
    host.advance(DAY + 1);
    let listing = ops::list(&pool, &host).expect("list");
    let states: Vec<State> = listing
        .rows
        .iter()
        .filter(|row| row.slot == Some(1))
        .map(|row| row.state)
        .collect();
    assert_eq!(states, vec![State::Reclaimable]);
    std::fs::write(first.path.join("a.txt"), "dirty\n").expect("dirty");
    let third = ops::add(&pool, &host, &who("s3", 30), &create("feature/c")).expect("third");
    assert_eq!(third.slot, 3, "a dirty slot is never handed out");
    git(&first.path, &["checkout", "--quiet", "--", "a.txt"]);
    let fourth = ops::add(&pool, &host, &who("s4", 40), &create("feature/d")).expect("fourth");
    assert_eq!(
        fourth.slot, 1,
        "a clean pushed slot of a dead holder is reused"
    );
    assert!(host.shutdown_count() >= 1);
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
    ops::prune_apply(&pool, &host, &plan);
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
fn ten_agents_in_a_row_use_at_most_two_slots() {
    let fixture = Fixture::new();
    let pool = fixture.pool();
    let host = FakeHost::new();
    for n in 0..10u32 {
        let me = who(&format!("agent-{n}"), 1000 + n);
        host.live(1000 + n);
        let name = format!("worktree-agent-{n}");
        let added = ops::add(
            &pool,
            &host,
            &me,
            &AddRequest {
                branch: name,
                create: Some(String::new()),
            },
        )
        .expect("add");
        std::fs::write(added.path.join("work.txt"), format!("{n}\n")).expect("work");
        git(&added.path, &["add", "."]);
        git(&added.path, &["commit", "--quiet", "-m", "work"]);
        let branch = git(&added.path, &["branch", "--show-current"]);
        git(&added.path, &["push", "--quiet", "-u", "origin", &branch]);
        ops::release(&pool, &host, &me, &added.path, false).expect("release");
    }
    assert!(pool.records().len() <= 2, "{:?}", pool.records());
    assert_eq!(pool.records().len(), 1);
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
        assert!(block("tools/dx worktree add feature/x").is_none());
        assert!(hook::guard(&serde_json::json!({})).is_none());
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
