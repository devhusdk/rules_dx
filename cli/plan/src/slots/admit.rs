use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use dx_adopt::defaults::BuildSlots;
use dx_worktree::host::{self, Host as _, Server, SystemHost};

pub const HEADROOM: u64 = 2 << 30;

pub const YOUNG: Duration = Duration::from_secs(180);

pub const WAIT: Duration = Duration::from_secs(60);

const POLL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub slot: u64,
    pub free: u64,
    pub servers: usize,
}

impl Limits {
    #[must_use]
    pub fn new(setting: BuildSlots, memory: u64, reserved: u64, slot: u64) -> Option<Limits> {
        let sizing = super::sizing(
            setting,
            super::per_slot_scaled(memory, slot),
            super::per_slot_scaled(reserved, slot),
        )?;
        Some(Limits {
            slot,
            free: slot.saturating_add(reserved).saturating_add(HEADROOM),
            servers: sizing.count,
        })
    }
}

pub trait Machine {
    fn mem_available(&self) -> Option<u64>;
    fn servers(&self) -> Vec<Server>;
    fn idle(&self, server: &Server) -> Option<bool>;
    fn age(&self, server: &Server) -> Option<Duration>;
    fn footprint(&self, server: &Server) -> u64;
    fn now(&self) -> SystemTime;
    fn pause(&self) -> Option<i32>;
    fn try_lock(&self, path: &Path) -> io::Result<Option<File>>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemMachine;

impl Machine for SystemMachine {
    fn mem_available(&self) -> Option<u64> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        super::parse_mem_available(&text)
    }

    fn servers(&self) -> Vec<Server> {
        SystemHost.servers()
    }

    fn idle(&self, server: &Server) -> Option<bool> {
        SystemHost.server_idle(server)
    }

    fn age(&self, server: &Server) -> Option<Duration> {
        let started = server
            .output_base
            .as_ref()?
            .join("server")
            .join("server.starttime");
        let modified = std::fs::metadata(started).ok()?.modified().ok()?;
        SystemTime::now().duration_since(modified).ok()
    }

    fn footprint(&self, server: &Server) -> u64 {
        SystemHost.server_footprint(server)
    }

    fn now(&self) -> SystemTime {
        SystemTime::now()
    }

    fn pause(&self) -> Option<i32> {
        std::thread::sleep(POLL);
        dx_process::interrupted()
    }

    fn try_lock(&self, path: &Path) -> io::Result<Option<File>> {
        dx_atomic_fs::lease::try_lock_file(path, true)
    }
}

#[must_use]
pub fn wait_message(free: Option<u64>, servers: usize, max: usize) -> String {
    format!(
        "waiting for memory to start a Bazel server: {}",
        load(free, servers, max)
    )
}

#[must_use]
pub fn gave_up_message(free: Option<u64>, servers: usize, max: usize) -> String {
    format!(
        "starting a Bazel server without waiting longer for memory: {}",
        load(free, servers, max)
    )
}

fn load(free: Option<u64>, servers: usize, max: usize) -> String {
    let servers = format!("{servers}/{max} servers");
    match free {
        Some(bytes) => {
            let tenths = bytes / ((1 << 30) / 10);
            format!("{}.{} GiB free, {servers}", tenths / 10, tenths % 10)
        }
        None => servers,
    }
}

fn has_server(servers: &[Server], workspace: &Path) -> bool {
    servers.iter().any(|server| {
        server
            .workspace
            .as_deref()
            .is_some_and(|dir| host::same_path(dir, workspace))
    })
}

fn record_path(dir: &Path) -> PathBuf {
    dir.join("admitted")
}

fn pending(
    dir: &Path,
    now: SystemTime,
    servers: &[Server],
    workspace: &Path,
) -> Vec<(u64, PathBuf)> {
    let now = seconds(now);
    let text = std::fs::read_to_string(record_path(dir)).unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let (at, path) = line.split_once(' ')?;
            Some((at.parse::<u64>().ok()?, PathBuf::from(path)))
        })
        .filter(|(at, path)| {
            now.saturating_sub(*at) < YOUNG.as_secs()
                && !host::same_path(path, workspace)
                && !has_server(servers, path)
        })
        .collect()
}

fn seconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn free_after_growth(
    machine: &dyn Machine,
    servers: &[Server],
    pending: usize,
    slot: u64,
) -> Option<u64> {
    let growth = servers
        .iter()
        .filter(|server| machine.age(server).is_some_and(|age| age < YOUNG))
        .map(|server| slot.saturating_sub(machine.footprint(server)))
        .fold(0u64, u64::saturating_add);
    let pending = u64::try_from(pending)
        .unwrap_or(u64::MAX)
        .saturating_mul(slot);
    machine
        .mem_available()
        .map(|bytes| bytes.saturating_sub(growth).saturating_sub(pending))
}

