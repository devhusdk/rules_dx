//! Build slots shared by every dx run on the machine.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::args::Command;

/// Set once a slot is held; nested dx calls see it and take no second slot.
pub const SLOT_ENV_VAR: &str = "DX_BUILD_SLOT";

/// Set by Bazel in every test action, so dx inside a test takes no slot.
pub const BAZEL_TEST_ENV_VAR: &str = "BAZEL_TEST";

/// Memory one Bazel server plus its build is planned to use.
pub const BYTES_PER_SLOT: u64 = 3 << 30;

/// Nested Bazel servers that tests may run at once on the machine.
pub const NESTED_BAZEL_SERVERS: u64 = 3;

/// Memory one nested Bazel test server is planned to use.
pub const BYTES_PER_NESTED_BAZEL: u64 = 2 << 30;

/// Most slots dx ever offers.
pub const MAX_SLOTS: usize = 16;

const POLL: Duration = Duration::from_millis(200);

/// A held build slot; dropping it, or exiting, frees it.
#[derive(Debug)]
pub struct Slot {
    _file: File,
    pub index: usize,
}

/// Reports whether the command starts Bazel work.
pub fn needs_slot(command: Command) -> bool {
    matches!(
        command,
        Command::Build
            | Command::Test
            | Command::Coverage
            | Command::Run
            | Command::Lint
            | Command::Typecheck
            | Command::Format
            | Command::Check
            | Command::Fix
            | Command::Deploy
            | Command::Generate
            | Command::Codegen
            | Command::Env
            | Command::Security
            | Command::License
            | Command::Docs
            | Command::ListTests
            | Command::Update
    )
}

/// Slots for a machine with `memory` bytes.
pub fn slot_count(memory: u64) -> usize {
    let reserved = NESTED_BAZEL_SERVERS * BYTES_PER_NESTED_BAZEL;
    usize::try_from(memory.saturating_sub(reserved) / BYTES_PER_SLOT)
        .unwrap_or(MAX_SLOTS)
        .clamp(1, MAX_SLOTS)
}

/// The slot directory under the user cache.
pub fn slots_dir(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let non_empty = |name: &str| env_get(name).filter(|value| !value.is_empty());
    let cache = match non_empty("XDG_CACHE_HOME") {
        Some(cache) => PathBuf::from(cache),
        None => match non_empty("HOME") {
            Some(home) => PathBuf::from(home).join(".cache"),
            None => PathBuf::from(non_empty("LOCALAPPDATA")?),
        },
    };
    Some(cache.join("dx").join("slots"))
}

fn slot_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.lock"))
}

fn holder_path(dir: &Path, index: usize) -> PathBuf {
    dir.join(format!("slot-{index}.holder"))
}

fn holder(dir: &Path, count: usize) -> String {
    (0..count)
        .filter_map(|index| std::fs::read_to_string(holder_path(dir, index)).ok())
        .map(|text| text.trim().to_owned())
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| "another build".to_owned())
}

fn try_take(dir: &Path, count: usize, label: &str) -> io::Result<Option<Slot>> {
    for index in 0..count {
        if let Some(file) = dx_atomic_fs::lease::try_lock_file(&slot_path(dir, index), true)? {
            let _ = std::fs::write(holder_path(dir, index), label);
            return Ok(Some(Slot { _file: file, index }));
        }
    }
    Ok(None)
}

/// Takes a free slot, calling `on_wait` once with the holder when none is free; a stop signal ends the wait.
pub fn acquire(
    dir: &Path,
    count: usize,
    label: &str,
    on_wait: &mut dyn FnMut(&str),
) -> io::Result<Slot> {
    let mut announced = false;
    loop {
        if let Some(slot) = try_take(dir, count, label)? {
            return Ok(slot);
        }
        if !announced {
            announced = true;
            on_wait(&holder(dir, count));
        }
        if let Some(signo) = dx_process::interrupted() {
            return Err(stopped(signo));
        }
        std::thread::sleep(POLL);
    }
}

fn stopped(signo: i32) -> io::Error {
    io::Error::new(
        io::ErrorKind::Interrupted,
        format!("stopped by signal {signo}"),
    )
}

/// The one line printed while waiting.
pub fn wait_message(holder: &str) -> String {
    format!("waiting for a build slot (held by {holder})")
}

