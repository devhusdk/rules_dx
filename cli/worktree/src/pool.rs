use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::git;
use super::host;
use super::WorktreeError;

pub const POOL_LOCK_WAIT: Duration = Duration::from_secs(120);
const QUIET_WAIT: Duration = Duration::from_millis(250);

/// Who asks for a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Who {
    pub session: Option<String>,
    pub agent: Option<String>,
    pub pid: u32,
}

/// The variable that names the agent inside a session.
pub(crate) const HOLDER_ENV: &str = "DX_WORKTREE_HOLDER";

/// The variable `hook guard` sets for agents whose shells carry no session id.
pub(crate) const SESSION_ENV: &str = "DX_WORKTREE_SESSION";

impl Who {
    /// The agent session and agent in the environment and the nearest non-shell ancestor.
    pub fn from_env(env_get: &dyn Fn(&str) -> Option<String>) -> Who {
        Who {
            session: session_from_env(env_get),
            agent: env_get(HOLDER_ENV).filter(|value| !value.is_empty()),
            pid: parent_pid(claude_pid(env_get)),
        }
    }
}

/// The agent session named by the environment, if any.
pub(crate) fn session_from_env(env_get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    ["CLAUDE_CODE_SESSION_ID", "CODEX_THREAD_ID", SESSION_ENV]
        .iter()
        .find_map(|name| env_get(name).filter(|value| !value.is_empty()))
}

fn claude_pid(env_get: &dyn Fn(&str) -> Option<String>) -> Option<u32> {
    env_get("CLAUDE_PID").and_then(|value| value.trim().parse().ok())
}

const SKIPPED: [&str; 20] = [
    "dx",
    "dx.exe",
    "sh",
    "bash",
    "zsh",
    "dash",
    "fish",
    "ksh",
    "cmd",
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "timeout",
    "env",
    "nice",
    "nohup",
    "setsid",
    "stdbuf",
    "time",
    "xargs",
];

/// `preferred` when it is in the chain, else the first ancestor that is not a shell, wrapper or dx.
fn outliving_ancestor(chain: &[(u32, String)], preferred: Option<u32>) -> Option<u32> {
    if let Some(pid) = preferred.filter(|pid| chain.iter().any(|(entry, _)| entry == pid)) {
        return Some(pid);
    }
    let is_skipped = |name: &str| {
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
        SKIPPED.contains(&base.trim_start_matches('-').to_lowercase().as_str())
    };
    chain
        .iter()
        .find(|(_, name)| !is_skipped(name))
        .or_else(|| chain.first())
        .map(|(pid, _)| *pid)
}

#[cfg(unix)]
fn ancestors() -> Vec<(u32, String)> {
    let mut chain = Vec::new();
    let mut pid = u32::try_from(unsafe { libc::getppid() }).unwrap_or(0);
    while pid > 1 && chain.len() < 8 {
        let ps = |field: &str| {
            dx_process::probe("ps", ["-o", field, "-p", &pid.to_string()])
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        };
        let Some(name) = ps("comm=") else {
            break;
        };
        chain.push((pid, name));
        match ps("ppid=").and_then(|text| text.parse().ok()) {
            Some(parent) => pid = parent,
            None => break,
        }
    }
    chain
}

#[cfg(windows)]
use dx_process::procs::process_table;

/// The ancestors of `own` in `table`, nearest first, at most eight.
#[cfg_attr(not(windows), allow(dead_code))]
fn chain_in(table: &[(u32, u32, String)], own: u32) -> Vec<(u32, String)> {
    let parent_of = |pid: u32| table.iter().find(|(entry, _, _)| *entry == pid);
    let mut chain: Vec<(u32, String)> = Vec::new();
    let mut seen = vec![own];
    let mut pid = parent_of(own).map_or(0, |(_, parent, _)| *parent);
    while pid != 0 && !seen.contains(&pid) && chain.len() < 8 {
        let Some((_, parent, name)) = parent_of(pid) else {
            break;
        };
        chain.push((pid, name.clone()));
        seen.push(pid);
        pid = *parent;
    }
    chain
}

/// The parent pid of `own` in `table`, even if that parent has exited.
#[cfg_attr(not(windows), allow(dead_code))]
fn parent_in(table: &[(u32, u32, String)], own: u32) -> Option<u32> {
    table
        .iter()
        .find(|(pid, _, _)| *pid == own)
        .map(|(_, parent, _)| *parent)
        .filter(|parent| *parent != 0 && *parent != own)
}