pub struct Wait<'a> {
    pub limit: Duration,
    pub reap: &'a mut dyn FnMut() -> usize,
}

pub fn admit(
    machine: &dyn Machine,
    dir: &Path,
    workspace: &Path,
    limits: Limits,
    wait: Wait<'_>,
    err: &mut dyn Write,
) -> Result<(), i32> {
    if has_server(&machine.servers(), workspace) || std::fs::create_dir_all(dir).is_err() {
        return Ok(());
    }
    let start = machine.now();
    let expired = || {
        machine
            .now()
            .duration_since(start)
            .is_ok_and(|waited| waited >= wait.limit)
    };
    let lock = loop {
        match machine.try_lock(&dir.join("admit.lock")) {
            Ok(Some(lock)) => break Some(lock),
            Ok(None) if expired() => break None,
            Ok(None) => {
                if let Some(signo) = machine.pause() {
                    return Err(signo);
                }
            }
            Err(_) => return Ok(()),
        }
    };
    let mut reaped = None;
    let mut told = false;
    loop {
        let servers = machine.servers();
        if has_server(&servers, workspace) {
            return Ok(());
        }
        let mut pending = pending(dir, machine.now(), &servers, workspace);
        let busy = servers
            .iter()
            .filter(|server| machine.idle(server) == Some(false))
            .count()
            + pending.len();
        let free = free_after_growth(machine, &servers, pending.len(), limits.slot);
        let fits = busy < limits.servers && free.is_none_or(|bytes| bytes >= limits.free);
        let alone = servers.is_empty() && pending.is_empty();
        if !(alone || fits) && reaped.is_none() {
            reaped = Some((wait.reap)());
            continue;
        }
        let gave_up = !(alone || fits) && expired();
        if gave_up {
            let message = gave_up_message(free, busy, limits.servers);
            let reaped = super::reaped_suffix(reaped.unwrap_or(0));
            dx_output::write_stderr(err, format_args!("dx: {message}{reaped}\n"));
        }
        if alone || fits || gave_up {
            if lock.is_some() {
                pending.push((seconds(machine.now()), workspace.to_path_buf()));
                let text: String = pending
                    .iter()
                    .map(|(at, path)| format!("{at} {}\n", path.display()))
                    .collect();
                drop(dx_atomic_fs::write_atomic(
                    &record_path(dir),
                    text.as_bytes(),
                ));
            }
            return Ok(());
        }
        if !told {
            told = true;
            let message = wait_message(free, busy, limits.servers);
            let reaped = super::reaped_suffix(reaped.unwrap_or(0));
            dx_output::write_stderr(err, format_args!("dx: {message}{reaped}\n"));
        }
        if let Some(signo) = machine.pause() {
            return Err(signo);
        }
    }
}

#[must_use]
pub fn env_setting(env_get: &dyn Fn(&str) -> Option<String>, is_ci: bool) -> Option<BuildSlots> {
    if is_ci {
        return None;
    }
    dx_adopt::defaults::env_string(env_get, dx_adopt::defaults::DX_BUILD_SLOTS_ENV)
        .and_then(|value| BuildSlots::parse(&value))
}