/// Total memory visible to this process, honoring a cgroup limit.
pub fn available_memory() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let total = std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| parse_meminfo(&text));
        let limit = cgroup_limit();
        return match (total, limit) {
            (Some(total), Some(limit)) => Some(total.min(limit)),
            (total, limit) => total.or(limit),
        };
    }
    #[cfg(target_os = "macos")]
    {
        let mut value: u64 = 0;
        let mut size = std::mem::size_of::<u64>();
        let name = c"hw.memsize";
        let status = unsafe {
            libc::sysctlbyname(
                name.as_ptr(),
                (&mut value as *mut u64).cast(),
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        return (status == 0).then_some(value);
    }
    #[cfg(windows)]
    {
        #[repr(C)]
        struct MemoryStatus {
            length: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virtual: u64,
            avail_virtual: u64,
            avail_extended: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        }
        let mut status = MemoryStatus {
            length: std::mem::size_of::<MemoryStatus>() as u32,
            load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page: 0,
            avail_page: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended: 0,
        };
        let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
        return (ok != 0).then_some(status.total_phys);
    }
    #[allow(unreachable_code)]
    None
}

/// Reads `MemTotal` from `/proc/meminfo` text, in bytes.
pub fn parse_meminfo(text: &str) -> Option<u64> {
    let line = text.lines().find(|line| line.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    kib.checked_mul(1024)
}

/// Reads a cgroup `memory.max` value; `max` means no limit.
pub fn parse_cgroup_limit(text: &str) -> Option<u64> {
    text.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn cgroup_limit() -> Option<u64> {
    let own = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    let relative = own
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap_or("/");
    let root = Path::new("/sys/fs/cgroup");
    let mut dir = root.join(relative.trim_start_matches('/'));
    let mut limit: Option<u64> = None;
    loop {
        let found = std::fs::read_to_string(dir.join("memory.max"))
            .ok()
            .and_then(|text| parse_cgroup_limit(&text));
        limit = match (limit, found) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        if dir == root || !dir.pop() || !dir.starts_with(root) {
            return limit;
        }
    }
}

/// Holds a slot for this invocation when it starts Bazel work; `Err` carries the stop signal that ended the wait.
pub fn hold(
    command: Command,
    workspace: &Path,
    env_get: &dyn Fn(&str) -> Option<String>,
    err: &mut dyn Write,
    json: bool,
    events: &mut dyn Write,
) -> Result<Option<Slot>, i32> {
    let set = |name: &str| env_get(name).is_some_and(|value| !value.is_empty());
    if !needs_slot(command) || set(SLOT_ENV_VAR) || set(BAZEL_TEST_ENV_VAR) {
        return Ok(None);
    }
    let (Some(dir), Some(memory)) = (slots_dir(env_get), available_memory()) else {
        return Ok(None);
    };
    let count = slot_count(memory);
    let label = workspace.display().to_string();
    let mut on_wait = |holder: &str| {
        let message = wait_message(holder);
        let _ = writeln!(err, "dx: {message}");
        if json {
            if let Ok(event) = dx_output::notice_event(&dx_output::NoticeEvent {
                level: "info".to_owned(),
                code: "waiting_for_build_slot".to_owned(),
                message,
                related_command: Some(command.name().to_owned()),
                scope: None,
                path: None,
                language: None,
                import: None,
            }) {
                let _ = dx_output::lifecycle::write_event(&mut *events, &event);
                let _ = events.flush();
            }
        }
    };
    match acquire(&dir, count, &label, &mut on_wait) {
        Ok(slot) => Ok(Some(slot)),
        Err(_) => dx_process::interrupted().map_or(Ok(None), Err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command as Process, Stdio};

    const HOLD_ENV: &str = "DX_SLOT_TEST_HOLD";
    const WAIT_ENV: &str = "DX_SLOT_TEST_WAIT";

    #[test]
    fn slot_holder_helper() {
        let Ok(dir) = std::env::var(HOLD_ENV) else {
            return;
        };
        let _slot = acquire(Path::new(&dir), 1, "helper-tree", &mut |_| {}).expect("slot");
        std::fs::write(Path::new(&dir).join("held"), b"1").expect("marker");
        std::thread::sleep(Duration::from_secs(120));
    }

    #[cfg(unix)]
    extern "C" fn record_stop(signo: libc::c_int) {
        dx_process::record_interrupt(signo);
    }

    #[cfg(unix)]
    #[test]
    fn slot_waiter_helper() {
        let Ok(dir) = std::env::var(WAIT_ENV) else {
            return;
        };
        unsafe {
            libc::signal(
                libc::SIGTERM,
                record_stop as *const () as libc::sighandler_t,
            );
        }
        let dir = Path::new(&dir);
        let result = acquire(dir, 1, "waiter-tree", &mut |_| {
            std::fs::write(dir.join("waiting"), b"1").expect("marker");
        });
        match (result, dx_process::interrupted()) {
            (Err(_), Some(signo)) => std::process::exit(dx_process::interrupted_code(signo)),
            _ => std::process::exit(1),
        }
    }

    fn wait_for(path: &Path, what: &str) {
        let start = std::time::Instant::now();
        while !path.exists() {
            assert!(start.elapsed() < Duration::from_secs(30), "{what}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn spawn_holder(dir: &Path) -> std::process::Child {
        let child = Process::new(std::env::current_exe().expect("exe"))
            .arg("slots::tests::slot_holder_helper")
            .arg("--exact")
            .env(HOLD_ENV, dir)
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn");
        wait_for(&dir.join("held"), "holder never took the slot");
        child
    }

    #[cfg(unix)]
    #[test]
    fn sigterm_ends_the_wait_with_128_plus_signal() {
        let scratch = dx_test_scratch::scratch("dx-slots-stop-");
        let dir = scratch.path().join("slots");
        let mut holder = spawn_holder(&dir);
        let mut waiter = Process::new(std::env::current_exe().expect("exe"))
            .arg("slots::tests::slot_waiter_helper")
            .arg("--exact")
            .env(WAIT_ENV, &dir)
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn waiter");
        wait_for(&dir.join("waiting"), "waiter never started waiting");
        let pid = i32::try_from(waiter.id()).expect("pid");
        assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
        let sent = std::time::Instant::now();
        let status = loop {
            if let Some(status) = waiter.try_wait().expect("try_wait") {
                break status;
            }
            assert!(
                sent.elapsed() < Duration::from_secs(10),
                "waiter ignored SIGTERM"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(128 + libc::SIGTERM));
        assert!(holder.try_wait().expect("holder").is_none());
        holder.kill().expect("kill");
        holder.wait().expect("wait");
    }

    #[test]
    fn count_follows_memory_with_bounds() {
        assert_eq!(slot_count(0), 1);
        assert_eq!(slot_count(4 << 30), 1);
        assert_eq!(slot_count(12 << 30), 2);
        assert_eq!(slot_count(19 << 30), 4);
        assert_eq!(slot_count(20 << 30), 4);
        assert_eq!(slot_count(24 << 30), 6);
        assert_eq!(slot_count(u64::MAX), MAX_SLOTS);
    }

    #[test]
    fn memory_text_parses() {
        assert_eq!(
            parse_meminfo("MemFree: 1 kB\nMemTotal:       2048 kB\n"),
            Some(2048 * 1024)
        );
        assert_eq!(parse_meminfo("MemFree: 1 kB\n"), None);
        assert_eq!(parse_cgroup_limit("8589934592\n"), Some(8 << 30));
        assert_eq!(parse_cgroup_limit("max\n"), None);
    }

    #[test]
    fn only_bazel_commands_take_slots() {
        for command in [
            Command::Build,
            Command::Test,
            Command::Coverage,
            Command::Run,
        ] {
            assert!(needs_slot(command));
        }
        for command in [
            Command::Lint,
            Command::Typecheck,
            Command::Format,
            Command::Deploy,
            Command::Generate,
            Command::Codegen,
            Command::Env,
            Command::Security,
            Command::License,
            Command::Docs,
            Command::ListTests,
            Command::Update,
        ] {
            assert!(needs_slot(command));
        }
        for command in [
            Command::Status,
            Command::Version,
            Command::Worktree,
            Command::Completion,
        ] {
            assert!(!needs_slot(command));
        }
    }

    #[test]
    fn cache_dir_follows_the_environment() {
        let get = |name: &str| match name {
            "XDG_CACHE_HOME" => Some("/x".to_owned()),
            _ => None,
        };
        assert_eq!(slots_dir(&get), Some(PathBuf::from("/x/dx/slots")));
        let home = |name: &str| (name == "HOME").then(|| "/h".to_owned());
        assert_eq!(slots_dir(&home), Some(PathBuf::from("/h/.cache/dx/slots")));
        assert_eq!(slots_dir(&|_| None), None);
    }

    #[test]
    fn a_second_taker_waits_until_the_first_drops() {
        let scratch = dx_test_scratch::scratch("dx-slots-wait-");
        let dir = scratch.path().join("slots");
        let first = acquire(&dir, 1, "tree-a", &mut |_| {}).expect("first");
        assert_eq!(first.index, 0);
        let mut seen = Vec::new();
        let second_dir = dir.clone();
        let waiter = std::thread::spawn(move || {
            let mut holders = Vec::new();
            let slot = acquire(&second_dir, 1, "tree-b", &mut |holder| {
                holders.push(holder.to_owned());
            })
            .expect("second");
            (slot.index, holders)
        });
        std::thread::sleep(Duration::from_millis(600));
        drop(first);
        let (index, holders) = waiter.join().expect("join");
        seen.extend(holders);
        assert_eq!(index, 0);
        assert_eq!(seen, vec!["tree-a".to_owned()]);
        assert_eq!(
            wait_message("tree-a"),
            "waiting for a build slot (held by tree-a)"
        );
    }

    #[test]
    fn a_free_slot_is_taken_without_waiting() {
        let scratch = dx_test_scratch::scratch("dx-slots-free-");
        let dir = scratch.path().join("slots");
        let first = acquire(&dir, 2, "a", &mut |_| {}).expect("first");
        let second = acquire(&dir, 2, "b", &mut |_| panic!("must not wait")).expect("second");
        assert_eq!((first.index, second.index), (0, 1));
    }

    #[test]
    fn a_crashed_holder_frees_its_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-crash-");
        let dir = scratch.path().join("slots");
        let mut child = spawn_holder(&dir);
        assert!(try_take(&dir, 1, "me").expect("try").is_none());
        child.kill().expect("kill");
        child.wait().expect("wait");
        let slot = acquire(&dir, 1, "me", &mut |_| {}).expect("slot after crash");
        assert_eq!(slot.index, 0);
    }

    #[test]
    fn a_nested_call_takes_no_slot() {
        let scratch = dx_test_scratch::scratch("dx-slots-nested-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let nested = |name: &str| match name {
            "XDG_CACHE_HOME" => Some(cache.clone()),
            SLOT_ENV_VAR => Some("1".to_owned()),
            _ => None,
        };
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Build,
            Path::new("/ws"),
            &nested,
            &mut err,
            false,
            &mut events,
        );
        assert!(matches!(slot, Ok(None)));
        assert!(!scratch.path().join("dx").exists());
    }

    #[test]
    fn a_dx_call_inside_a_bazel_test_takes_no_slot_when_all_are_full() {
        let scratch = dx_test_scratch::scratch("dx-slots-bazel-test-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let dir = scratch.path().join("dx").join("slots");
        let count = slot_count(available_memory().expect("memory"));
        let _held: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
            .collect();
        let inside_test = |name: &str| match name {
            "XDG_CACHE_HOME" => Some(cache.clone()),
            BAZEL_TEST_ENV_VAR => Some("1".to_owned()),
            _ => None,
        };
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Test,
            Path::new("/ws"),
            &inside_test,
            &mut err,
            true,
            &mut events,
        );
        assert!(matches!(slot, Ok(None)));
        assert!(err.is_empty() && events.is_empty());
    }

    #[test]
    fn waiting_prints_one_line_and_one_json_notice() {
        let scratch = dx_test_scratch::scratch("dx-slots-notice-");
        let cache = scratch.path().to_string_lossy().into_owned();
        let dir = scratch.path().join("dx").join("slots");
        let count = slot_count(available_memory().expect("memory"));
        let held: Vec<Slot> = (0..count)
            .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
            .collect();
        let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(600));
            drop(held);
        });
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Test,
            Path::new("/ws"),
            &get,
            &mut err,
            true,
            &mut events,
        );
        releaser.join().expect("join");
        assert!(matches!(slot, Ok(Some(_))));
        let text = String::from_utf8(err).expect("utf8");
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("waiting for a build slot (held by tree-0)"));
        let json = String::from_utf8(events).expect("utf8");
        assert_eq!(json.lines().count(), 1);
        assert!(json.contains("\"event\":\"notice\""));
        assert!(json.contains("waiting_for_build_slot"));
    }

    #[test]
    fn list_tests_and_update_wait_for_a_slot() {
        for command in [Command::ListTests, Command::Update] {
            let scratch = dx_test_scratch::scratch("dx-slots-list-update-");
            let cache = scratch.path().to_string_lossy().into_owned();
            let dir = scratch.path().join("dx").join("slots");
            let count = slot_count(available_memory().expect("memory"));
            let held: Vec<Slot> = (0..count)
                .map(|index| {
                    acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill")
                })
                .collect();
            let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
            let releaser = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(600));
                drop(held);
            });
            let mut err = Vec::new();
            let mut events = Vec::new();
            let slot = hold(
                command,
                Path::new("/ws"),
                &get,
                &mut err,
                false,
                &mut events,
            );
            releaser.join().expect("join");
            assert!(matches!(slot, Ok(Some(_))), "{}", command.name());
            let text = String::from_utf8(err).expect("utf8");
            assert!(text.contains("waiting for a build slot (held by tree-0)"));
            let nested = |name: &str| (name == SLOT_ENV_VAR).then(|| "1".to_owned());
            let slot = hold(
                command,
                Path::new("/ws"),
                &nested,
                &mut Vec::new(),
                false,
                &mut Vec::new(),
            );
            assert!(matches!(slot, Ok(None)));
        }
    }
}