#[cfg(unix)]
fn parent_pid(preferred: Option<u32>) -> u32 {
    outliving_ancestor(&ancestors(), preferred)
        .unwrap_or_else(|| u32::try_from(unsafe { libc::getppid() }).unwrap_or(0))
}

#[cfg(windows)]
fn parent_pid(preferred: Option<u32>) -> u32 {
    let table = process_table();
    let own = std::process::id();
    outliving_ancestor(&chain_in(&table, own), preferred)
        .or_else(|| parent_in(&table, own))
        .unwrap_or(0)
}

/// One slot's lease file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub slot: u32,
    pub held: bool,
    pub branch: Option<String>,
    pub last_branch: Option<String>,
    pub session: Option<String>,
    #[serde(default)]
    pub agent: Option<String>,
    pub pid: Option<u32>,
    #[serde(default)]
    pub gone: bool,
    /// The first agent that runs `dx worktree` inside the slot takes the lease.
    #[serde(default)]
    pub unclaimed: bool,
    /// A delete left part of the checkout on disk; `add` never leases it, `prune` deletes it again.
    #[serde(default)]
    pub broken: bool,
    pub leased_at: u64,
    pub last_used: u64,
    pub lock_digest: Option<String>,
}

impl Record {
    /// Whether `who` is the session that holds this lease, whichever agent.
    pub fn same_session(&self, who: &Who) -> bool {
        match (&self.session, &who.session) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.pid == Some(who.pid),
            _ => false,
        }
    }

    /// Whether `who` must stay out of this slot: another session holds it, or another agent
    /// of the same session does. Agent-less callers, unclaimed hook slots and agents that
    /// stopped keep the session rule.
    pub fn refuses(&self, who: &Who) -> bool {
        if !self.held || self.session.is_none() {
            return false;
        }
        if !self.same_session(who) {
            return true;
        }
        !self.unclaimed
            && !self.gone
            && matches!((&self.agent, &who.agent), (Some(mine), Some(theirs)) if mine != theirs)
    }

    /// Whether `who` is the very agent that holds this lease.
    pub(crate) fn held_by(&self, who: &Who) -> bool {
        self.same_session(who) && self.agent == who.agent
    }

    /// Whether `who` may take this unclaimed lease.
    pub(crate) fn claimable_by(&self, who: &Who) -> bool {
        self.held && self.unclaimed && self.same_session(who)
    }

    /// Hands the lease to `who`.
    pub(crate) fn lease_to(&mut self, who: &Who) {
        self.session = who.session.clone();
        self.agent = who.agent.clone();
        self.pid = Some(who.pid);
        self.gone = false;
        self.unclaimed = false;
    }

    /// Clears the holder of a freed lease.
    pub(crate) fn free(&mut self) {
        self.held = false;
        self.branch = None;
        self.session = None;
        self.agent = None;
        self.pid = None;
        self.gone = false;
        self.unclaimed = false;
    }

    /// A short name for the holder, as `list` and refusals print it.
    pub fn holder(&self, alive: bool) -> Option<String> {
        if !self.held {
            return None;
        }
        let mut parts = Vec::new();
        if let Some(session) = &self.session {
            parts.push(format!("session {session}"));
        }
        if let Some(agent) = &self.agent {
            parts.push(format!("agent {agent}"));
        }
        if let Some(pid) = self.pid {
            parts.push(format!("pid {pid}"));
        }
        let mut label = if parts.is_empty() {
            "unknown".to_owned()
        } else {
            parts.join(" ")
        };
        if !alive {
            label.push_str(" (gone)");
        }
        Some(label)
    }
}

/// The slots of one repository.
#[derive(Debug, Clone)]
pub struct Pool {
    pub(crate) main: PathBuf,
    pub(crate) base: PathBuf,
    pub(crate) lock_wait: Duration,
}

/// The directory every repository's slots live under.
pub fn cache_base(env_get: &dyn Fn(&str) -> Option<String>) -> Result<PathBuf, WorktreeError> {
    let cache = dx_process::user_cache_dir(env_get).ok_or_else(|| {
        WorktreeError::refused(
            "no_home",
            "cannot find a cache directory for the worktree pool; set XDG_CACHE_HOME, HOME or LOCALAPPDATA",
        )
    })?;
    Ok(cache.join("dx").join("worktrees"))
}