pub fn admit_here(
    workspace: &Path,
    setting: BuildSlots,
    env_get: &dyn Fn(&str) -> Option<String>,
    err: &mut dyn Write,
) -> Result<(), i32> {
    let Some(gate) = super::Gate::from_args(workspace, env_get, false, &[], setting, &[]) else {
        return Ok(());
    };
    let wait = dx_adopt::defaults::env_string(env_get, dx_adopt::defaults::DX_BUILD_SLOTS_WAIT_ENV)
        .and_then(|value| dx_adopt::defaults::parse_wait(&value));
    gate.waiting_at_most(wait).admit(Duration::ZERO, err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    const GIB: u64 = 1 << 30;

    type Listed = Vec<(Server, u64)>;

    struct Fake {
        free: RefCell<Vec<Option<u64>>>,
        servers: RefCell<Vec<Listed>>,
        now: Cell<SystemTime>,
        pauses: Cell<usize>,
        stop_after: Option<usize>,
        idle: RefCell<Vec<u32>>,
        locked: Cell<bool>,
    }

    fn server(pid: u32, workspace: &str, rss: u64) -> Server {
        Server {
            pid,
            rss_bytes: rss,
            workspace: Some(PathBuf::from(workspace)),
            output_base: None,
            output_user_root: None,
        }
    }

    impl Fake {
        fn new(steps: Vec<(Option<u64>, Listed)>) -> Fake {
            let (free, servers) = steps.into_iter().rev().unzip();
            Fake {
                free: RefCell::new(free),
                servers: RefCell::new(servers),
                now: Cell::new(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000)),
                pauses: Cell::new(0),
                stop_after: None,
                idle: RefCell::new(Vec::new()),
                locked: Cell::new(false),
            }
        }

        fn current<T: Clone>(steps: &RefCell<Vec<T>>) -> T {
            steps.borrow().last().cloned().expect("a step")
        }
    }

    impl Machine for Fake {
        fn mem_available(&self) -> Option<u64> {
            Fake::current(&self.free)
        }

        fn servers(&self) -> Vec<Server> {
            Fake::current(&self.servers)
                .into_iter()
                .map(|(server, _)| server)
                .collect()
        }

        fn idle(&self, server: &Server) -> Option<bool> {
            Some(self.idle.borrow().contains(&server.pid))
        }

        fn age(&self, server: &Server) -> Option<Duration> {
            Fake::current(&self.servers)
                .into_iter()
                .find(|(listed, _)| listed.pid == server.pid)
                .map(|(_, age)| Duration::from_secs(age))
        }

        fn footprint(&self, server: &Server) -> u64 {
            server.rss_bytes
        }

        fn now(&self) -> SystemTime {
            self.now.get()
        }

        fn pause(&self) -> Option<i32> {
            self.pauses.set(self.pauses.get() + 1);
            self.now.set(self.now.get() + POLL);
            if self.stop_after == Some(self.pauses.get()) {
                return Some(15);
            }
            let mut free = self.free.borrow_mut();
            if free.len() > 1 {
                free.pop();
            }
            let mut servers = self.servers.borrow_mut();
            if servers.len() > 1 {
                servers.pop();
            }
            None
        }

        // A real flock outlives drop in children forked by parallel tests, and each pause spent
        // waiting on it would consume a scripted step.
        fn try_lock(&self, path: &Path) -> io::Result<Option<File>> {
            if self.locked.get() {
                return Ok(None);
            }
            File::create(path).map(Some)
        }
    }

    const LIMITS: Limits = Limits {
        slot: 3 * GIB,
        free: 5 * GIB,
        servers: 3,
    };

    fn run(fake: &Fake, dir: &Path, workspace: &str) -> (Result<(), i32>, String) {
        run_for(fake, dir, workspace, Duration::from_secs(3600), &mut || 0)
    }

    fn run_for(
        fake: &Fake,
        dir: &Path,
        workspace: &str,
        limit: Duration,
        reap: &mut dyn FnMut() -> usize,
    ) -> (Result<(), i32>, String) {
        let mut err = Vec::new();
        let wait = Wait { limit, reap };
        let result = admit(fake, dir, Path::new(workspace), LIMITS, wait, &mut err);
        (result, String::from_utf8(err).unwrap())
    }

    fn old(pid: u32, workspace: &str) -> (Server, u64) {
        (server(pid, workspace, 3 * GIB), 3600)
    }

    #[test]
    fn limits_need_a_slot_the_nested_reserve_and_headroom_and_cap_servers() {
        let auto = Limits::new(BuildSlots::Auto, 32 * GIB, 6 * GIB, 3 * GIB).unwrap();
        assert_eq!(auto.free, 11 * GIB);
        assert_eq!(auto.servers, super::super::slot_count(32 * GIB, 6 * GIB));
        let two = std::num::NonZeroUsize::new(2).unwrap();
        let capped = Limits::new(BuildSlots::Count(two), 32 * GIB, 0, 3 * GIB).unwrap();
        assert_eq!((capped.free, capped.servers), (5 * GIB, 2));
        assert_eq!(Limits::new(BuildSlots::Off, 32 * GIB, 0, 3 * GIB), None);
    }

    #[test]
    fn a_workspace_with_a_server_or_a_host_with_room_starts_at_once() {
        let scratch = dx_test_scratch::scratch("dx-admit-room-").unwrap();
        let dir = |name: &str| scratch.path().join(name);
        let own = Fake::new(vec![(Some(0), vec![old(1, "/w/a"), old(2, "/w/b")])]);
        assert_eq!(run(&own, &dir("own"), "/w/a"), (Ok(()), String::new()));
        let room = Fake::new(vec![(Some(9 * GIB), vec![old(1, "/w/b")])]);
        assert_eq!(run(&room, &dir("room"), "/w/a"), (Ok(()), String::new()));
        let alone = Fake::new(vec![(Some(GIB), Vec::new())]);
        assert_eq!(run(&alone, &dir("alone"), "/w/c"), (Ok(()), String::new()));
        let unknown = Fake::new(vec![(None, vec![old(1, "/w/b")])]);
        assert_eq!(
            run(&unknown, &dir("unknown"), "/w/d"),
            (Ok(()), String::new())
        );
        assert_eq!(own.pauses.get() + room.pauses.get() + alone.pauses.get(), 0);
    }

    #[test]
    fn low_memory_waits_with_one_line_until_memory_frees() {
        let scratch = dx_test_scratch::scratch("dx-admit-wait-").unwrap();
        let dir = scratch.path().join("slots");
        let fake = Fake::new(vec![
            (Some(4 * GIB), vec![old(1, "/w/b")]),
            (Some(4 * GIB), vec![old(1, "/w/b")]),
            (Some(6 * GIB), vec![old(1, "/w/b")]),
        ]);
        let (result, err) = run(&fake, &dir, "/w/a");
        assert_eq!(result, Ok(()));
        assert_eq!(
            err,
            "dx: waiting for memory to start a Bazel server: 4.0 GiB free, 1/3 servers\n"
        );
        assert_eq!(fake.pauses.get(), 2);
    }

    #[test]
    fn a_full_server_count_waits_even_with_memory_free() {
        let scratch = dx_test_scratch::scratch("dx-admit-count-").unwrap();
        let dir = scratch.path().join("slots");
        let full = vec![old(1, "/w/b"), old(2, "/w/c"), old(3, "/w/d")];
        let fake = Fake::new(vec![
            (Some(64 * GIB), full),
            (Some(64 * GIB), vec![old(1, "/w/b")]),
        ]);
        let (result, err) = run(&fake, &dir, "/w/a");
        assert_eq!(result, Ok(()));
        assert!(err.contains("64.0 GiB free, 3/3 servers"), "{err}");
        assert_eq!(fake.pauses.get(), 1);
    }

    #[test]
    fn young_servers_count_at_full_size() {
        let scratch = dx_test_scratch::scratch("dx-admit-young-").unwrap();
        let dir = scratch.path().join("slots");
        let young = (server(1, "/w/b", GIB), 30);
        let grown = (server(1, "/w/b", GIB), YOUNG.as_secs());
        let fake = Fake::new(vec![
            (Some(6 * GIB), vec![young]),
            (Some(6 * GIB), vec![grown]),
        ]);
        let (result, err) = run(&fake, &dir, "/w/a");
        assert_eq!(result, Ok(()));
        assert!(err.contains("4.0 GiB free, 1/3 servers"), "{err}");
        assert_eq!(fake.pauses.get(), 1);
    }

    #[test]
    fn a_recent_admission_counts_until_its_server_shows() {
        let scratch = dx_test_scratch::scratch("dx-admit-pending-").unwrap();
        let dir = scratch.path().join("slots");
        let first = Fake::new(vec![(Some(9 * GIB), vec![old(1, "/w/b")])]);
        assert_eq!(run(&first, &dir, "/w/a"), (Ok(()), String::new()));
        let second = Fake::new(vec![
            (Some(7 * GIB), vec![old(1, "/w/b")]),
            (Some(7 * GIB), vec![old(1, "/w/b"), old(2, "/w/a")]),
        ]);
        let (result, err) = run(&second, &dir, "/w/c");
        assert_eq!(result, Ok(()));
        assert!(err.contains("4.0 GiB free, 2/3 servers"), "{err}");
        assert_eq!(second.pauses.get(), 1);
        let record = std::fs::read_to_string(record_path(&dir)).unwrap();
        assert!(!record.contains("/w/a"), "{record}");
        assert!(record.contains("/w/c"), "{record}");
    }

    #[test]
    fn a_stop_signal_ends_the_wait() {
        let scratch = dx_test_scratch::scratch("dx-admit-stop-").unwrap();
        let dir = scratch.path().join("slots");
        let mut fake = Fake::new(vec![(Some(GIB), vec![old(1, "/w/b")])]);
        fake.stop_after = Some(3);
        assert_eq!(run(&fake, &dir, "/w/a").0, Err(15));
    }

    #[test]
    fn a_held_lock_holds_admission_back_until_the_wait_limit() {
        let scratch = dx_test_scratch::scratch("dx-admit-lock-").unwrap();
        let dir = scratch.path().join("slots");
        let mut fake = Fake::new(vec![(Some(64 * GIB), vec![old(1, "/w/b")])]);
        fake.locked.set(true);
        fake.stop_after = Some(2);
        assert_eq!(run(&fake, &dir, "/w/a"), (Err(15), String::new()));
        let fake = Fake::new(vec![(Some(64 * GIB), vec![old(1, "/w/b")])]);
        fake.locked.set(true);
        let (result, err) = run_for(&fake, &dir, "/w/a", 3 * POLL, &mut || 0);
        assert_eq!((result, err.as_str()), (Ok(()), ""));
        assert_eq!(fake.pauses.get(), 3);
        assert!(!record_path(&dir).exists(), "admitted without the lock");
    }

    #[test]
    fn idle_servers_do_not_count_against_the_server_cap() {
        let scratch = dx_test_scratch::scratch("dx-admit-idle-").unwrap();
        let dir = scratch.path().join("slots");
        let full = vec![old(1, "/w/b"), old(2, "/w/c"), old(3, "/w/d")];
        let fake = Fake::new(vec![(Some(64 * GIB), full)]);
        fake.idle.borrow_mut().extend([1, 2]);
        assert_eq!(run(&fake, &dir, "/w/a"), (Ok(()), String::new()));
        assert_eq!(fake.pauses.get(), 0);
    }

    #[test]
    fn idle_servers_are_reaped_once_before_any_wait() {
        let scratch = dx_test_scratch::scratch("dx-admit-reap-").unwrap();
        let dir = |name: &str| scratch.path().join(name);
        let tight = || vec![(Some(4 * GIB), vec![old(1, "/w/b"), old(2, "/w/c")])];
        let fake = Fake::new(tight());
        fake.idle.borrow_mut().push(2);
        let mut reaps = 0;
        let mut reap = || {
            reaps += 1;
            *fake.free.borrow_mut() = vec![Some(7 * GIB)];
            *fake.servers.borrow_mut() = vec![vec![old(1, "/w/b")]];
            1
        };
        let (result, err) = run_for(&fake, &dir("freed"), "/w/a", WAIT, &mut reap);
        assert_eq!((result, err.as_str()), (Ok(()), ""));
        assert_eq!((reaps, fake.pauses.get()), (1, 0));

        let mut fake = Fake::new(tight());
        fake.idle.borrow_mut().push(2);
        let mut reaps = 0;
        let mut reap = || {
            reaps += 1;
            0
        };
        fake.stop_after = Some(3);
        let (result, err) = run_for(&fake, &dir("full"), "/w/a", WAIT, &mut reap);
        assert_eq!(result, Err(15));
        assert_eq!(
            err,
            "dx: waiting for memory to start a Bazel server: 4.0 GiB free, 1/3 servers\n"
        );
        assert_eq!(reaps, 1, "one reap per admission");
    }

    #[test]
    fn the_wait_limit_starts_the_server_anyway_with_a_warning() {
        let scratch = dx_test_scratch::scratch("dx-admit-limit-").unwrap();
        let dir = |name: &str| scratch.path().join(name);
        let low = || Fake::new(vec![(Some(GIB), vec![old(1, "/w/b")])]);
        let fake = low();
        let (result, err) = run_for(&fake, &dir("short"), "/w/a", 5 * POLL, &mut || 2);
        assert_eq!(result, Ok(()));
        assert_eq!(
            err,
            "dx: waiting for memory to start a Bazel server: 1.0 GiB free, 1/3 servers; stopped 2 idle Bazel servers\n\
             dx: starting a Bazel server without waiting longer for memory: 1.0 GiB free, 1/3 servers; stopped 2 idle Bazel servers\n"
        );
        assert_eq!(fake.pauses.get(), 5);
        let record = std::fs::read_to_string(record_path(&dir("short"))).unwrap();
        assert!(record.contains("/w/a"), "{record}");

        let fake = low();
        let (result, err) = run_for(&fake, &dir("zero"), "/w/a", Duration::ZERO, &mut || 0);
        assert_eq!(result, Ok(()));
        assert_eq!(
            err,
            "dx: starting a Bazel server without waiting longer for memory: 1.0 GiB free, 1/3 servers\n"
        );
        assert_eq!(fake.pauses.get(), 0);
    }

    #[test]
    fn the_wait_line_names_free_memory_when_known() {
        assert_eq!(
            wait_message(Some(3 * GIB / 2), 2, 4),
            "waiting for memory to start a Bazel server: 1.5 GiB free, 2/4 servers"
        );
        assert_eq!(
            wait_message(None, 4, 4),
            "waiting for memory to start a Bazel server: 4/4 servers"
        );
    }
}
