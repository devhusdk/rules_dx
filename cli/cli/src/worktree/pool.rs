use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::git;
use super::WorktreeError;

const POOL_LOCK_WAIT: Duration = Duration::from_secs(120);

/// Who asks for a slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Who {
    pub(crate) session: Option<String>,
    pub(crate) pid: u32,
}

impl Who {
    /// The agent session in the environment and the parent process.
    pub(crate) fn from_env(env_get: &dyn Fn(&str) -> Option<String>) -> Who {
        Who {
            session: session_from_env(env_get),
            pid: parent_pid(),
        }
    }
}

/// The agent session named by the environment, if any.
pub(crate) fn session_from_env(env_get: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    ["CLAUDE_CODE_SESSION_ID", "CODEX_THREAD_ID"]
        .iter()
        .find_map(|name| env_get(name).filter(|value| !value.is_empty()))
}

#[cfg(unix)]
fn parent_pid() -> u32 {
    u32::try_from(unsafe { libc::getppid() }).unwrap_or(0)
}

#[cfg(not(unix))]
fn parent_pid() -> u32 {
    std::process::id()
}

/// One slot's lease file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) slot: u32,
    pub(crate) held: bool,
    pub(crate) branch: Option<String>,
    pub(crate) last_branch: Option<String>,
    pub(crate) session: Option<String>,
    pub(crate) pid: Option<u32>,
    pub(crate) leased_at: u64,
    pub(crate) last_used: u64,
    pub(crate) lock_digest: Option<String>,
}

impl Record {
    pub(crate) fn held_by(&self, who: &Who) -> bool {
        match (&self.session, &who.session) {
            (Some(left), Some(right)) => left == right,
            (None, None) => self.pid == Some(who.pid),
            _ => false,
        }
    }

    /// A short name for the holder, as `list` and refusals print it.
    pub(crate) fn holder(&self, alive: bool) -> Option<String> {
        if !self.held {
            return None;
        }
        let mut label = match (&self.session, self.pid) {
            (Some(session), Some(pid)) => format!("session {session} pid {pid}"),
            (Some(session), None) => format!("session {session}"),
            (None, Some(pid)) => format!("pid {pid}"),
            (None, None) => "unknown".to_owned(),
        };
        if !alive {
            label.push_str(" (gone)");
        }
        Some(label)
    }
}

/// The slots of one repository.
#[derive(Debug, Clone)]
pub(crate) struct Pool {
    pub(crate) main: PathBuf,
    pub(crate) base: PathBuf,
    pub(crate) lock_wait: Duration,
}

/// The directory every repository's slots live under.
pub(crate) fn cache_base(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    let cache = match non_empty("XDG_CACHE_HOME") {
        Some(cache) => PathBuf::from(cache),
        None => match non_empty("HOME") {
            Some(home) => PathBuf::from(home).join(".cache"),
            None => PathBuf::from(non_empty("LOCALAPPDATA")?),
        },
    };
    Some(cache.join("dx").join("worktrees"))
}

/// The checkout that owns the git directory, whichever worktree `dir` is in.
pub(crate) fn main_checkout(dir: &Path) -> Result<PathBuf, WorktreeError> {
    let common = git::line(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .map_err(|_| {
        WorktreeError::usage(
            "not_a_repository",
            &format!("{} is not inside a git repository", dir.display()),
        )
    })?;
    let common = PathBuf::from(common);
    if common.file_name().and_then(|name| name.to_str()) != Some(".git") {
        return Err(WorktreeError::usage(
            "bare_repository",
            "dx worktree needs a repository with a main checkout, not a bare one",
        ));
    }
    common
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| WorktreeError::usage("not_a_repository", "cannot find the main checkout"))
}

impl Pool {
    /// Opens the pool for the repository that holds `dir`.
    pub(crate) fn open(dir: &Path, cache_base: &Path) -> Result<Pool, WorktreeError> {
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
                    return Ok(Pool {
                        main,
                        base,
                        lock_wait: POOL_LOCK_WAIT,
                    });
                }
                Some(_) => continue,
                None => {
                    std::fs::create_dir_all(base.join(".dx")).map_err(WorktreeError::io)?;
                    dx_atomic_fs::write_atomic(&marker, main.to_string_lossy().as_bytes())
                        .map_err(WorktreeError::io)?;
                    return Ok(Pool {
                        main,
                        base,
                        lock_wait: POOL_LOCK_WAIT,
                    });
                }
            }
        }
        Err(WorktreeError::refused(
            "pool_unavailable",
            "cannot name a pool for this repository",
        ))
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

    /// Holds the pool lock while a slot is chosen.
    pub(crate) fn lock(&self) -> Result<dx_atomic_fs::lease::ExclusiveLease, WorktreeError> {
        dx_atomic_fs::lease::acquire_exclusive_at(
            &self.meta_dir().join("pool.lock"),
            self.lock_wait,
        )
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::WouldBlock => WorktreeError::refused(
                "pool_busy",
                "another dx worktree command holds the pool lock",
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
    pub(crate) fn slot_of(&self, path: &Path) -> Option<u32> {
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let base = std::fs::canonicalize(&self.base).unwrap_or_else(|_| self.base.clone());
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

/// The slot a workspace is in and its lease, found from the path alone.
pub(crate) fn lease_for_workspace(workspace: &Path) -> Option<(PathBuf, Record)> {
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
    fn sessions_come_from_claude_then_codex_then_nothing() {
        let both = |name: &str| match name {
            "CLAUDE_CODE_SESSION_ID" => Some("c1".to_owned()),
            "CODEX_THREAD_ID" => Some("x1".to_owned()),
            _ => None,
        };
        assert_eq!(session_from_env(&both).as_deref(), Some("c1"));
        let codex = |name: &str| (name == "CODEX_THREAD_ID").then(|| "x1".to_owned());
        assert_eq!(session_from_env(&codex).as_deref(), Some("x1"));
        let empty = |name: &str| (name == "CLAUDE_CODE_SESSION_ID").then(String::new);
        assert_eq!(session_from_env(&empty), None);
    }

    #[test]
    fn holders_match_by_session_or_by_process_when_both_have_none() {
        let mut record = Record {
            slot: 1,
            held: true,
            branch: Some("b".to_owned()),
            last_branch: None,
            session: Some("s1".to_owned()),
            pid: Some(7),
            leased_at: 0,
            last_used: 0,
            lock_digest: None,
        };
        let same = Who {
            session: Some("s1".to_owned()),
            pid: 99,
        };
        let other = Who {
            session: Some("s2".to_owned()),
            pid: 7,
        };
        assert!(record.held_by(&same));
        assert!(!record.held_by(&other));
        record.session = None;
        let bare = Who {
            session: None,
            pid: 7,
        };
        assert!(record.held_by(&bare));
        assert!(!record.held_by(&Who {
            session: None,
            pid: 8
        }));
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
        assert_eq!(cache_base(&xdg), Some(PathBuf::from("/x/dx/worktrees")));
        let home = |name: &str| (name == "HOME").then(|| "/home/u".to_owned());
        assert_eq!(
            cache_base(&home),
            Some(PathBuf::from("/home/u/.cache/dx/worktrees"))
        );
        let windows =
            |name: &str| (name == "LOCALAPPDATA").then(|| "C:\\Users\\u\\AppData".to_owned());
        assert_eq!(
            cache_base(&windows),
            Some(
                PathBuf::from("C:\\Users\\u\\AppData")
                    .join("dx")
                    .join("worktrees")
            )
        );
        assert_eq!(cache_base(&|_| None), None);
    }
}