/// The checkout that owns the git directory, whichever worktree `dir` is in.
pub(crate) fn main_checkout(dir: &Path) -> Result<PathBuf, WorktreeError> {
    let dirs = git::line(
        dir,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
            "--git-dir",
        ],
    )
    .map_err(|_| {
        WorktreeError::usage(
            "not_a_repository",
            &format!(
                "{} is not inside a git repository; run dx worktree from a git checkout",
                dir.display()
            ),
        )
    })?;
    let mut lines = dirs.lines();
    let common = PathBuf::from(lines.next().unwrap_or_default().trim());
    let git_dir = PathBuf::from(lines.next().unwrap_or_default().trim());
    if git::checkouts(dir)?.first().is_some_and(|entry| entry.bare) {
        return Err(WorktreeError::usage(
            "bare_repository",
            "dx worktree needs a repository with a main checkout, not a bare one; clone it without --bare and run dx worktree there",
        ));
    }
    git::common_worktree(&common)
        .or_else(|| dot_git_parent(&common))
        .or_else(|| (git_dir == common).then(|| toplevel(dir)).flatten())
        .or_else(|| pool_marker(dir))
        .ok_or_else(|| {
            WorktreeError::usage(
                "no_main_checkout",
                "cannot find the main checkout; run dx worktree from it",
            )
        })
}

fn dot_git_parent(common: &Path) -> Option<PathBuf> {
    if common.file_name().and_then(|name| name.to_str()) != Some(".git") {
        return None;
    }
    common.parent().map(Path::to_path_buf)
}

fn toplevel(dir: &Path) -> Option<PathBuf> {
    let top = git::line(dir, &["rev-parse", "--show-toplevel"]).ok()?;
    (!top.is_empty()).then(|| host::resolved(Path::new(&top)))
}

fn pool_marker(dir: &Path) -> Option<PathBuf> {
    let (slot, _) = lease_for_workspace(&host::resolved(dir))?;
    let marker = slot.parent()?.join(".dx").join("repo");
    let text = std::fs::read_to_string(marker).ok()?;
    let main = text.trim();
    (!main.is_empty()).then(|| PathBuf::from(main))
}

impl Pool {
    /// Opens the pool for the repository that holds `dir`.
    pub fn open(dir: &Path, cache_base: &Path) -> Result<Pool, WorktreeError> {
        Pool::locate(dir, cache_base, true)?.ok_or_else(|| {
            WorktreeError::refused(
                "pool_unavailable",
                &format!(
                    "cannot name a pool for this repository; remove unused pool directories under {}",
                    cache_base.display()
                ),
            )
        })
    }

    /// The existing pool for the repository that holds `dir`, without creating one.
    pub(crate) fn find(dir: &Path, cache_base: &Path) -> Option<Pool> {
        Pool::locate(dir, cache_base, false).ok().flatten()
    }

    fn locate(dir: &Path, cache_base: &Path, create: bool) -> Result<Option<Pool>, WorktreeError> {
        let main = main_checkout(dir)?;
        let name = main
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repo")
            .to_owned();
        for suffix in 0u32.. {
            let candidate = if suffix == 0 {
                name.clone()
            } else {
                format!("{name}-{}", suffix + 1)
            };
            let base = cache_base.join(&candidate);
            let marker = base.join(".dx").join("repo");
            let claimed = std::fs::read_to_string(&marker).ok();
            match claimed {
                Some(text) if Path::new(text.trim()) == main => {
                    return Ok(Some(Pool {
                        main,
                        base,
                        lock_wait: POOL_LOCK_WAIT,
                    }));
                }
                Some(_) => continue,
                None if !create => return Ok(None),
                None => {
                    std::fs::create_dir_all(base.join(".dx")).map_err(WorktreeError::io)?;
                    dx_atomic_fs::write_atomic(&marker, main.to_string_lossy().as_bytes())
                        .map_err(WorktreeError::io)?;
                    return Ok(Some(Pool {
                        main,
                        base,
                        lock_wait: POOL_LOCK_WAIT,
                    }));
                }
            }
        }
        Ok(None)
    }

    /// Every checkout of the repository, with the main entry at `main`.
    pub(super) fn checkouts(&self) -> Result<Vec<git::Checkout>, WorktreeError> {
        let mut found = git::checkouts(&self.main)?;
        if let Some(first) = found.first_mut() {
            first.path = self.main.clone();
        }
        Ok(found)
    }

    pub(crate) fn slot_path(&self, slot: u32) -> PathBuf {
        self.base.join(slot.to_string())
    }

    fn meta_dir(&self) -> PathBuf {
        self.base.join(".dx")
    }

    fn record_path(&self, slot: u32) -> PathBuf {
        self.meta_dir().join(format!("{slot}.json"))
    }

    /// The lock file every dx command in `slot` holds shared while it runs.
    pub(crate) fn busy_path(&self, slot: u32) -> PathBuf {
        busy_file(&self.base, slot)
    }

    /// Locks `slot` against dx commands; `None` means one runs there now.
    pub(crate) fn try_quiet(&self, slot: u32) -> Result<Option<std::fs::File>, WorktreeError> {
        match dx_atomic_fs::lease::acquire_exclusive_at(&self.busy_path(slot), QUIET_WAIT) {
            Ok(lease) => Ok(Some(lease.into_file())),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(WorktreeError::io(error)),
        }
    }

    /// Whether a dx command runs in `slot` now.
    pub(crate) fn busy(&self, slot: u32) -> bool {
        matches!(self.try_quiet(slot), Ok(None))
    }

    /// Holds the pool lock while a slot is chosen.
    pub(crate) fn lock(&self) -> Result<dx_atomic_fs::lease::ExclusiveLease, WorktreeError> {
        dx_atomic_fs::lease::acquire_exclusive_at(
            &self.meta_dir().join("pool.lock"),
            self.lock_wait,
        )
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::WouldBlock => WorktreeError::refused(
                "pool_busy",
                "another dx worktree command holds the pool lock; run the command again",
            ),
            _ => WorktreeError::io(error),
        })
    }

    pub(crate) fn records(&self) -> Vec<Record> {
        let mut records: Vec<Record> = Vec::new();
        let Ok(entries) = std::fs::read_dir(self.meta_dir()) else {
            return records;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if let Some(record) = std::fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            {
                records.push(record);
            }
        }
        records.sort_by_key(|record| record.slot);
        records
    }

    pub(crate) fn record(&self, slot: u32) -> Option<Record> {
        self.records()
            .into_iter()
            .find(|record| record.slot == slot)
    }

    pub(crate) fn write(&self, record: &Record) -> Result<(), WorktreeError> {
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|error| WorktreeError::refused("record_unwritable", &error.to_string()))?;
        dx_atomic_fs::write_atomic(&self.record_path(record.slot), &bytes)
            .map_err(WorktreeError::io)
    }

    pub(crate) fn remove(&self, slot: u32) {
        let _ = std::fs::remove_file(self.record_path(slot));
    }

    /// The slot a path is, or is inside.
    pub fn slot_of(&self, path: &Path) -> Option<u32> {
        let path = super::host::resolved(path);
        let base = super::host::resolved(&self.base);
        let rest = path.strip_prefix(&base).ok()?;
        rest.components()
            .next()?
            .as_os_str()
            .to_str()?
            .parse::<u32>()
            .ok()
    }

    /// Drops records whose checkout was removed by hand.
    pub(crate) fn forget_missing(&self) -> Vec<u32> {
        let mut gone = Vec::new();
        for record in self.records() {
            if !self.slot_path(record.slot).exists() {
                self.remove(record.slot);
                gone.push(record.slot);
            }
        }
        if !gone.is_empty() {
            let _ = git::run(&self.main, &["worktree", "prune"]);
        }
        gone
    }

    pub(crate) fn next_slot(&self, records: &[Record]) -> u32 {
        (1u32..)
            .find(|candidate| {
                !records.iter().any(|record| record.slot == *candidate)
                    && !self.slot_path(*candidate).exists()
            })
            .unwrap_or(1)
    }
}

fn busy_file(base: &Path, slot: u32) -> PathBuf {
    base.join(".dx").join(format!("{slot}.busy"))
}

/// Holds the busy lock of the slot `workspace` is in; `None` outside a slot.
pub fn hold_busy(
    workspace: &Path,
    wait: Duration,
) -> Option<Result<dx_atomic_fs::lease::SharedLease, String>> {
    let (dir, record) = lease_for_workspace(workspace)?;
    let base = dir.parent()?;
    Some(
        dx_atomic_fs::lease::acquire_shared_at(&busy_file(base, record.slot), wait).map_err(
            |error| match error.kind() {
                std::io::ErrorKind::WouldBlock => format!(
                    "slot {} is being switched by dx worktree; run the command again",
                    record.slot
                ),
                _ => format!("cannot lock slot {}: {error}", record.slot),
            },
        ),
    )
}

/// The slot a workspace is in and its lease, found from the path alone.
pub fn lease_for_workspace(workspace: &Path) -> Option<(PathBuf, Record)> {
    let mut current = Some(workspace);
    while let Some(dir) = current {
        let parent = dir.parent()?;
        let slot = dir
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.parse::<u32>().ok());
        if let Some(slot) = slot {
            let record = parent.join(".dx").join(format!("{slot}.json"));
            if let Some(found) = std::fs::read(&record)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Record>(&bytes).ok())
            {
                if found.slot == slot {
                    return Some((dir.to_path_buf(), found));
                }
            }
        }
        current = Some(parent);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recorded_process_skips_shells() {
        let chain = |names: &[&str]| -> Vec<(u32, String)> {
            names
                .iter()
                .enumerate()
                .map(|(index, name)| (index as u32 + 10, (*name).to_owned()))
                .collect()
        };
        assert_eq!(
            outliving_ancestor(&chain(&["bash", "-zsh", "/usr/bin/claude", "login"]), None),
            Some(12)
        );
        assert_eq!(outliving_ancestor(&chain(&["bash", "sh"]), None), Some(10));
        assert_eq!(outliving_ancestor(&[], None), None);
    }

    #[test]
    fn the_recorded_process_skips_wrappers() {
        let chain: Vec<(u32, String)> = ["timeout", "/usr/bin/env", "bash", "nice", "claude"]
            .iter()
            .enumerate()
            .map(|(index, name)| (index as u32 + 10, (*name).to_owned()))
            .collect();
        assert_eq!(outliving_ancestor(&chain, None), Some(14));
    }

    #[test]
    fn claude_pid_in_the_chain_wins() {
        let chain = vec![
            (30, "bash".to_owned()),
            (20, "node".to_owned()),
            (10, "claude".to_owned()),
        ];
        assert_eq!(outliving_ancestor(&chain, Some(10)), Some(10));
        assert_eq!(outliving_ancestor(&chain, Some(99)), Some(20));
        let env = |name: &str| (name == "CLAUDE_PID").then(|| " 4242 ".to_owned());
        assert_eq!(claude_pid(&env), Some(4242));
        assert_eq!(claude_pid(&|_: &str| Some("x".to_owned())), None);
    }

    #[test]
    fn the_windows_chain_skips_dx() {
        let table = vec![
            (4200, 4100, "dx.exe".to_owned()),
            (4100, 4000, "cmd.exe".to_owned()),
            (4000, 3900, "cmd.exe".to_owned()),
            (3900, 1, "claude.exe".to_owned()),
        ];
        let chain = chain_in(&table, 4200);
        assert_eq!(
            chain,
            vec![
                (4100, "cmd.exe".to_owned()),
                (4000, "cmd.exe".to_owned()),
                (3900, "claude.exe".to_owned()),
            ]
        );
        assert_eq!(outliving_ancestor(&chain, None), Some(3900));
        let unix = vec![
            (30, "dx".to_owned()),
            (20, "bash".to_owned()),
            (10, "claude".to_owned()),
        ];
        assert_eq!(outliving_ancestor(&unix, None), Some(10));
    }

    #[test]
    fn the_holder_falls_back_to_the_parent_not_dx() {
        let orphan = vec![(4200, 4100, "dx.exe".to_owned())];
        assert_eq!(chain_in(&orphan, 4200), Vec::new());
        assert_eq!(parent_in(&orphan, 4200), Some(4100));
        assert_eq!(parent_in(&[], 4200), None);
        assert_eq!(chain_in(&[], 4200), Vec::new());
        let own_parent = vec![(4200, 4200, "dx.exe".to_owned())];
        assert_eq!(parent_in(&own_parent, 4200), None);
        assert_eq!(chain_in(&own_parent, 4200), Vec::new());
        let cycle = vec![
            (4200, 10, "dx.exe".to_owned()),
            (10, 20, "a.exe".to_owned()),
            (20, 10, "b.exe".to_owned()),
        ];
        assert_eq!(
            chain_in(&cycle, 4200),
            vec![(10, "a.exe".to_owned()), (20, "b.exe".to_owned())]
        );
        let mut deep = vec![(4200, 1, "dx.exe".to_owned())];
        deep.extend((1..=20).map(|pid| (pid, pid + 1, format!("p{pid}.exe"))));
        let chain = chain_in(&deep, 4200);
        assert_eq!(chain.len(), 8);
        assert_eq!(chain.first(), Some(&(1, "p1.exe".to_owned())));
        assert_eq!(chain.last(), Some(&(8, "p8.exe".to_owned())));
    }

    #[test]
    fn sessions_come_from_claude_then_codex_then_the_guard_then_nothing() {
        let both = |name: &str| match name {
            "CLAUDE_CODE_SESSION_ID" => Some("c1".to_owned()),
            "CODEX_THREAD_ID" => Some("x1".to_owned()),
            _ => None,
        };
        assert_eq!(session_from_env(&both).as_deref(), Some("c1"));
        let codex = |name: &str| (name == "CODEX_THREAD_ID").then(|| "x1".to_owned());
        assert_eq!(session_from_env(&codex).as_deref(), Some("x1"));
        let codex_and_guard = |name: &str| match name {
            "CODEX_THREAD_ID" => Some("x1".to_owned()),
            SESSION_ENV => Some("g1".to_owned()),
            _ => None,
        };
        assert_eq!(session_from_env(&codex_and_guard).as_deref(), Some("x1"));
        let guard = |name: &str| (name == SESSION_ENV).then(|| "g1".to_owned());
        assert_eq!(session_from_env(&guard).as_deref(), Some("g1"));
        let empty = |name: &str| (name == "CLAUDE_CODE_SESSION_ID").then(String::new);
        assert_eq!(session_from_env(&empty), None);
        let empty_guard = |name: &str| (name == SESSION_ENV).then(String::new);
        assert_eq!(session_from_env(&empty_guard), None);
    }

    fn record() -> Record {
        Record {
            slot: 1,
            held: true,
            branch: Some("b".to_owned()),
            last_branch: None,
            session: Some("s1".to_owned()),
            agent: None,
            pid: Some(7),
            gone: false,
            unclaimed: false,
            broken: false,
            leased_at: 0,
            last_used: 0,
            lock_digest: None,
        }
    }

    fn who(session: Option<&str>, agent: Option<&str>, pid: u32) -> Who {
        Who {
            session: session.map(str::to_owned),
            agent: agent.map(str::to_owned),
            pid,
        }
    }

    #[test]
    fn the_holder_agent_comes_from_its_own_variable() {
        let env = |name: &str| match name {
            "CLAUDE_CODE_SESSION_ID" => Some("s1".to_owned()),
            HOLDER_ENV => Some("a1".to_owned()),
            _ => None,
        };
        let got = Who::from_env(&env);
        assert_eq!(got.session.as_deref(), Some("s1"));
        assert_eq!(got.agent.as_deref(), Some("a1"));
        let empty = |name: &str| (name == HOLDER_ENV).then(String::new);
        assert_eq!(Who::from_env(&empty).agent, None);
    }

    #[test]
    fn two_agents_in_one_session_are_different_holders() {
        let mut record = record();
        let first = who(Some("s1"), Some("a1"), 7);
        let second = who(Some("s1"), Some("a2"), 7);
        record.lease_to(&first);
        assert!(record.held_by(&first));
        assert!(!record.held_by(&second));
        assert!(!record.held_by(&who(Some("s1"), None, 7)));
        assert!(record.same_session(&second));
        assert_eq!(
            record.holder(true).as_deref(),
            Some("session s1 agent a1 pid 7")
        );
        record.lease_to(&second);
        assert_eq!(
            record.holder(true).as_deref(),
            Some("session s1 agent a2 pid 7")
        );
    }

    #[test]
    fn refuses_other_sessions_and_other_agents_only() {
        let mut record = record();
        record.agent = Some("a1".to_owned());
        assert!(record.refuses(&who(Some("s2"), None, 9)));
        assert!(record.refuses(&who(Some("s1"), Some("a2"), 9)));
        assert!(!record.refuses(&who(Some("s1"), Some("a1"), 9)));
        assert!(!record.refuses(&who(Some("s1"), None, 9)));
        record.unclaimed = true;
        assert!(!record.refuses(&who(Some("s1"), Some("a2"), 9)));
        record.unclaimed = false;
        record.gone = true;
        assert!(!record.refuses(&who(Some("s1"), Some("a2"), 9)));
        assert!(record.refuses(&who(Some("s2"), Some("a2"), 9)));
        record.held = false;
        assert!(!record.refuses(&who(Some("s2"), None, 9)));
    }

    #[test]
    fn only_the_same_session_can_claim_an_unclaimed_lease_once() {
        let mut record = record();
        let first = who(Some("s1"), Some("a1"), 7);
        assert!(!record.claimable_by(&first));
        record.unclaimed = true;
        assert!(record.claimable_by(&first));
        assert!(record.claimable_by(&who(Some("s1"), None, 9)));
        assert!(!record.claimable_by(&who(Some("s2"), Some("a1"), 7)));
        record.lease_to(&first);
        assert!(!record.unclaimed);
        assert!(!record.claimable_by(&who(Some("s1"), Some("a2"), 7)));
        assert!(record.held_by(&first));
        record.unclaimed = true;
        record.free();
        assert!(!record.unclaimed);
        assert!(!record.claimable_by(&first));
    }

    #[test]
    fn old_records_without_an_agent_still_load() {
        let text = r#"{"slot":2,"held":true,"branch":"b","last_branch":null,"session":"s1","pid":7,"leased_at":0,"last_used":0,"lock_digest":null}"#;
        let loaded: Record = serde_json::from_str(text).expect("loads");
        assert_eq!(loaded.agent, None);
        assert!(!loaded.gone);
        assert!(!loaded.unclaimed);
        assert!(loaded.held_by(&who(Some("s1"), None, 1)));
    }

    #[test]
    fn holders_match_by_session_or_by_process_when_both_have_none() {
        let mut record = record();
        let same = who(Some("s1"), None, 99);
        let other = who(Some("s2"), None, 7);
        assert!(record.held_by(&same));
        assert!(!record.held_by(&other));
        record.session = None;
        assert!(record.held_by(&who(None, None, 7)));
        assert!(!record.held_by(&who(None, None, 8)));
        assert_eq!(record.holder(true).as_deref(), Some("pid 7"));
        assert_eq!(record.holder(false).as_deref(), Some("pid 7 (gone)"));
        record.held = false;
        assert_eq!(record.holder(true), None);
    }

    #[test]
    fn the_cache_base_follows_xdg_then_home_then_local_app_data() {
        let xdg = |name: &str| match name {
            "XDG_CACHE_HOME" => Some("/x".to_owned()),
            "HOME" => Some("/home/u".to_owned()),
            _ => None,
        };
        assert_eq!(cache_base(&xdg), Ok(PathBuf::from("/x/dx/worktrees")));
        let home = |name: &str| (name == "HOME").then(|| "/home/u".to_owned());
        assert_eq!(
            cache_base(&home),
            Ok(PathBuf::from("/home/u/.cache/dx/worktrees"))
        );
        let windows =
            |name: &str| (name == "LOCALAPPDATA").then(|| "C:\\Users\\u\\AppData".to_owned());
        assert_eq!(
            cache_base(&windows),
            Ok(PathBuf::from("C:\\Users\\u\\AppData")
                .join("dx")
                .join("worktrees"))
        );
        let empty_home = |name: &str| match name {
            "HOME" => Some(String::new()),
            "LOCALAPPDATA" => Some("C:\\Users\\u\\AppData".to_owned()),
            _ => None,
        };
        assert_eq!(
            cache_base(&empty_home),
            Ok(PathBuf::from("C:\\Users\\u\\AppData")
                .join("dx")
                .join("worktrees"))
        );
        let error = cache_base(&|_| None).unwrap_err();
        assert_eq!(error.code, "no_home");
        assert!(
            error.message.contains("XDG_CACHE_HOME"),
            "{}",
            error.message
        );
        assert!(error.message.contains("HOME"), "{}", error.message);
    }
}
