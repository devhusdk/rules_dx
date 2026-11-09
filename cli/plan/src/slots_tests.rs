use super::*;
use std::process::{Command as Process, Stdio};

fn hold(
    command: Command,
    workspace: &Path,
    env_get: &dyn Fn(&str) -> Option<String>,
    err: &mut dyn Write,
    json: bool,
    events: &mut dyn Write,
) -> Result<Option<Slot>, Stop> {
    Gate::from_env(workspace, env_get, json, &[], BuildSlots::Auto)
        .map(|gate| gate.with_pressure(Arc::new(Fixed(Some(false)))))
        .map_or(Ok(None), |gate| gate.hold(command, err, events))
}

const ALONE_ENV: &str = "DX_SLOT_TEST_ALONE";

/// Runs `body` in a child test process so no other test touches the process share.
fn alone(test: &str, body: impl FnOnce()) {
    if std::env::var_os(ALONE_ENV).is_some() {
        body();
        return;
    }
    let status = Process::new(std::env::current_exe().expect("exe"))
        .arg(format!("slots::tests::{test}"))
        .arg("--exact")
        .env(ALONE_ENV, "1")
        .stdout(Stdio::null())
        .status()
        .expect("spawn");
    assert!(status.success(), "{test} failed in its own process");
}

const HOLD_ENV: &str = "DX_SLOT_TEST_HOLD";

#[test]
fn slot_holder_helper() {
    let Ok(dir) = std::env::var(HOLD_ENV) else {
        return;
    };
    let _slot = acquire(Path::new(&dir), 1, "helper-tree", &mut |_| {}).expect("slot");
    std::fs::write(Path::new(&dir).join("held"), b"1").expect("marker");
    std::thread::sleep(Duration::from_secs(120));
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

#[test]
fn count_follows_memory_with_bounds() {
    assert_eq!(slot_count(0, 0), 1);
    assert_eq!(slot_count(4 << 30, 0), 1);
    assert_eq!(slot_count(8 << 30, 0), 2);
    assert_eq!(slot_count(16 << 30, 0), 5);
    assert_eq!(slot_count(16 << 30, 6 << 30), 3);
    assert_eq!(slot_count(24 << 30, 0), 8);
    assert_eq!(slot_count(u64::MAX, 0), MAX_SLOTS);
    assert_eq!(slot_count(12 << 30, 6 << 30), 2);
    assert_eq!(slot_count(20 << 30, 6 << 30), 4);
    assert_eq!(slot_count(24 << 30, 6 << 30), 6);
}

#[test]
fn remote_count_follows_memory_with_bounds() {
    assert_eq!(remote_slot_count(0, 0), 1);
    assert_eq!(remote_slot_count(19 << 30, 0), 12);
    assert_eq!(remote_slot_count(19 << 30, 6 << 30), 8);
    assert_eq!(remote_slot_count(u64::MAX, 0), MAX_SLOTS);
    let extras = |memory, reserved| {
        remote_slot_count(memory, reserved).saturating_sub(slot_count(memory, reserved))
    };
    assert_eq!(extras(19 << 30, 0), 6);
    assert_eq!(extras(19 << 30, 6 << 30), 4);
    assert_eq!(extras(2 << 30, 0), 0);
}

#[test]
fn extras_sit_above_the_pool() {
    let rules_dx = Gate::new(PathBuf::from("/slots"), 4, "r".to_owned(), false)
        .reserving(6)
        .extra(4, true)
        .layout();
    assert_eq!(
        (
            rules_dx.count,
            rules_dx.reserve.clone(),
            rules_dx.extras.clone()
        ),
        (4, 4..6, 6..10)
    );
    assert_eq!(
        rules_dx.counted().collect::<Vec<_>>(),
        [0, 1, 2, 3, 6, 7, 8, 9]
    );
    let consumer = Gate::new(PathBuf::from("/slots"), 6, "c".to_owned(), false)
        .reserving(6)
        .extra(6, false)
        .layout();
    assert_eq!(
        (consumer.count, consumer.reserve, consumer.extras),
        (6, 6..6, 6..12)
    );
}

#[derive(Debug)]
struct Fixed(Option<bool>);

impl Pressure for Fixed {
    fn high(&self) -> Option<bool> {
        self.0
    }
}

fn remote_gate(dir: &Path, remote: bool, high: Option<bool>) -> Gate {
    Gate::new(dir.to_path_buf(), 1, "remote".to_owned(), false)
        .extra(2, remote)
        .with_pressure(Arc::new(Fixed(high)))
}

fn free(dir: &Path, index: usize) -> bool {
    matches!(lock_slot(&slot_path(dir, index), false), Ok(Some(_)))
}

#[test]
fn a_slot_without_a_holder_file_still_counts_as_held() {
    let scratch = dx_test_scratch::scratch("dx-slots-no-holder-");
    let dir = scratch.path().join("slots");
    let slot = try_take(&dir, 2, "busy").expect("try").expect("slot");
    std::fs::remove_file(holder_path(&dir, slot.index)).expect("remove holder");
    assert_eq!(held(&dir, 0..2, usize::MAX), 2);
}

#[test]
fn a_local_gate_never_takes_an_extra() {
    let scratch = dx_test_scratch::scratch("dx-slots-local-extra-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
    let gate = remote_gate(&dir, false, Some(false));
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    std::thread::sleep(POLL * 2);
    assert!(free(&dir, 1) && free(&dir, 2));
    drop(fill);
    assert_eq!(waiter.join().expect("join").0.index, 0);
}

#[test]
fn a_remote_gate_takes_a_base_slot_then_an_extra_when_calm() {
    let scratch = dx_test_scratch::scratch("dx-slots-remote-extra-");
    let dir = scratch.path().join("slots");
    let gate = remote_gate(&dir, true, Some(false));
    let (base, base_err, _) = take_gate(&gate);
    let (extra, extra_err, _) = take_gate(&gate);
    assert_eq!((base.index, extra.index), (0, 1));
    assert_eq!((base_err.as_str(), extra_err.as_str()), ("", ""));
    assert!(extra_mark(&dir).is_file());
}

#[test]
fn a_remote_gate_waits_under_pressure_or_without_a_signal() {
    for high in [Some(true), None] {
        let scratch = dx_test_scratch::scratch("dx-slots-remote-pressure-");
        let dir = scratch.path().join("slots");
        let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
        let gate = remote_gate(&dir, true, high);
        let (waiter, rx) = spawn_take(gate);
        announced(&rx);
        std::thread::sleep(POLL * 2);
        assert!(free(&dir, 1) && free(&dir, 2));
        drop(fill);
        let (slot, err, _) = waiter.join().expect("join");
        assert_eq!(slot.index, 0);
        let want = match high {
            Some(true) => wait_message("fill; extra slots paused under memory pressure"),
            _ => wait_message("fill"),
        };
        assert_eq!(err, format!("dx: {want}\n"));
    }
}

#[test]
fn a_remote_waiter_does_not_jump_the_line_through_a_free_extra() {
    let scratch = dx_test_scratch::scratch("dx-slots-remote-line-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
    let queued = enqueue(&dir, "").expect("queued");
    let gate = remote_gate(&dir, true, Some(false));
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    std::thread::sleep(POLL * 2);
    assert!(!waiter.is_finished(), "a remote waiter must keep its place");
    assert!(free(&dir, 1) && free(&dir, 2));
    drop(queued);
    let (slot, err, _) = waiter.join().expect("join");
    assert_eq!(slot.index, 1);
    assert_eq!(
        err,
        format!("dx: {}\n", wait_message("fill, 1 ahead in line"))
    );
    drop(fill);
}

#[test]
fn held_extras_under_pressure_add_no_paused_suffix() {
    let scratch = dx_test_scratch::scratch("dx-slots-remote-full-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
    let extras: Vec<Slot> = (1..3)
        .map(|index| {
            scan(&dir, index..index + 1, &format!("extra-{index}"), &|_| true)
                .0
                .expect("scan")
                .expect("extra")
        })
        .collect();
    // Each clock read is past the cap, so the second read releases the pause.
    let start = Instant::now();
    let ticks = std::sync::atomic::AtomicU32::new(0);
    let clock = move || {
        let n = ticks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        start + PRESSURE_WAIT * (n + 1)
    };
    let gate = remote_gate(&dir, true, Some(true)).with_clock(clock);
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    drop(fill);
    let (slot, err, _) = waiter.join().expect("join");
    assert_eq!(slot.index, 0);
    assert_eq!(err, format!("dx: {}\n", wait_message("fill and 2 more")));
    drop(extras);
}

#[derive(Debug)]
struct Switch(std::sync::atomic::AtomicBool);

impl Pressure for Switch {
    fn high(&self) -> Option<bool> {
        Some(self.0.load(std::sync::atomic::Ordering::SeqCst))
    }
}

/// A clock that starts now and moves only when told.
fn fake_clock() -> (
    impl Fn() -> Instant + Send + Sync + 'static,
    Arc<Mutex<Duration>>,
) {
    let start = Instant::now();
    let offset = Arc::new(Mutex::new(Duration::ZERO));
    let read = offset.clone();
    (move || start + *read.lock().expect("clock"), offset)
}

#[test]
fn a_new_gate_reads_no_host_pressure() {
    let gate = Gate::new(PathBuf::from("/slots"), 1, "g".to_owned(), false);
    assert_eq!(gate.pressure.high(), None);
}

fn base_gate(dir: &Path, high: Arc<dyn Pressure>) -> Gate {
    Gate::new(dir.to_path_buf(), 2, "gate".to_owned(), true).with_pressure(high)
}

#[test]
fn a_free_base_slot_waits_under_pressure_while_another_is_held() {
    let scratch = dx_test_scratch::scratch("dx-slots-base-pressure-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
    let switch = Arc::new(Switch(std::sync::atomic::AtomicBool::new(true)));
    let (waiter, rx) = spawn_take(base_gate(&dir, switch.clone()));
    announced(&rx);
    std::thread::sleep(POLL * 2);
    assert!(!waiter.is_finished());
    switch.0.store(false, std::sync::atomic::Ordering::SeqCst);
    let (slot, err, events) = waiter.join().expect("join");
    assert_eq!(slot.index, 1);
    assert_eq!(
        err,
        format!(
            "dx: {}\n",
            wait_message("fill; paused under memory pressure")
        )
    );
    assert!(events.contains("\"reason\":\"pressure\""), "{events}");
    drop(fill);
}

#[test]
fn the_pause_ends_after_the_cap_even_under_pressure() {
    let scratch = dx_test_scratch::scratch("dx-slots-base-cap-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
    let (clock, offset) = fake_clock();
    let gate = base_gate(&dir, Arc::new(Fixed(Some(true)))).with_clock(clock);
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    std::thread::sleep(POLL * 2);
    assert!(!waiter.is_finished());
    *offset.lock().expect("clock") = PRESSURE_WAIT;
    let (slot, _, _) = waiter.join().expect("join");
    assert_eq!(slot.index, 1);
    drop(fill);
}

#[test]
fn nothing_pauses_when_no_other_slot_is_held_or_there_is_no_signal() {
    let scratch = dx_test_scratch::scratch("dx-slots-base-free-");
    let dir = scratch.path().join("slots");
    let gate = base_gate(&dir, Arc::new(Fixed(Some(true))));
    let (slot, err, _) = take_gate(&gate);
    assert_eq!((slot.index, err.as_str()), (0, ""));
    let blind = base_gate(&dir, Arc::new(Fixed(None)));
    let (other, err, _) = take_gate(&blind);
    assert_eq!((other.index, err.as_str()), (1, ""));
}

#[test]
fn a_normal_wait_reports_the_held_reason() {
    let scratch = dx_test_scratch::scratch("dx-slots-base-held-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
    let gate = Gate::new(dir.clone(), 1, "gate".to_owned(), true)
        .with_pressure(Arc::new(Fixed(Some(false))));
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    drop(fill);
    let (_, _, events) = waiter.join().expect("join");
    assert!(events.contains("\"reason\":\"held\""), "{events}");
}

#[test]
fn a_local_rules_dx_gate_counts_a_held_extra_but_not_the_reserve() {
    let scratch = dx_test_scratch::scratch("dx-slots-local-counted-");
    let dir = scratch.path().join("slots");
    let layout = Gate::new(dir.clone(), 4, "r".to_owned(), false)
        .reserving(6)
        .extra(4, false)
        .layout();
    let take = |index: usize| {
        scan(&dir, index..index + 1, "other", &|_| true)
            .0
            .expect("scan")
            .expect("slot")
    };
    let reserve = take(4);
    assert_eq!(held(&dir, layout.counted(), 0), 1);
    let extra = take(6);
    assert_eq!(held(&dir, layout.counted(), 0), 2);
    let base = take(1);
    assert_eq!(held(&dir, layout.counted(), 0), 3);
    drop((reserve, extra, base));
}

#[test]
fn extra_grants_are_spaced_out() {
    let scratch = dx_test_scratch::scratch("dx-slots-remote-spacing-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 1, "fill").expect("try").expect("fill");
    std::fs::write(extra_mark(&dir), "earlier").expect("mark");
    let gate = remote_gate(&dir, true, Some(false));
    let waiting = gate.clone();
    let (waiter, rx) = spawn_take(waiting);
    announced(&rx);
    std::thread::sleep(POLL * 2);
    assert!(free(&dir, 1) && free(&dir, 2));
    drop(fill);
    let base = waiter.join().expect("join").0;
    assert_eq!(base.index, 0);
    let old = SystemTime::now() - EXTRA_SPACING - Duration::from_secs(1);
    File::options()
        .write(true)
        .open(extra_mark(&dir))
        .and_then(|file| file.set_modified(old))
        .expect("age mark");
    let (extra, err, _) = take_gate(&gate);
    assert_eq!((extra.index, err.as_str()), (1, ""));
}

#[test]
fn a_held_extra_counts_toward_the_share() {
    alone("a_held_extra_counts_toward_the_share", || {
        let scratch = dx_test_scratch::scratch("dx-slots-remote-share-");
        let dir = scratch.path().join("slots");
        let gate = remote_gate(&dir, true, Some(false)).sharing(12, 12 << 30, 0);
        let (base, _, _) = take_gate(&gate);
        let (extra, _, _) = take_gate(&gate);
        assert_eq!(extra.index, 1);
        let share = extra.share().expect("share");
        assert_eq!((share.cpus, share.memory_mb), (6, 6 << 10));
        drop(base);
        let local = remote_gate(&dir, false, None).sharing(12, 12 << 30, 0);
        let (again, _, _) = take_gate(&local);
        assert_eq!(again.index, 0);
        assert_eq!(again.share().expect("share").memory_mb, 6 << 10);
    });
}

#[test]
fn share_splits_the_host_over_held_slots() {
    let share = |cpus, memory, reserved, held| {
        let share = local_share(cpus, memory, reserved, held);
        (share.cpus, share.memory_mb)
    };
    assert_eq!(share(12, 19 << 30, 0, 1), (12, 19 << 10));
    assert_eq!(share(12, 19 << 30, 0, 2), (6, 9728));
    assert_eq!(share(12, 19 << 30, 0, 4), (3, 4864));
    assert_eq!(share(4, 8 << 30, 0, 4), (2, 2048));
    assert_eq!(share(1, 8 << 30, 0, 1), (2, 8192));
    assert_eq!(share(12, 19 << 30, 6 << 30, 2), (6, 6656));
    assert_eq!(share(12, 4 << 30, 6 << 30, 2), (6, 1));
    assert_eq!(share(12, 19 << 30, 0, 0), (12, 19 << 10));
}

#[cfg(unix)]
#[test]
fn a_child_forked_while_a_slot_is_held_keeps_it_until_exec() {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt;
    let scratch = dx_test_scratch::scratch("dx-slots-fork-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    let slot = try_take(&dir, 1, "held").expect("try").expect("free");
    let (mut reader, writer) = UnixStream::pair().expect("pair");
    let fd = writer.as_raw_fd();
    let spawner = std::thread::spawn(move || {
        let mut cmd = Process::new("/bin/sh");
        cmd.args(["-c", "exit 0"]);
        // SAFETY: the hook only calls write and nanosleep, both async-signal-safe.
        unsafe {
            cmd.pre_exec(move || {
                let byte = 1u8;
                libc::write(fd, (&byte as *const u8).cast(), 1);
                let nap = libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 150_000_000,
                };
                libc::nanosleep(&nap, std::ptr::null_mut());
                Ok(())
            });
        }
        let mut child = cmd.spawn().expect("spawn");
        drop(writer);
        child.wait().expect("wait");
    });
    let mut byte = [0u8; 1];
    reader.read_exact(&mut byte).expect("forked");
    drop(slot);
    assert!(
        try_take(&dir, 1, "probe").expect("try").is_none(),
        "the forked child holds the lock until it execs"
    );
    assert!(slot_settles_free(&dir, 1));
    spawner.join().expect("spawner");
}

#[test]
fn held_counts_locked_slots_and_the_caller() {
    let scratch = dx_test_scratch::scratch("dx-slots-held-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    assert_eq!(held(&dir, 0..3, 0), 1);
    let mut child = spawn_holder(&dir);
    assert_eq!(held(&dir, 0..3, 1), 2);
    assert_eq!(held(&dir, 0..3, 0), 1);
    let _ = child.kill();
    let _ = child.wait();
    std::fs::write(holder_path(&dir, 2), "gone").expect("stale holder");
    assert!(holder_path(&dir, 0).exists());
    assert_eq!(held(&dir, 0..3, 1), 1);
    assert!(slot_settles_free(&dir, 1));
}

#[test]
fn a_taken_slot_sets_the_share_until_it_drops() {
    alone(
        "a_taken_slot_sets_the_share_until_it_drops",
        take_sets_the_share,
    );
}

fn take_sets_the_share() {
    let scratch = dx_test_scratch::scratch("dx-slots-take-share-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 2, "me".to_owned(), false).sharing(600, 9 << 30, 1 << 30);
    let other = try_take(&dir, 2, "other").expect("try").expect("other");
    let (slot, _, _) = take_gate(&gate);
    let want = dx_process::LocalShare {
        cpus: 300,
        memory_mb: 4096,
    };
    assert_eq!(slot.share(), Some(want));
    assert_eq!(dx_process::share(), Some(want));
    drop(slot);
    assert_eq!(dx_process::share(), None);
    drop(other);
    let plain = Gate::new(dir, 2, "plain".to_owned(), false);
    let (slot, _, _) = take_gate(&plain);
    assert_eq!(slot.share(), None);
    assert_eq!(dx_process::share(), None);
}

#[test]
fn share_shrinks_when_another_slot_is_taken() {
    alone("share_shrinks_when_another_slot_is_taken", || {
        let scratch = dx_test_scratch::scratch("dx-slots-live-share-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 2, "me".to_owned(), false).sharing(12, 12 << 30, 0);
        let (slot, _, _) = take_gate(&gate);
        let cpus = || dx_process::share().map(|share| share.cpus);
        assert_eq!(cpus(), Some(12));
        let other = try_take(&dir, 2, "other").expect("try").expect("other");
        assert_eq!(cpus(), Some(6));
        drop(other);
        assert_eq!(cpus(), Some(12));
        drop(slot);
        assert_eq!(cpus(), None);
    });
}

#[test]
fn slot_runner_exports_the_current_share() {
    use dx_process::Runner as _;
    alone("slot_runner_exports_the_current_share", || {
        let scratch = dx_test_scratch::scratch("dx-slots-live-env-");
        let dir = scratch.path().join("slots");
        let gate = Gate::new(dir.clone(), 2, "me".to_owned(), false).sharing(12, 12 << 30, 0);
        let (slot, _, _) = take_gate(&gate);
        let inner = EnvRecorder::default();
        let held = slot.runner(&inner);
        held.run(&["bazel".to_owned()], scratch.path(), &[])
            .expect("run");
        let other = try_take(&dir, 2, "other").expect("try").expect("other");
        held.run(&["bazel".to_owned()], scratch.path(), &[])
            .expect("run");
        drop(other);
        let shares: Vec<String> = inner
            .seen
            .borrow()
            .iter()
            .flat_map(|(_, env)| env.iter())
            .filter(|(name, _)| name == SHARE_ENV_VAR)
            .map(|(_, value)| value.clone())
            .collect();
        assert_eq!(shares, vec!["cpu=12,memory=12288", "cpu=6,memory=6144"]);
    });
}

#[test]
fn a_nested_call_reuses_the_parent_share() {
    alone(
        "a_nested_call_reuses_the_parent_share",
        nested_reuses_the_share,
    );
}

fn nested_reuses_the_share() {
    let env = |slot: Option<&'static str>, share: &'static str| {
        move |name: &str| match name {
            SLOT_ENV_VAR => slot.map(str::to_owned),
            SHARE_ENV_VAR => Some(share.to_owned()),
            _ => None,
        }
    };
    dx_process::set_share(None);
    inherit_share(&env(None, "cpu=5,memory=700"));
    assert_eq!(dx_process::share(), None);
    inherit_share(&env(Some("1"), "cpu=5,memory=bad"));
    assert_eq!(dx_process::share(), None);
    inherit_share(&env(Some("1"), "cpu=5,memory=700"));
    let want = dx_process::LocalShare {
        cpus: 5,
        memory_mb: 700,
    };
    assert_eq!(dx_process::share(), Some(want));
    dx_process::clear_share(&want);
}

fn rules_dx_layout(prefix: &str) -> tempfile::TempDir {
    let scratch = dx_test_scratch::scratch(prefix);
    std::fs::create_dir_all(scratch.path().join("cli/cli")).expect("dirs");
    std::fs::write(scratch.path().join("cli/cli/BUILD.bazel"), "").expect("build");
    std::fs::write(
        scratch.path().join("MODULE.bazel"),
        "module(\n    name = \"rules_dx\",\n    version = \"0.0.0\",\n)\n",
    )
    .expect("module");
    scratch
}

#[test]
fn consumer_workspaces_reserve_nothing() {
    let none = |_: &str| None;
    assert_eq!(nested_reserve(Path::new("/ws"), &none), 0);
    let scratch = dx_test_scratch::scratch("dx-slots-consumer-");
    std::fs::create_dir_all(scratch.path().join("cli/cli")).expect("dirs");
    std::fs::write(scratch.path().join("cli/cli/BUILD.bazel"), "").expect("build");
    std::fs::write(
        scratch.path().join("MODULE.bazel"),
        "module(\n    name = \"my_app\",\n)\nbazel_dep(name = \"rules_dx\", version = \"1.0.0\")\n",
    )
    .expect("module");
    assert_eq!(nested_reserve(scratch.path(), &none), 0);
}

#[test]
fn rules_dx_gate_spans_the_whole_pool() {
    let scratch = rules_dx_layout("dx-slots-pool-");
    let cache = scratch.path().to_string_lossy().into_owned();
    let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    let memory = available_memory().expect("memory");
    let gate = Gate::from_env(scratch.path(), &get, false, &[], BuildSlots::Auto).expect("gate");
    let resources = gate.resources.expect("resources");
    let cpus = std::thread::available_parallelism().expect("cpus").get();
    assert_eq!(resources.cpus, u64::try_from(cpus).expect("u64"));
    assert_eq!((resources.memory, resources.reserved), (memory, 6 << 30));
    assert_eq!(gate.count, slot_count(memory, 6 << 30));
    assert_eq!(gate.pool, slot_count(memory, 0));
    let consumer =
        Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Auto).expect("gate");
    assert_eq!(consumer.pool, consumer.count);
    let narrow = Gate::new(PathBuf::from("/slots"), 3, "r".to_owned(), false).reserving(1);
    assert_eq!(narrow.pool, 3);
}

#[test]
fn rules_dx_reserves_the_default_nested_servers() {
    let scratch = rules_dx_layout("dx-slots-rules-dx-");
    assert_eq!(nested_reserve(scratch.path(), &|_| None), 3 * (2 << 30));
}

#[test]
fn rules_dx_reserve_is_zero_when_tag_filters_exclude_nested_bazel() {
    for (filters, reserved) in [
        ("-resources:nested_bazel:1,-exclusive", false),
        ("-resources:nested_bazel", false),
        ("-exclusive", true),
        ("smoke", false),
        ("smoke,resources:nested_bazel:1", true),
    ] {
        let scratch = rules_dx_layout("dx-slots-tag-filters-");
        std::fs::write(
            scratch.path().join(".bazelrc"),
            format!("test --test_tag_filters={filters}\n"),
        )
        .expect("rc");
        let want = if reserved { 3 * (2 << 30) } else { 0 };
        assert_eq!(nested_reserve(scratch.path(), &|_| None), want, "{filters}");
    }
}

#[test]
fn rules_dx_reserve_follows_the_nested_slot_env() {
    let scratch = rules_dx_layout("dx-slots-rules-dx-env-");
    let with = |value: &'static str| {
        move |name: &str| (name == NESTED_BAZEL_SLOTS_ENV).then(|| value.to_owned())
    };
    assert_eq!(nested_reserve(scratch.path(), &with("5")), 10 << 30);
    for fallback in ["0", "x", ""] {
        assert_eq!(nested_reserve(scratch.path(), &with(fallback)), 6 << 30);
    }
}

#[test]
fn auto_gate_reserve_is_the_shared_reserve_range() {
    for gib in [4_u64, 8, 12, 16, 24, 64] {
        let memory = gib << 30;
        let reserved = u64::from(DEFAULT_NESTED_BAZEL_SLOTS as u16) * BYTES_PER_NESTED_BAZEL;
        let size = sizing(BuildSlots::Auto, memory, reserved).expect("sizing");
        let range = dx_process::slot_pool::reserve_range(memory, DEFAULT_NESTED_BAZEL_SLOTS);
        assert_eq!(size.count..size.pool, range, "{gib} GiB");
    }
}

#[test]
fn a_consumer_gate_leaves_the_held_reserve_alone() {
    let scratch = dx_test_scratch::scratch("dx-slots-reserve-held-");
    let dir = scratch.path().join("dx").join("slots");
    let memory = 16_u64 << 30;
    let reserve = dx_process::slot_pool::reserve_range(memory, DEFAULT_NESTED_BAZEL_SLOTS);
    let held = dx_process::slot_pool::hold_reserve(&dir, reserve.clone());
    assert!(held.busy.is_empty());
    let count = reserve.start;
    let slots: Vec<Slot> = (0..count)
        .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("slot"))
        .collect();
    assert!(slots.iter().all(|slot| slot.index < count));
    for index in reserve {
        assert!(!free(&dir, index), "reserve index {index} is held");
    }
}

#[test]
fn slot_memory_parses_sizes_and_scales_counts() {
    assert_eq!(parse_slot_memory("4g"), Some(4 << 30));
    assert_eq!(parse_slot_memory(" 512M "), Some(512 << 20));
    assert_eq!(parse_slot_memory("2048"), Some(2048 << 20));
    for bad in ["", "0", "g", "-1g", "1.5g", "4x"] {
        assert_eq!(parse_slot_memory(bad), None, "{bad}");
    }
    let env =
        |value: &'static str| move |name: &str| (name == SLOT_MEMORY_ENV).then(|| value.to_owned());
    assert_eq!(slot_bytes(&env("6g")), 6 << 30);
    assert_eq!(slot_bytes(&env("nope")), BYTES_PER_SLOT);
    assert_eq!(slot_bytes(&|_: &str| None), BYTES_PER_SLOT);
    let memory = per_slot_scaled(24 << 30, 6 << 30);
    assert_eq!(slot_count(memory, 0), 4);
    assert_eq!(
        slot_count(per_slot_scaled(24 << 30, 1 << 30), 0),
        MAX_SLOTS.min(24)
    );
}

#[test]
fn available_memory_text_parses() {
    let text = "MemTotal: 4096 kB\nMemAvailable:    2048 kB\n";
    assert_eq!(parse_mem_available(text), Some(2 << 20));
    assert_eq!(parse_mem_available("MemAvailable: x kB\n"), None);
    assert_eq!(parse_mem_available("MemTotal: 1 kB\n"), None);
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
    assert_eq!(parse_cgroup_limit("9223372036854771712\n"), None);
    assert_eq!(parse_cgroup_limit("9223372036854710272\n"), None);
    assert_eq!(parse_cgroup_limit("-1\n"), None);
    assert_eq!(parse_cgroup_limit("2147483648\n"), Some(2 << 30));
}

#[test]
fn extras_follow_the_cgroup_limit() {
    let scratch = dx_test_scratch::scratch("dx-cgroup-extras-");
    let root = scratch.path();
    std::fs::create_dir_all(root.join("a")).expect("dirs");
    std::fs::write(root.join("a/memory.max"), format!("{}\n", 6u64 << 30)).expect("write");
    let host = 19u64 << 30;
    let limit = cgroup_limit_in("0::/a\n", root).expect("limit");
    let memory = host.min(limit);
    let extras = remote_slot_count(memory, 0).saturating_sub(slot_count(memory, 0));
    let host_extras = remote_slot_count(host, 0).saturating_sub(slot_count(host, 0));
    assert!(extras < host_extras);
    assert_eq!(extras, 2);
}

#[test]
fn cgroup_limit_reads_v1_and_v2() {
    let put = |root: &Path, rel: &str, text: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).expect("dirs");
        std::fs::write(path, text).expect("write");
    };
    let scratch = dx_test_scratch::scratch("dx-cgroup-v2-");
    let root = scratch.path();
    put(root, "a/memory.max", "1000\n");
    std::fs::create_dir_all(root.join("a/b")).expect("dirs");
    assert_eq!(cgroup_limit_in("0::/a/b\n", root), Some(1000));
    assert_eq!(cgroup_limit_in("", root), None);

    let scratch = dx_test_scratch::scratch("dx-cgroup-v1-");
    let root = scratch.path();
    put(root, "memory/memory.limit_in_bytes", "2000\n");
    assert_eq!(cgroup_limit_in("4:memory:/docker/x\n", root), Some(2000));
    assert_eq!(cgroup_limit_in("5:cpu,memory:/x\n", root), Some(2000));
    assert_eq!(cgroup_limit_in("5:cpu:/x\n", root), None);
    put(root, "a/memory.max", "3000\n");
    let hybrid = "4:memory:/docker/x\n0::/a\n";
    assert_eq!(cgroup_limit_in(hybrid, root), Some(2000));
    put(root, "a/memory.max", "1500\n");
    assert_eq!(cgroup_limit_in(hybrid, root), Some(1500));
    put(root, "memory/p/c/memory.limit_in_bytes", "5000\n");
    put(root, "memory/p/memory.limit_in_bytes", "700\n");
    assert_eq!(cgroup_limit_in("4:memory:/p/c\n", root), Some(700));

    let scratch = dx_test_scratch::scratch("dx-cgroup-v1-unlimited-");
    let root = scratch.path();
    put(
        root,
        "memory/memory.limit_in_bytes",
        "9223372036854771712\n",
    );
    assert_eq!(cgroup_limit_in("4:memory:/x\n", root), None);
}

#[derive(Default)]
struct EnvRecorder {
    #[allow(clippy::type_complexity)]
    seen: RefCell<Vec<(&'static str, Vec<(String, String)>)>>,
}

impl EnvRecorder {
    fn record(&self, method: &'static str, env: &[(&str, &str)]) {
        let env = env
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        self.seen.borrow_mut().push((method, env));
    }
}

impl dx_process::Runner for EnvRecorder {
    fn run(
        &self,
        _argv: &[String],
        _cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<dx_process::ChildStatus> {
        self.record("run", env);
        Ok(dx_process::ChildStatus::exited(0))
    }

    fn run_hermetic(
        &self,
        _argv: &[String],
        _cwd: &Path,
        env: &[(&str, &str)],
    ) -> io::Result<dx_process::ChildStatus> {
        self.record("run_hermetic", env);
        Ok(dx_process::ChildStatus::exited(3))
    }

    fn run_withdrawable(
        &self,
        _argv: &[String],
        _cwd: &Path,
        env: &[(&str, &str)],
        _withdraw: &dyn Fn() -> bool,
    ) -> io::Result<Option<dx_process::ChildStatus>> {
        self.record("run_withdrawable", env);
        Ok(Some(dx_process::ChildStatus::exited(4)))
    }

    fn free_disk_bytes(&self, _path: &Path) -> Option<u64> {
        Some(9)
    }

    fn pinned_tool(&self, _var: &str) -> dx_process::PinnedVar {
        dx_process::PinnedVar::NotAbsolute
    }
}

#[test]
fn slot_runner_forwards_the_pinned_tool_state() {
    use dx_process::Runner as _;
    let scratch = dx_test_scratch::scratch("dx-slots-pinned-");
    let dir = scratch.path().join("slots");
    let slot = try_take(&dir, 1, "pinned").expect("try").expect("free");
    let inner = EnvRecorder::default();
    let held = slot.runner(&inner);
    assert_eq!(
        held.pinned_tool("DX_X_BIN"),
        dx_process::PinnedVar::NotAbsolute
    );
}

#[test]
fn slot_runner_marks_children_while_the_slot_is_held() {
    use dx_process::Runner as _;
    let scratch = dx_test_scratch::scratch("dx-slots-env-");
    let dir = scratch.path().join("slots");
    let slot = try_take(&dir, 1, "env").expect("try").expect("free");
    let inner = EnvRecorder::default();
    let held = slot.runner(&inner);
    let argv = ["bazel".to_owned()];
    let cwd = scratch.path();
    let env = [("KEEP", "yes")];
    assert_eq!(held.run(&argv, cwd, &env).expect("run").code, Some(0));
    assert_eq!(
        held.run_withdrawable(&argv, cwd, &env, &|| true)
            .expect("withdrawable")
            .map(|status| status.code),
        Some(Some(4))
    );
    assert_eq!(
        held.run_hermetic(&argv, cwd, &env).expect("hermetic").code,
        Some(3)
    );
    assert_eq!(held.free_disk_bytes(cwd), Some(9));
    let marked = vec![
        ("KEEP".to_owned(), "yes".to_owned()),
        (SLOT_ENV_VAR.to_owned(), "1".to_owned()),
    ];
    let plain = vec![("KEEP".to_owned(), "yes".to_owned())];
    assert_eq!(
        *inner.seen.borrow(),
        vec![
            ("run", marked.clone()),
            ("run_withdrawable", marked),
            ("run_hermetic", plain),
        ]
    );
}

#[test]
fn only_bazel_commands_take_slots() {
    for command in [Command::Build, Command::Test, Command::Coverage] {
        assert!(needs_slot(command));
    }
    for command in [Command::Run, Command::Watch, Command::Update] {
        assert!(!needs_slot(command));
    }
    for command in [
        Command::Lint,
        Command::Typecheck,
        Command::Format,
        Command::Deploy,
        Command::Generate,
        Command::Sync,
        Command::Security,
        Command::License,
        Command::Docs,
    ] {
        assert!(needs_slot(command));
    }
    for command in [Command::Status, Command::Worktree, Command::Completion] {
        assert!(!needs_slot(command));
    }
}

#[test]
fn slot_runner_passes_the_share_to_children() {
    use dx_process::Runner as _;
    let scratch = dx_test_scratch::scratch("dx-slots-share-env-");
    let dir = scratch.path().join("slots");
    let mut slot = try_take(&dir, 1, "env").expect("try").expect("free");
    slot.sizing = Some(Arc::new(ShareSizing {
        dir: dir.clone(),
        indices: vec![0],
        own: 0,
        resources: Resources {
            cpus: 7,
            memory: 777 << 20,
            reserved: 0,
        },
    }));
    let inner = EnvRecorder::default();
    let held = slot.runner(&inner);
    held.run(&["bazel".to_owned()], scratch.path(), &[])
        .expect("run");
    assert_eq!(
        *inner.seen.borrow(),
        vec![(
            "run",
            vec![
                (SLOT_ENV_VAR.to_owned(), "1".to_owned()),
                (SHARE_ENV_VAR.to_owned(), "cpu=7,memory=777".to_owned()),
            ]
        )]
    );
}

#[test]
fn a_gate_ignores_slot_env_set_after_it_was_built() {
    let scratch = dx_test_scratch::scratch("dx-slots-gate-env-");
    let cache = scratch.path().to_string_lossy().into_owned();
    let nested = std::cell::Cell::new(false);
    let get = |name: &str| match name {
        "XDG_CACHE_HOME" => Some(cache.clone()),
        SLOT_ENV_VAR => nested.get().then(|| "1".to_owned()),
        _ => None,
    };
    let gate = Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Auto).expect("gate");
    nested.set(true);
    assert!(Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Auto).is_none());
    let slot = with_gate(Some(gate), || {
        take(Command::Run, &mut Vec::new(), &mut Vec::new())
    });
    assert!(matches!(
        slot,
        Ok(Some(Slot {
            sizing: Some(_),
            ..
        }))
    ));
    assert!(
        scratch
            .path()
            .join("dx")
            .join("slots")
            .join("slot-0.lock")
            .exists()
    );
}

fn counting(
    message: Option<&'static str>,
) -> (
    Arc<std::sync::atomic::AtomicUsize>,
    impl Fn() -> Option<String> + Send + Sync + 'static,
) {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&calls);
    let reclaim = move || {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        message.map(str::to_owned)
    };
    (calls, reclaim)
}

const RECLAIMED: &str =
    "low disk: 5.0 GiB free; removed the output bases of free slot(s) 2; 30.0 GiB free now";

#[test]
fn take_runs_the_reclaimer_once_and_reports_it() {
    for json in [true, false] {
        let scratch = dx_test_scratch::scratch("dx-slots-reclaim-");
        let (calls, reclaim) = counting(Some(RECLAIMED));
        let gate =
            Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), json).reclaiming(reclaim);
        let (_slot, err, events) = take_gate(&gate);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(err, format!("dx: {RECLAIMED}\n"));
        if !json {
            assert!(events.is_empty(), "text mode writes no events");
            continue;
        }
        let notices: Vec<serde_json::Value> = events
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(notices.len(), 1, "{events}");
        assert_eq!(notices[0]["event"], "notice");
        assert_eq!(notices[0]["severity"], "warning");
        assert_eq!(notices[0]["code"], "disk_low");
        assert_eq!(notices[0]["message"], RECLAIMED);
        assert_eq!(notices[0]["related_command"], "test");
    }
}

#[test]
fn hold_for_a_command_without_a_build_slot_never_reclaims() {
    let scratch = dx_test_scratch::scratch("dx-slots-no-reclaim-");
    let (calls, reclaim) = counting(Some(RECLAIMED));
    let gate =
        Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), true).reclaiming(reclaim);
    for command in [
        Command::Worktree,
        Command::Status,
        Command::Clean,
        Command::Watch,
    ] {
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = gate.hold(command, &mut err, &mut events).expect("hold");
        assert!(slot.is_none(), "{command:?}");
        assert!(err.is_empty() && events.is_empty(), "{command:?}");
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    let mut err = Vec::new();
    let slot = gate
        .hold(Command::Build, &mut err, &mut Vec::new())
        .expect("hold");
    assert!(slot.is_some());
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "a build reclaims"
    );
}

#[test]
fn a_quiet_reclaim_prints_nothing() {
    let scratch = dx_test_scratch::scratch("dx-slots-quiet-reclaim-");
    let (calls, reclaim) = counting(None);
    let gate =
        Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), true).reclaiming(reclaim);
    let (_slot, err, events) = take_gate(&gate);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert!(err.is_empty(), "{err}");
    assert!(events.is_empty(), "{events}");
}

#[test]
fn take_reclaims_when_slots_are_disabled() {
    let scratch = dx_test_scratch::scratch("dx-slots-disabled-reclaim-");
    let blocker = scratch.path().join("slots");
    std::fs::write(&blocker, b"").expect("blocker");
    let (calls, reclaim) = counting(Some(RECLAIMED));
    let gate = Gate::new(blocker, 1, "me".to_owned(), false).reclaiming(reclaim);
    let mut err = Vec::new();
    let slot = gate
        .take(Command::Test, &mut err, &mut Vec::new())
        .expect("take");
    assert!(slot.is_none());
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    let text = String::from_utf8(err).expect("utf8");
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(lines[0].starts_with("dx: build slots disabled: "), "{text}");
    assert_eq!(lines[1], format!("dx: {RECLAIMED}"));
}

#[test]
fn a_nested_take_without_a_gate_never_reclaims() {
    let scratch = dx_test_scratch::scratch("dx-slots-nested-reclaim-");
    let (calls, reclaim) = counting(Some(RECLAIMED));
    let gate =
        Gate::new(scratch.path().join("slots"), 1, "me".to_owned(), false).reclaiming(reclaim);
    with_gate(Some(gate), || {
        with_gate(None, || {
            take(Command::Build, &mut Vec::new(), &mut Vec::new()).expect("take")
        })
    });
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[test]
fn take_without_a_gate_takes_nothing() {
    let scratch = dx_test_scratch::scratch("dx-slots-no-gate-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 1, "me".to_owned(), false);
    with_gate(Some(gate), || {
        let slot = take(Command::Run, &mut Vec::new(), &mut Vec::new()).expect("take");
        assert!(slot.is_some());
        assert!(
            with_gate(None, || take(
                Command::Run,
                &mut Vec::new(),
                &mut Vec::new()
            ))
            .expect("none")
            .is_none()
        );
    });
    assert!(
        take(Command::Run, &mut Vec::new(), &mut Vec::new())
            .expect("none")
            .is_none()
    );
    assert!(slot_settles_free(&dir, 1));
}

#[test]
fn a_phase_takes_nothing_until_needed() {
    let scratch = dx_test_scratch::scratch("dx-slots-phase-lazy-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 1, "me".to_owned(), false);
    with_gate(Some(gate), || {
        let mut phase = Phase::new(Command::Run);
        let (mut err, mut events) = (Vec::new(), Vec::new());
        assert!(phase.need(false, &mut err, &mut events).unwrap().is_none());
        assert!(phase.slot().is_none());
        assert!(slot_settles_free(&dir, 1));
    });
}

#[test]
fn a_phase_takes_once() {
    let scratch = dx_test_scratch::scratch("dx-slots-phase-once-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir, 1, "me".to_owned(), false);
    with_gate(Some(gate), || {
        let mut phase = Phase::new(Command::Run);
        let (mut err, mut events) = (Vec::new(), Vec::new());
        assert!(phase.need(true, &mut err, &mut events).unwrap().is_some());
        assert!(phase.need(true, &mut err, &mut events).unwrap().is_some());
        assert!(err.is_empty());
    });
}

#[test]
fn a_phase_without_a_gate_holds_nothing() {
    let mut phase = Phase::new(Command::Run);
    let (mut err, mut events) = (Vec::new(), Vec::new());
    assert!(phase.need(true, &mut err, &mut events).unwrap().is_none());
    assert!(phase.need(true, &mut err, &mut events).unwrap().is_none());
    assert!(phase.slot().is_none());
}

#[test]
fn a_released_phase_frees_its_slot() {
    let scratch = dx_test_scratch::scratch("dx-slots-phase-release-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 1, "me".to_owned(), false);
    with_gate(Some(gate), || {
        let mut phase = Phase::new(Command::Run);
        let (mut err, mut events) = (Vec::new(), Vec::new());
        assert!(phase.need(true, &mut err, &mut events).unwrap().is_some());
        phase.release();
        assert!(phase.slot().is_none());
        assert!(slot_settles_free(&dir, 1));
    });
}

#[test]
fn share_report_counts_held_slots_read_only() {
    let scratch = dx_test_scratch::scratch("dx-slots-report-");
    let cache = scratch.path().display().to_string();
    let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    let dir = slots_dir(&get).expect("dir");
    std::fs::create_dir_all(&dir).expect("dir");
    let report = |get: &dyn Fn(&str) -> Option<String>| {
        share_report(
            scratch.path(),
            get,
            BuildSlots::Count(2.try_into().expect("two")),
        )
    };
    let idle = report(&get).expect("report");
    assert_eq!((idle.held, idle.count), (0, 2));
    let _held = try_take(&dir, 2, "me").expect("try").expect("slot");
    std::fs::write(holder_path(&dir, 0), "me").expect("holder");
    let busy = report(&get).expect("report");
    assert_eq!(busy.held, 1);
    assert!(busy.next.cpus <= idle.next.cpus);
    assert!(!slot_path(&dir, 1).exists(), "the probe creates no slot");
}

#[test]
fn held_count_includes_extra_slots() {
    let scratch = dx_test_scratch::scratch("dx-slots-extra-");
    let size = Sizing {
        count: 2,
        pool: 3,
        extras: 1,
    };
    assert_eq!(size.counted().collect::<Vec<_>>(), [0, 1, 3]);
    let _extra = lock_slot(&slot_path(scratch.path(), 3), false).expect("lock");
    assert_eq!(held(scratch.path(), size.counted(), usize::MAX), 2);
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
    let local = |name: &str| (name == "LOCALAPPDATA").then(|| "/l".to_owned());
    assert_eq!(slots_dir(&local), Some(PathBuf::from("/l/dx/slots")));
    let empty_home = |name: &str| match name {
        "HOME" => Some(String::new()),
        "LOCALAPPDATA" => Some("/l".to_owned()),
        _ => None,
    };
    assert_eq!(slots_dir(&empty_home), Some(PathBuf::from("/l/dx/slots")));
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
    let (sender, receiver) = std::sync::mpsc::channel();
    let waiter = std::thread::spawn(move || {
        let mut holders = Vec::new();
        let slot = acquire(&second_dir, 1, "tree-b", &mut |holder| {
            holders.push(holder.to_owned());
            let _ = sender.send(());
        })
        .expect("second");
        (slot.index, holders)
    });
    announced(&receiver);
    drop(first);
    let (index, holders) = waiter.join().expect("join");
    seen.extend(holders);
    assert_eq!(index, 0);
    assert_eq!(seen, vec!["tree-a".to_owned()]);
    assert_eq!(
        wait_message("tree-a"),
        "waiting for a build slot (held by tree-a); see dx status, or set --build-slots or DX_BUILD_SLOTS"
    );
}

#[test]
fn waiters_get_slots_in_arrival_order() {
    let scratch = dx_test_scratch::scratch("dx-slots-order-");
    let dir = scratch.path().join("slots");
    let held = try_take(&dir, 1, "first").expect("try").expect("free");
    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut notices = Vec::new();
    let mut waiters = Vec::new();
    for id in 0..3 {
        let (sender, receiver) = std::sync::mpsc::channel();
        let dir = dir.clone();
        let order = order.clone();
        waiters.push(std::thread::spawn(move || {
            let slot = acquire(&dir, 1, &format!("w{id}"), &mut |holder| {
                sender.send(holder.to_owned()).expect("send");
            })
            .expect("slot");
            order.lock().expect("order").push(id);
            std::thread::sleep(Duration::from_millis(50));
            drop(slot);
        }));
        notices.push(
            receiver
                .recv_timeout(Duration::from_secs(30))
                .expect("notice"),
        );
    }
    drop(held);
    for waiter in waiters {
        waiter.join().expect("join");
    }
    assert_eq!(*order.lock().expect("order"), vec![0, 1, 2]);
    assert_eq!(
        notices,
        vec![
            "first".to_owned(),
            "first, 1 ahead in line".to_owned(),
            "first, 2 ahead in line".to_owned(),
        ]
    );
    assert!(tickets(&dir).is_empty());
}

#[test]
fn a_newcomer_queues_behind_a_waiter() {
    let scratch = dx_test_scratch::scratch("dx-slots-newcomer-");
    let dir = scratch.path().join("slots");
    let queued = enqueue(&dir, "").expect("queued");
    let (sender, receiver) = std::sync::mpsc::channel();
    let newcomer_dir = dir.clone();
    let newcomer = std::thread::spawn(move || {
        acquire(&newcomer_dir, 1, "newcomer", &mut |holder| {
            sender.send(holder.to_owned()).expect("send");
        })
        .expect("slot")
        .index
    });
    let notice = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("notice");
    assert_eq!(notice, "another build, 1 ahead in line");
    let first = try_take(&dir, 1, "queued").expect("try").expect("free");
    drop(queued);
    drop(first);
    assert_eq!(newcomer.join().expect("join"), 0);
}

#[test]
fn an_unlocked_ticket_does_not_block_and_is_swept() {
    let scratch = dx_test_scratch::scratch("dx-slots-swept-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(queue_dir(&dir)).expect("queue");
    let dead = queue_dir(&dir).join(format!("ticket-{:020}.lock", 7));
    std::fs::write(&dead, b"").expect("dead ticket");
    std::fs::write(queue_dir(&dir).join("ticket-x.lock"), b"").expect("odd name");
    let slot = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("slot");
    assert_eq!(slot.index, 0);
    assert!(!dead.exists());
    assert!(tickets(&dir).is_empty());
}

#[test]
fn a_leaving_ticket_never_deletes_the_next_waiters_want() {
    let scratch = dx_test_scratch::scratch("dx-slots-leave-");
    let dir = scratch.path().join("slots");
    let mut first = enqueue(&dir, "").expect("first");
    let first_number = first.number;
    let mut next = None;
    first.leave(|| next = Some(enqueue(&dir, "").expect("next")));
    let next = next.expect("next ticket");
    assert_ne!(next.number, first_number);
    drop(first);
    assert!(next.path.exists());
    assert!(want_path(&next.path).exists());
    let listed = tickets(&dir);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].1, next.path);
}

#[test]
fn a_dropped_ticket_leaves_no_file() {
    let scratch = dx_test_scratch::scratch("dx-slots-drop-");
    let dir = scratch.path().join("slots");
    drop(enqueue(&dir, "").expect("ticket"));
    assert!(tickets(&dir).is_empty());
}

#[test]
fn a_stale_ticket_does_not_block() {
    let scratch = dx_test_scratch::scratch("dx-slots-stale-");
    let dir = scratch.path().join("slots");
    let stale = enqueue(&dir, "").expect("stale");
    let earlier = SystemTime::now() - Duration::from_secs(60);
    stale
        .file
        .as_ref()
        .expect("file")
        .set_modified(earlier)
        .expect("mtime");
    let slot = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("slot");
    assert_eq!(slot.index, 0);
    assert!(stale.path.exists());
}

#[test]
fn a_broken_queue_still_gates() {
    let scratch = dx_test_scratch::scratch("dx-slots-no-queue-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("slots");
    std::fs::write(queue_dir(&dir), b"").expect("blocker");
    assert!(enqueue(&dir, "").is_err());
    let held = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("held");
    let (sender, receiver) = std::sync::mpsc::channel();
    let waiter_dir = dir.clone();
    let waiter = std::thread::spawn(move || {
        acquire(&waiter_dir, 1, "b", &mut |holder| {
            sender.send(holder.to_owned()).expect("send");
        })
        .expect("slot")
        .index
    });
    let notice = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("notice");
    assert_eq!(notice, "a");
    drop(held);
    assert_eq!(waiter.join().expect("join"), 0);
}

#[test]
fn a_freed_slot_clears_its_holder() {
    let scratch = dx_test_scratch::scratch("dx-slots-holder-");
    let dir = scratch.path().join("slots");
    let a = try_take(&dir, 3, "a").expect("try").expect("a");
    let b = try_take(&dir, 3, "b").expect("try").expect("b");
    let c = try_take(&dir, 3, "c").expect("try").expect("c");
    assert_eq!(holder(&dir, 0..3, 2), "a and 2 more, 2 ahead in line");
    drop(a);
    assert!(!holder_path(&dir, 0).exists());
    assert_eq!(holder(&dir, 0..3, 0), "b and 1 more");
    drop((b, c));
    assert_eq!(holder(&dir, 0..3, 0), "another build");
}

#[test]
fn a_free_slot_is_taken_without_waiting() {
    let scratch = dx_test_scratch::scratch("dx-slots-free-");
    let dir = scratch.path().join("slots");
    let first = acquire(&dir, 2, "a", &mut |_| {}).expect("first");
    let second = acquire(&dir, 2, "b", &mut |_| panic!("must not wait")).expect("second");
    assert_eq!((first.index, second.index), (0, 1));
}

/// A writer that buffers everything and signals once on its first write.
///
/// `Gate::take` writes the wait line from `on_wait`, which `acquire_reserving` runs only after
/// the waiter is queued and failed to take a slot, so the signal proves the waiter is in line.
struct Announce {
    buf: Vec<u8>,
    signal: Option<std::sync::mpsc::Sender<()>>,
}

impl Announce {
    fn new() -> (Self, std::sync::mpsc::Receiver<()>) {
        let (signal, rx) = std::sync::mpsc::channel();
        let announce = Self {
            buf: Vec::new(),
            signal: Some(signal),
        };
        (announce, rx)
    }

    fn text(self) -> String {
        String::from_utf8(self.buf).expect("utf8")
    }
}

impl Write for Announce {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(data);
        if let Some(signal) = self.signal.take() {
            let _ = signal.send(());
        }
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

const ANNOUNCE_TIMEOUT: Duration = Duration::from_secs(30);

/// Waits for the waiter's announcement; panics rather than hanging when it never comes.
fn announced(rx: &std::sync::mpsc::Receiver<()>) {
    rx.recv_timeout(ANNOUNCE_TIMEOUT)
        .expect("waiter never announced a wait");
}

/// Waits for the announcement, then always drops `held`, so a missing wait fails the caller's
/// assertions instead of hanging `hold()` forever. Returns whether the wait was announced.
fn released_after_wait<T>(rx: &std::sync::mpsc::Receiver<()>, held: T) -> bool {
    let seen = rx.recv_timeout(ANNOUNCE_TIMEOUT).is_ok();
    drop(held);
    seen
}

/// Like `take_gate` on a thread; the receiver fires once the thread is waiting in line.
fn spawn_take(
    gate: Gate,
) -> (
    std::thread::JoinHandle<(Slot, String, String)>,
    std::sync::mpsc::Receiver<()>,
) {
    let (mut err, rx) = Announce::new();
    let waiter = std::thread::spawn(move || {
        let mut events = Vec::new();
        let slot = gate
            .take(Command::Test, &mut err, &mut events)
            .expect("take")
            .expect("slot");
        (slot, err.text(), String::from_utf8(events).expect("utf8"))
    });
    (waiter, rx)
}

fn take_gate(gate: &Gate) -> (Slot, String, String) {
    let mut err = Vec::new();
    let mut events = Vec::new();
    let slot = gate
        .take(Command::Test, &mut err, &mut events)
        .expect("take")
        .expect("slot");
    (
        slot,
        String::from_utf8(err).expect("utf8"),
        String::from_utf8(events).expect("utf8"),
    )
}

#[test]
fn a_rules_dx_run_keeps_consumers_off_the_reserve() {
    let scratch = dx_test_scratch::scratch("dx-slots-reserve-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 1, "r".to_owned(), false).reserving(2);
    let (slot, err, _) = take_gate(&gate);
    assert_eq!(slot.index, 0);
    assert_eq!(err, "");
    assert!(try_take(&dir, 2, "c").expect("try").is_none());
    drop(slot);
    assert!(slot_settles_free(&dir, 2));
}

fn count(n: usize) -> BuildSlots {
    BuildSlots::Count(std::num::NonZeroUsize::new(n).expect("n"))
}

#[test]
fn auto_sizing_matches_memory_sizing() {
    for (memory, reserved) in [
        (64u64 << 30, 0u64),
        (64 << 30, 6 << 30),
        (2 << 30, 0),
        (19 << 30, 6 << 30),
    ] {
        let count = slot_count(memory, reserved);
        assert_eq!(
            sizing(BuildSlots::Auto, memory, reserved),
            Some(Sizing {
                count,
                pool: slot_count(memory, 0),
                extras: remote_slot_count(memory, reserved).saturating_sub(count),
            }),
            "{memory} {reserved}"
        );
    }
}

#[test]
fn count_sizing_keeps_the_reserve_width_above_n() {
    let memory = 30u64 << 30;
    assert_eq!(
        sizing(count(2), memory, 0),
        Some(Sizing {
            count: 2,
            pool: 2,
            extras: 0
        })
    );
    let width = slot_count(memory, 0) - slot_count(memory, 6 << 30);
    assert_eq!(width, 2);
    assert_eq!(
        sizing(count(2), memory, 6 << 30),
        Some(Sizing {
            count: 2,
            pool: 2 + width,
            extras: 0
        })
    );
    assert_eq!(
        sizing(count(40), memory, 0),
        Some(Sizing {
            count: 40,
            pool: 40,
            extras: 0
        }),
        "an explicit count is not clamped to {MAX_SLOTS}"
    );
    assert_eq!(sizing(BuildSlots::Off, memory, 0), None);
}

#[test]
fn the_default_setting_is_auto_only_inside_a_worktree_slot() {
    let plain = dx_test_scratch::scratch("dx-slots-setting-");
    assert_eq!(effective_setting(None, plain.path()), BuildSlots::Off);
    assert_eq!(effective_setting(Some(count(2)), plain.path()), count(2));
    let pool = dx_test_scratch::scratch("dx-slots-setting-pool-");
    let slot = pool.path().join("pool/1");
    std::fs::create_dir_all(&slot).expect("slot");
    std::fs::create_dir_all(pool.path().join("pool/.dx")).expect("meta");
    let record = dx_worktree::Record {
        slot: 1,
        held: false,
        branch: None,
        last_branch: None,
        session: None,
        agent: None,
        pid: None,
        pid_start: None,
        boot_id: None,
        gone: false,
        stopped_at: None,
        unclaimed: false,
        broken: false,
        leased_at: 0,
        last_used: 0,
        lock_digest: None,
    };
    std::fs::write(
        pool.path().join("pool/.dx/1.json"),
        serde_json::to_vec(&record).expect("json"),
    )
    .expect("record");
    assert_eq!(effective_setting(None, &slot), BuildSlots::Auto);
    assert_eq!(
        effective_setting(Some(BuildSlots::Off), &slot),
        BuildSlots::Off
    );
}

#[test]
fn off_and_nested_runs_build_no_gate() {
    let scratch = dx_test_scratch::scratch("dx-slots-off-");
    let cache = scratch.path().to_string_lossy().into_owned();
    let get = |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    assert!(Gate::from_env(Path::new("/ws"), &get, false, &[], BuildSlots::Off).is_none());
    let gate = Gate::from_env(Path::new("/ws"), &get, false, &[], count(3)).expect("gate");
    assert_eq!((gate.count, gate.pool, gate.extras), (3, 3, 0));
    let nested = |name: &str| match name {
        "XDG_CACHE_HOME" => Some(cache.clone()),
        SLOT_ENV_VAR => Some("1".to_owned()),
        _ => None,
    };
    for setting in [BuildSlots::Auto, BuildSlots::Off, count(1), count(40)] {
        assert!(
            Gate::from_env(Path::new("/ws"), &nested, false, &[], setting).is_none(),
            "a child under a held slot ignores {setting}"
        );
    }
}

#[test]
fn a_count_gate_waits_on_the_take_past_n() {
    let scratch = dx_test_scratch::scratch("dx-slots-count-wait-");
    let dir = scratch.path().join("slots");
    let size = sizing(count(2), 64 << 30, 0).expect("sizing");
    let gate = Gate::new(dir.clone(), size.count, "fill".to_owned(), false)
        .reserving(size.pool)
        .extra(size.extras, false);
    let (first, _, _) = take_gate(&gate);
    let (second, _, _) = take_gate(&gate);
    assert_eq!((first.index, second.index), (0, 1));
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    drop(first);
    let (third, err, _) = waiter.join().expect("join");
    assert_eq!(third.index, 0);
    assert!(
        err.starts_with("dx: waiting for a build slot (held by fill"),
        "{err}"
    );
    drop(second);
}

#[test]
fn rules_dx_runs_share_the_reserve() {
    let scratch = dx_test_scratch::scratch("dx-slots-share-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 2, "r".to_owned(), false).reserving(3);
    let (first, first_err, _) = take_gate(&gate);
    let (second, second_err, _) = take_gate(&gate);
    assert_eq!((first.index, second.index), (0, 1));
    assert_eq!((first_err.as_str(), second_err.as_str()), ("", ""));
    assert!(try_take(&dir, 3, "c").expect("try").is_none());
}

#[test]
fn a_rules_dx_run_waits_for_a_consumer_on_the_reserve() {
    let scratch = dx_test_scratch::scratch("dx-slots-reserve-wait-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
    let consumer = try_take(&dir, 2, "consumer")
        .expect("try")
        .expect("consumer");
    assert_eq!(consumer.index, 1);
    drop(fill);
    let gate = Gate::new(dir.clone(), 1, "r".to_owned(), true).reserving(2);
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    drop(consumer);
    let (slot, err, events) = waiter.join().expect("join");
    assert_eq!(slot.index, 0);
    assert_eq!(err, format!("dx: {}\n", wait_message("consumer")));
    assert_eq!(
        events.matches("waiting_for_build_slot").count(),
        1,
        "{events}"
    );
}

#[test]
fn a_want_round_trips_and_garbage_wants_everything() {
    let want = Want {
        base: 0..3,
        reserve: 3..5,
        extras: 5..7,
        remote: true,
    };
    assert_eq!(Want::parse(&want.render()), Some(want));
    for garbage in [
        "",
        "base=0..1",
        "base=x",
        "base=0..1 reserve=1..1 extras=1..1 remote=0 bogus=1",
    ] {
        assert_eq!(Want::parse(garbage), None, "{garbage}");
    }
    let scratch = dx_test_scratch::scratch("dx-slots-want-garbage-");
    assert_eq!(
        Want::read(&scratch.path().join("missing.want")),
        Want::everything()
    );
}

#[test]
fn a_dropped_ticket_removes_its_want_and_the_sweep_removes_orphans() {
    let scratch = dx_test_scratch::scratch("dx-slots-want-drop-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(queue_dir(&dir)).expect("queue");
    let want = Want::of(
        &Layout {
            count: 1,
            reserve: 1..2,
            extras: 2..2,
        },
        false,
    );
    let ticket = enqueue_wanting(&dir, "", &want).expect("ticket");
    let path = want_path(&ticket.path);
    assert_eq!(Want::read(&path), want);
    drop(ticket);
    assert!(!path.exists());
    let orphan = queue_dir(&dir).join("ticket-00000000000000000099.want");
    std::fs::write(&orphan, "x").expect("orphan");
    drop(enqueue(&dir, "").expect("ticket"));
    assert!(!orphan.exists());
}

#[test]
fn a_waiters_label_is_kept_from_its_sidecar_and_swept_with_the_ticket() {
    let scratch = dx_test_scratch::scratch("dx-slots-label-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(queue_dir(&dir)).expect("queue");
    let ticket = enqueue(&dir, "/ws/waiter").expect("ticket");
    let label = label_path(&ticket.path);
    std::fs::write(&label, "/ws/sidecar").expect("sidecar");
    let kept = kept_workspaces(&dir, 0..0, "/ws/own");
    assert!(kept.contains(&PathBuf::from("/ws/sidecar")));
    assert!(label.exists());
    drop(ticket);
    assert!(!label.exists());
    let orphan = queue_dir(&dir).join("ticket-00000000000000000099.label");
    std::fs::write(&orphan, "x").expect("orphan");
    drop(enqueue(&dir, "").expect("ticket"));
    assert!(!orphan.exists());
}

#[test]
fn a_future_mtime_is_not_recent_and_not_a_live_waiter() {
    let scratch = dx_test_scratch::scratch("dx-slots-future-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    let future = SystemTime::now() + Duration::from_secs(3600);
    let set = |path: &Path| {
        let file = File::options().write(true).open(path).expect("open");
        file.set_modified(future).expect("set mtime");
    };
    std::fs::write(extra_mark(&dir), "").expect("mark");
    assert!(extra_recent(&dir));
    set(&extra_mark(&dir));
    assert!(!extra_recent(&dir));
    let ticket = dir.join("ticket");
    std::fs::write(&ticket, "").expect("ticket");
    assert!(!waiting(&ticket));
    set(&ticket);
    assert!(!waiting(&ticket));
}

#[test]
fn an_unwritable_queue_dir_warns_and_the_run_still_gets_a_slot() {
    let scratch = dx_test_scratch::scratch("dx-slots-queue-warn-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(queue_dir(&dir), "not a directory").expect("block queue");
    let layout = Layout {
        count: 1,
        reserve: 1..1,
        extras: 1..1,
    };
    let mut skipped = Vec::new();
    let slot = acquire_reserving(
        &dir,
        &layout,
        Admission::none(),
        "r",
        &mut |_, _| {},
        &mut || {},
        &mut |path, _| skipped.push(path.to_path_buf()),
    )
    .expect("slot");
    assert_eq!(slot.index, 0);
    assert_eq!(skipped, vec![queue_dir(&dir)]);
}

#[test]
fn an_unwritable_queue_warns_once_through_the_gate() {
    let scratch = dx_test_scratch::scratch("dx-slots-queue-gate-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(queue_dir(&dir), "").expect("blocker");
    let gate = Gate::new(dir.clone(), 2, "a".to_owned(), false);
    let mut err = Vec::new();
    let mut events = Vec::new();
    let slot = gate.take(Command::Test, &mut err, &mut events);
    assert!(matches!(slot, Ok(Some(_))));
    let text = String::from_utf8(err).expect("utf8");
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(
        text.contains(&queue_dir(&dir).display().to_string()),
        "{text}"
    );
}

#[test]
fn a_consumer_passes_a_rules_dx_head_blocked_on_its_reserve() {
    let scratch = dx_test_scratch::scratch("dx-slots-pass-head-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
    let consumer = try_take(&dir, 2, "consumer")
        .expect("try")
        .expect("consumer");
    assert_eq!(consumer.index, 1);
    drop(fill);
    let (sender, receiver) = std::sync::mpsc::channel();
    let head_dir = dir.clone();
    let head = std::thread::spawn(move || {
        let layout = Layout {
            count: 1,
            reserve: 1..2,
            extras: 2..2,
        };
        acquire_reserving(
            &head_dir,
            &layout,
            Admission::none(),
            "r",
            &mut |holder, _| {
                sender.send(holder.to_owned()).expect("send");
            },
            &mut || {},
            &mut |_, _| {},
        )
        .map(|slot| slot.index)
    });
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("head waits");
    let behind = acquire(&dir, 2, "behind", &mut |_| {}).expect("behind passes");
    assert_eq!(behind.index, 0);
    drop(behind);
    drop(consumer);
    assert_eq!(head.join().expect("join").expect("head"), 0);
}

#[test]
fn a_queued_consumer_does_not_take_the_reserve_behind_a_rules_dx_run() {
    let scratch = dx_test_scratch::scratch("dx-slots-reserve-line-");
    let dir = scratch.path().join("slots");
    let fill = try_take(&dir, 2, "fill").expect("try").expect("fill");
    let consumer = try_take(&dir, 2, "consumer")
        .expect("try")
        .expect("consumer");
    drop(fill);
    let (sender, receiver) = std::sync::mpsc::channel();
    let rules_dx_dir = dir.clone();
    let rules_dx = std::thread::spawn(move || {
        let layout = Layout {
            count: 1,
            reserve: 1..2,
            extras: 2..2,
        };
        acquire_reserving(
            &rules_dx_dir,
            &layout,
            Admission::none(),
            "r",
            &mut |holder, _| {
                sender.send(holder.to_owned()).expect("send");
            },
            &mut || {},
            &mut |_, _| {},
        )
        .expect("slot")
    });
    let notice = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("notice");
    assert_eq!(notice, "consumer");
    drop(consumer);
    let held = rules_dx.join().expect("join");
    assert_eq!(held.index, 0);
    let (sender, receiver) = std::sync::mpsc::channel();
    let late_dir = dir.clone();
    let late = std::thread::spawn(move || {
        acquire(&late_dir, 2, "late", &mut |holder| {
            sender.send(holder.to_owned()).expect("send");
        })
        .expect("slot")
        .index
    });
    let notice = receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("notice");
    assert_eq!(notice, "r");
    std::thread::sleep(Duration::from_millis(600));
    assert!(
        !late.is_finished(),
        "a consumer must not take a reserve slot"
    );
    drop(held);
    assert!(late.join().expect("join") < 2);
}

#[test]
fn an_unopenable_slot_file_is_skipped() {
    let scratch = dx_test_scratch::scratch("dx-slots-skip-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
    let slot = acquire(&dir, 2, "a", &mut |_| panic!("must not wait")).expect("slot");
    assert_eq!(slot.index, 1);
}

#[cfg(unix)]
#[test]
fn read_only_slot_file_still_counts() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = dx_test_scratch::scratch("dx-slots-readonly-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = slot_path(&dir, 0);
    std::fs::write(&path, b"").expect("slot file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
    let slot = acquire(&dir, 1, "a", &mut |_| panic!("must not wait")).expect("slot");
    assert_eq!(slot.index, 0);
    assert!(try_take(&dir, 1, "b").expect("try").is_none());
    drop(slot);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
}

#[cfg(unix)]
#[test]
fn a_read_only_reserve_slot_still_reserves() {
    use std::os::unix::fs::PermissionsExt;
    let scratch = dx_test_scratch::scratch("dx-slots-readonly-reserve-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    let path = slot_path(&dir, 1);
    std::fs::write(&path, b"").expect("slot file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
    let gate = Gate::new(dir.clone(), 1, "r".to_owned(), false).reserving(2);
    let (slot, err, _) = take_gate(&gate);
    assert_eq!((slot.index, err.as_str()), (0, ""));
    assert!(try_take(&dir, 2, "c").expect("try").is_none());
    drop(slot);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
}

#[test]
fn a_skipped_slot_warns_once() {
    for json in [true, false] {
        let scratch = dx_test_scratch::scratch("dx-slots-warn-");
        let dir = scratch.path().join("slots");
        std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
        let gate = Gate::new(dir, 2, "a".to_owned(), json);
        let mut err = Vec::new();
        let mut events = Vec::new();
        for _ in 0..2 {
            let slot = gate.clone().take(Command::Test, &mut err, &mut events);
            assert!(matches!(slot, Ok(Some(ref slot)) if slot.index == 1));
        }
        let text = String::from_utf8(err).expect("utf8");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1, "{text}");
        assert!(lines[0].starts_with("dx: skipped build slot "), "{text}");
        assert!(lines[0].contains("slot-0.lock"), "{text}");
        let events = String::from_utf8(events).expect("utf8");
        if json {
            assert_eq!(events.lines().count(), 1, "{events}");
            assert!(events.contains("\"event\":\"notice\""));
            assert!(events.contains("build_slot_skipped"));
            assert!(events.contains("\"severity\":\"warning\""));
        } else {
            assert!(events.is_empty(), "{events}");
        }
    }
}

#[test]
fn no_openable_slot_file_is_an_error() {
    let scratch = dx_test_scratch::scratch("dx-slots-none-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
    assert!(try_take(&dir, 1, "a").is_err());
}

#[test]
fn a_bad_slot_and_busy_slots_still_wait() {
    let scratch = dx_test_scratch::scratch("dx-slots-partial-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(slot_path(&dir, 0)).expect("blocker");
    let _held = acquire(&dir, 2, "a", &mut |_| {}).expect("held");
    assert!(try_take(&dir, 2, "b").expect("try").is_none());
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
    let count = slot_count(available_memory().expect("memory"), 0);
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

fn busy(count: usize) -> String {
    let more = (1..count)
        .last()
        .map_or(String::new(), |n| format!(" and {n} more"));
    format!("tree-0{more}")
}

#[test]
fn waiting_prints_one_line_and_one_json_notice() {
    let scratch = dx_test_scratch::scratch("dx-slots-notice-");
    let cache = scratch.path().to_string_lossy().into_owned();
    let dir = scratch.path().join("dx").join("slots");
    let count = slot_count(available_memory().expect("memory"), 0);
    let held: Vec<Slot> = (0..count)
        .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
        .collect();
    let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    let (mut err, rx) = Announce::new();
    let releaser = std::thread::spawn(move || released_after_wait(&rx, held));
    let mut events = Vec::new();
    let slot = hold(
        Command::Test,
        Path::new("/ws"),
        &get,
        &mut err,
        true,
        &mut events,
    );
    assert!(
        releaser.join().expect("join"),
        "waiter never announced a wait"
    );
    assert!(matches!(slot, Ok(Some(_))));
    let text = err.text();
    assert_eq!(text.lines().count(), 1);
    assert!(text.contains(&wait_message(&busy(count))), "{text}");
    let json = String::from_utf8(events).expect("utf8");
    assert_eq!(json.lines().count(), 1);
    assert!(json.contains("\"event\":\"notice\""));
    assert!(json.contains("waiting_for_build_slot"));
}

#[test]
fn unopenable_slots_print_one_disabled_line_and_one_json_notice() {
    let scratch = dx_test_scratch::scratch("dx-slots-disabled-");
    let cache = scratch.path().to_string_lossy().into_owned();
    std::fs::create_dir_all(scratch.path().join("dx")).expect("dx dir");
    std::fs::write(scratch.path().join("dx").join("slots"), b"").expect("blocker");
    let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    for json in [true, false] {
        let mut err = Vec::new();
        let mut events = Vec::new();
        let slot = hold(
            Command::Test,
            Path::new("/ws"),
            &get,
            &mut err,
            json,
            &mut events,
        );
        assert!(matches!(slot, Ok(None)));
        let text = String::from_utf8(err).expect("utf8");
        assert_eq!(text.lines().count(), 1);
        assert!(text.starts_with("dx: build slots disabled: "), "{text}");
        let lines = String::from_utf8(events).expect("utf8");
        if json {
            assert_eq!(lines.lines().count(), 1);
            assert!(lines.contains("\"event\":\"notice\""));
            assert!(lines.contains("build_slots_disabled"));
            assert!(lines.contains("\"severity\":\"warning\""));
        } else {
            assert!(lines.is_empty());
        }
    }
}

#[test]
fn update_holds_no_process_slot() {
    let scratch = dx_test_scratch::scratch("dx-slots-update-");
    let cache = scratch.path().to_string_lossy().into_owned();
    let dir = scratch.path().join("dx").join("slots");
    let count = slot_count(available_memory().expect("memory"), 0);
    let held: Vec<Slot> = (0..count)
        .map(|index| acquire(&dir, count, &format!("tree-{index}"), &mut |_| {}).expect("fill"))
        .collect();
    let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    let mut err = Vec::new();
    let mut events = Vec::new();
    let slot = hold(
        Command::Update,
        Path::new("/ws"),
        &get,
        &mut err,
        true,
        &mut events,
    );
    assert!(matches!(slot, Ok(None)));
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    assert!(events.is_empty(), "{}", String::from_utf8_lossy(&events));
    drop(held);
}

#[test]
fn a_waiter_polls_while_it_waits_and_stops_once_it_takes_the_slot() {
    let scratch = dx_test_scratch::scratch("dx-slots-poll-");
    let dir = scratch.path().join("slots");
    let first = acquire(&dir, 1, "tree-a", &mut |_| {}).expect("first");
    let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = polls.clone();
    let waiter_dir = dir.clone();
    let waiter = std::thread::spawn(move || {
        acquire_with(
            &waiter_dir,
            1,
            "tree-b",
            &mut |_, _| {},
            &mut || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            },
            &mut |_, _| {},
        )
        .expect("second")
    });
    let start = Instant::now();
    while polls.load(std::sync::atomic::Ordering::SeqCst) < 2 {
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "waiter never polled"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    drop(first);
    let slot = waiter.join().expect("join");
    let after = polls.load(std::sync::atomic::Ordering::SeqCst);
    assert!(after >= 2);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(polls.load(std::sync::atomic::Ordering::SeqCst), after);
    drop(slot);
}

type Reaps = Arc<Mutex<Vec<(Vec<PathBuf>, u64)>>>;

fn recording(gate: Gate, stopped: usize) -> (Gate, Reaps) {
    let calls: Reaps = Arc::default();
    let seen = Arc::clone(&calls);
    let gate = gate.reaping(move |keep, target| {
        seen.lock().expect("reaps").push((keep.to_vec(), target));
        stopped
    });
    (gate, calls)
}

/// Makes `gate` wait behind a holder, then frees the slot; returns the wait text and JSON.
fn wait_behind_holder(dir: &Path, gate: Gate) -> (String, String) {
    let holder = take_gate(&Gate::new(
        dir.to_path_buf(),
        1,
        "/w/holder".to_owned(),
        false,
    ))
    .0;
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    drop(holder);
    let (_slot, err, events) = waiter.join().expect("join");
    (err, events)
}

fn wait_notice(events: &str) -> serde_json::Value {
    let notices: Vec<serde_json::Value> = events
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    assert_eq!(notices.len(), 1, "{events}");
    assert_eq!(notices[0]["code"], "waiting_for_build_slot");
    notices[0].clone()
}

fn busy_gate(dir: &Path, wait: Duration) -> (Gate, Slot) {
    let holder = take_gate(&Gate::new(
        dir.to_path_buf(),
        1,
        "/w/holder".to_owned(),
        false,
    ))
    .0;
    let start = Instant::now();
    let ticks = std::sync::atomic::AtomicU64::new(0);
    let gate = Gate::new(dir.to_path_buf(), 1, "/w/me".to_owned(), true)
        .with_clock(move || {
            start + Duration::from_secs(ticks.fetch_add(1, std::sync::atomic::Ordering::SeqCst))
        })
        .waiting_at_most(Some(wait));
    (gate, holder)
}

#[test]
fn a_zero_wait_fails_at_once_naming_the_holder() {
    let scratch = dx_test_scratch::scratch("dx-slots-busy-");
    let dir = scratch.path().join("slots");
    let (gate, _holder) = busy_gate(&dir, Duration::ZERO);
    let (mut err, mut events) = (Vec::new(), Vec::new());
    let stop = gate
        .take(Command::Test, &mut err, &mut events)
        .err()
        .expect("busy");
    assert_eq!(stop, Stop::Busy("/w/holder".to_owned()));
    assert!(err.is_empty() && events.is_empty());
}

#[test]
fn a_wait_gives_up_once_the_clock_passes_the_limit() {
    let scratch = dx_test_scratch::scratch("dx-slots-deadline-");
    let dir = scratch.path().join("slots");
    let (gate, _holder) = busy_gate(&dir, Duration::from_secs(3));
    let (mut err, mut events) = (Vec::new(), Vec::new());
    let stop = gate
        .take(Command::Test, &mut err, &mut events)
        .err()
        .expect("busy");
    assert_eq!(stop, Stop::Busy("/w/holder".to_owned()));
    assert!(
        String::from_utf8(err)
            .expect("utf8")
            .contains("waiting for a build slot")
    );
}

#[test]
fn a_free_slot_is_taken_even_with_a_zero_wait() {
    let scratch = dx_test_scratch::scratch("dx-slots-free-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir, 1, "/w/me".to_owned(), false).waiting_at_most(Some(Duration::ZERO));
    assert!(take_gate(&gate).0.index == 0);
}

#[test]
fn a_wait_reaps_once_keeping_holders_waiters_and_itself() {
    let scratch = dx_test_scratch::scratch("dx-slots-reap-");
    let dir = scratch.path().join("slots");
    let holder = take_gate(&Gate::new(dir.clone(), 1, "/w/holder".to_owned(), false)).0;
    let queued = enqueue(&dir, "/w/waiter").expect("queued");
    let (gate, calls) = recording(Gate::new(dir.clone(), 1, "/w/me".to_owned(), true), 2);
    let (waiter, rx) = spawn_take(gate);
    announced(&rx);
    drop(queued);
    drop(holder);
    let (_slot, err, events) = waiter.join().expect("join");
    let calls = calls.lock().expect("reaps").clone();
    assert_eq!(calls.len(), 1);
    let (keep, target) = &calls[0];
    for workspace in ["/w/me", "/w/holder", "/w/waiter"] {
        assert!(keep.contains(&PathBuf::from(workspace)), "{keep:?}");
    }
    assert_eq!(*target, BYTES_PER_SLOT);
    assert_eq!(
        err.trim(),
        format!(
            "dx: {}; stopped 2 idle Bazel servers",
            wait_message("/w/holder, 1 ahead in line")
        )
    );
    let notice = wait_notice(&events);
    assert_eq!(notice["reaped_servers"], 2);
    assert!(
        notice["message"]
            .as_str()
            .is_some_and(|text| text.ends_with("; stopped 2 idle Bazel servers"))
    );
}

#[test]
fn a_wait_that_stops_nothing_keeps_the_plain_line_and_reports_zero() {
    let scratch = dx_test_scratch::scratch("dx-slots-reap-none-");
    let dir = scratch.path().join("slots");
    let (gate, calls) = recording(Gate::new(dir.clone(), 1, "/w/me".to_owned(), true), 0);
    let (err, events) = wait_behind_holder(&dir, gate);
    assert_eq!(calls.lock().expect("reaps").len(), 1);
    assert_eq!(err.trim(), format!("dx: {}", wait_message("/w/holder")));
    assert_eq!(wait_notice(&events)["reaped_servers"], 0);
}

#[test]
fn a_gate_without_a_reaper_reports_no_reaped_servers() {
    let scratch = dx_test_scratch::scratch("dx-slots-no-reaper-");
    let dir = scratch.path().join("slots");
    let gate = Gate::new(dir.clone(), 1, "/w/me".to_owned(), true);
    let (err, events) = wait_behind_holder(&dir, gate);
    assert_eq!(err.trim(), format!("dx: {}", wait_message("/w/holder")));
    assert!(wait_notice(&events).get("reaped_servers").is_none());
}

#[test]
fn another_reaping_waiter_skips_the_reap() {
    let scratch = dx_test_scratch::scratch("dx-slots-reap-busy-");
    let dir = scratch.path().join("slots");
    std::fs::create_dir_all(&dir).expect("dir");
    let busy = dx_atomic_fs::lease::try_lock_file(&dir.join("reap.lock"), true)
        .expect("lock")
        .expect("free");
    let (gate, calls) = recording(Gate::new(dir.clone(), 1, "/w/me".to_owned(), true), 3);
    let (err, events) = wait_behind_holder(&dir, gate);
    drop(busy);
    assert!(calls.lock().expect("reaps").is_empty());
    assert_eq!(err.trim(), format!("dx: {}", wait_message("/w/holder")));
    assert_eq!(wait_notice(&events)["reaped_servers"], 0);
}

#[test]
fn a_free_slot_never_reaps() {
    let scratch = dx_test_scratch::scratch("dx-slots-reap-free-");
    let (gate, calls) = recording(
        Gate::new(scratch.path().join("slots"), 1, "/w/me".to_owned(), true),
        1,
    );
    let (_slot, err, events) = take_gate(&gate);
    assert!(calls.lock().expect("reaps").is_empty());
    assert!(err.is_empty() && events.is_empty());
}

#[test]
fn a_remote_gate_reaps_one_remote_slot() {
    let scratch = dx_test_scratch::scratch("dx-slots-reap-remote-");
    let dir = scratch.path().join("slots");
    let (gate, calls) = recording(
        Gate::new(dir.clone(), 1, "/w/me".to_owned(), false).extra(0, true),
        0,
    );
    wait_behind_holder(&dir, gate);
    assert_eq!(calls.lock().expect("reaps")[0].1, BYTES_PER_REMOTE_SLOT);
}

#[test]
fn reaped_suffix_counts_servers() {
    assert_eq!(reaped_suffix(0), "");
    assert_eq!(reaped_suffix(1), "; stopped 1 idle Bazel server");
    assert_eq!(reaped_suffix(3), "; stopped 3 idle Bazel servers");
}

#[test]
fn a_free_slot_never_polls() {
    let scratch = dx_test_scratch::scratch("dx-slots-nopoll-");
    let dir = scratch.path().join("slots");
    let mut polls = 0;
    let slot = acquire_with(
        &dir,
        1,
        "tree",
        &mut |_, _| {},
        &mut || polls += 1,
        &mut |_, _| {},
    )
    .expect("slot");
    assert_eq!(slot.index, 0);
    assert_eq!(polls, 0);
}

#[test]
fn ping_interval_is_a_third_of_the_idle_limit_within_bounds() {
    assert_eq!(ping_interval(Some(90)), Some(Duration::from_secs(30)));
    assert_eq!(ping_interval(Some(300)), Some(Duration::from_secs(60)));
    assert_eq!(ping_interval(Some(900)), Some(Duration::from_secs(60)));
    assert_eq!(ping_interval(None), Some(Duration::from_secs(60)));
    assert_eq!(ping_interval(Some(2)), Some(Duration::from_secs(1)));
    assert_eq!(ping_interval(Some(0)), None);
}

#[test]
fn idle_limit_is_read_from_the_server_cmdline() {
    let cmdline = "bazel(ws)\0-jar\0/i/A-server.jar\0--max_idle_secs=90\0--output_base=/o\0";
    assert_eq!(idle_limit(cmdline), Some(90));
    assert_eq!(idle_limit("bazel(ws)\0--output_base=/o\0"), None);
    assert_eq!(idle_limit("--max_idle_secs=x\0"), None);
}

#[test]
fn startup_options_come_from_the_dx_command_line() {
    let words = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
    assert_eq!(
        own_startup_options(&words(&["test", "//..."])),
        Some(Vec::new())
    );
    assert_eq!(
        own_startup_options(&words(&[
            "test",
            "--bazel-startup-option=--output_base=/o",
            "//..."
        ])),
        Some(vec!["--output_base=/o".to_owned()])
    );
    assert_eq!(own_startup_options(&words(&["slots::"])), None);
}

const BASE: &str = "0123456789abcdef0123456789abcdef";

fn server(pid: u32, workspace: &str, root: &str, base: &str) -> Server {
    Server {
        pid,
        rss_bytes: 0,
        workspace: Some(PathBuf::from(workspace)),
        output_base: Some(PathBuf::from(base)),
        output_user_root: Some(PathBuf::from(root)),
    }
}

#[test]
fn own_server_is_the_one_this_workspace_reaches() {
    let roots = vec![PathBuf::from("/c/_bazel_u")];
    let base = format!("/c/_bazel_u/{BASE}");
    let servers = vec![
        server(
            1,
            "/other",
            "/c/_bazel_u",
            "/c/_bazel_u/ffffffffffffffffffffffffffffffff",
        ),
        server(2, "/ws", "/c/_bazel_u", &base),
    ];
    let pid = |root: Option<&str>, base: Option<&str>| {
        own_server(
            &servers,
            Path::new("/ws"),
            root.map(Path::new),
            base.map(Path::new),
            &roots,
        )
        .map(|found| found.pid)
    };
    assert_eq!(pid(None, None), Some(2));
    assert_eq!(pid(None, Some(&base)), Some(2));
    assert_eq!(pid(None, Some("/elsewhere")), None);
    assert_eq!(pid(Some("/elsewhere"), None), None);
    assert_eq!(
        own_server(&servers, Path::new("/nope"), None, None, &roots).map(|found| found.pid),
        None
    );
}

#[test]
fn own_server_skips_servers_a_plain_call_would_not_reach() {
    let roots = vec![PathBuf::from("/c/_bazel_u")];
    let custom = vec![server(3, "/ws", "/c/_bazel_u", "/c/_bazel_u/custom")];
    assert!(own_server(&custom, Path::new("/ws"), None, None, &roots).is_none());
    let other_root = vec![server(4, "/ws", "/r", &format!("/r/{BASE}"))];
    assert!(own_server(&other_root, Path::new("/ws"), None, None, &roots).is_none());
    assert_eq!(
        own_server(
            &other_root,
            Path::new("/ws"),
            Some(Path::new("/r")),
            None,
            &roots
        )
        .map(|found| found.pid),
        Some(4)
    );
    let two = vec![
        server(5, "/ws", "/c/_bazel_u", &format!("/c/_bazel_u/{BASE}")),
        server(
            6,
            "/ws",
            "/c/_bazel_u",
            "/c/_bazel_u/fedcba9876543210fedcba9876543210",
        ),
    ];
    assert!(own_server(&two, Path::new("/ws"), None, None, &roots).is_none());
}

#[test]
fn own_server_follows_an_rc_root_and_base() {
    let roots = vec![PathBuf::from("/c/_bazel_u")];
    let servers = vec![server(7, "/ws", "/r", "/r/custom")];
    let pid = |root: Option<&str>, base: Option<&str>| {
        own_server(
            &servers,
            Path::new("/ws"),
            root.map(Path::new),
            base.map(Path::new),
            &roots,
        )
        .map(|found| found.pid)
    };
    assert_eq!(pid(Some("/r"), Some("/r/custom")), Some(7));
    assert_eq!(pid(None, Some("/r/custom")), None);
    assert_eq!(pid(Some("/r"), None), None);
}

#[test]
fn reap_roots_add_the_rc_root_to_the_defaults() {
    let defaults = vec![PathBuf::from("/c/_bazel_u")];
    let none = dx_process::rc::StartupPaths::default();
    assert_eq!(reap_roots(&defaults, &none), defaults);
    let paths = dx_process::rc::StartupPaths {
        output_user_root: Some(PathBuf::from("/r")),
        output_base: None,
    };
    assert_eq!(
        reap_roots(&defaults, &paths),
        vec![PathBuf::from("/c/_bazel_u"), PathBuf::from("/r")]
    );
}

#[derive(Default)]
struct FakeLog {
    lookups: usize,
    pings: usize,
    stopped: usize,
    running: bool,
    dead: bool,
}

struct FakeWarm {
    server: Option<(u32, Option<u64>)>,
    log: std::rc::Rc<RefCell<FakeLog>>,
}

impl Warm for FakeWarm {
    type Ping = ();

    fn own_server(&self) -> Option<(u32, Option<u64>)> {
        self.log.borrow_mut().lookups += 1;
        self.server
    }

    fn alive(&self, _pid: u32) -> bool {
        !self.log.borrow().dead
    }

    fn ping(&self) -> Option<()> {
        let mut log = self.log.borrow_mut();
        assert!(!log.running, "pings overlap");
        log.pings += 1;
        log.running = true;
        Some(())
    }

    fn finished(&self, _ping: &mut ()) -> bool {
        !self.log.borrow().running
    }

    fn stop(&self, _ping: ()) {
        let mut log = self.log.borrow_mut();
        log.stopped += 1;
        log.running = false;
    }
}

fn keeper(
    server: Option<(u32, Option<u64>)>,
) -> (KeepWarm<FakeWarm>, std::rc::Rc<RefCell<FakeLog>>) {
    let log = std::rc::Rc::new(RefCell::new(FakeLog::default()));
    let warm = FakeWarm {
        server,
        log: log.clone(),
    };
    (KeepWarm::new(warm), log)
}

#[test]
fn no_running_server_means_no_ping() {
    let (mut keep, log) = keeper(None);
    let start = Instant::now();
    for step in 0..10 {
        keep.tick(start + Duration::from_secs(step * 100));
    }
    drop(keep);
    let log = log.borrow();
    assert_eq!(log.lookups, 1);
    assert_eq!(log.pings, 0);
    assert_eq!(log.stopped, 0);
}

#[test]
fn a_server_that_never_idles_out_is_never_pinged() {
    let (mut keep, log) = keeper(Some((7, Some(0))));
    let start = Instant::now();
    for step in 0..10 {
        keep.tick(start + Duration::from_secs(step * 100));
    }
    drop(keep);
    let log = log.borrow();
    assert_eq!(log.lookups, 1);
    assert_eq!(log.pings, 0);
    assert_eq!(log.stopped, 0);
}

#[test]
fn the_first_tick_pings_and_later_pings_wait_for_the_interval() {
    let (mut keep, log) = keeper(Some((7, Some(90))));
    let start = Instant::now();
    keep.tick(start);
    assert_eq!(log.borrow().pings, 1);
    log.borrow_mut().running = false;
    keep.tick(start + Duration::from_secs(29));
    assert_eq!(log.borrow().pings, 1);
    keep.tick(start + Duration::from_secs(30));
    assert_eq!(log.borrow().pings, 2);
    assert_eq!(log.borrow().lookups, 1);
}

#[test]
fn a_running_ping_is_never_doubled() {
    let (mut keep, log) = keeper(Some((7, Some(3))));
    let start = Instant::now();
    keep.tick(start);
    keep.tick(start + Duration::from_secs(5));
    keep.tick(start + Duration::from_secs(10));
    assert_eq!(log.borrow().pings, 1);
    log.borrow_mut().running = false;
    keep.tick(start + Duration::from_secs(11));
    assert_eq!(log.borrow().pings, 2);
}

#[test]
fn a_dead_server_stops_the_pings() {
    let (mut keep, log) = keeper(Some((7, Some(3))));
    let start = Instant::now();
    keep.tick(start);
    {
        let mut log = log.borrow_mut();
        log.running = false;
        log.dead = true;
    }
    keep.tick(start + Duration::from_secs(1));
    log.borrow_mut().dead = false;
    keep.tick(start + Duration::from_secs(100));
    assert_eq!(log.borrow().pings, 1);
}

#[test]
fn dropping_stops_a_running_ping() {
    let (mut keep, log) = keeper(Some((7, None)));
    keep.tick(Instant::now());
    assert!(log.borrow().running);
    drop(keep);
    assert_eq!(log.borrow().stopped, 1);
    assert!(!log.borrow().running);
    let (mut idle, idle_log) = keeper(Some((8, Some(900))));
    idle.tick(Instant::now());
    idle_log.borrow_mut().running = false;
    idle.tick(Instant::now());
    drop(idle);
    assert_eq!(idle_log.borrow().stopped, 0);
}

/// Frees the holder's slot on its first ping, so a waiting `take` gets in on its next poll.
struct ReleasingWarm {
    inner: FakeWarm,
    holder: RefCell<Option<Slot>>,
}

impl Warm for ReleasingWarm {
    type Ping = ();

    fn own_server(&self) -> Option<(u32, Option<u64>)> {
        self.inner.own_server()
    }

    fn alive(&self, pid: u32) -> bool {
        self.inner.alive(pid)
    }

    fn ping(&self) -> Option<()> {
        let ping = self.inner.ping();
        drop(self.holder.borrow_mut().take());
        ping
    }

    fn finished(&self, ping: &mut ()) -> bool {
        self.inner.finished(ping)
    }

    fn stop(&self, ping: ()) {
        self.inner.stop(ping);
    }
}

#[test]
fn a_waiting_take_keeps_its_server_warm_until_it_gets_a_slot() {
    let scratch = dx_test_scratch::scratch("dx-slots-keepwarm-");
    let dir = scratch.path().to_path_buf();
    let (holder, _, _) = take_gate(&Gate::new(dir.clone(), 1, "/w/holder".to_owned(), false));
    let log = std::rc::Rc::new(RefCell::new(FakeLog::default()));
    let warm = ReleasingWarm {
        inner: FakeWarm {
            server: Some((7, Some(90))),
            log: log.clone(),
        },
        holder: RefCell::new(Some(holder)),
    };
    let waiter = Gate::new(dir, 1, "/w/waiter".to_owned(), false);
    let mut err = Vec::new();
    let mut events = Vec::new();
    let got = waiter.take_warming(Command::Test, &mut err, &mut events, Some(warm));
    assert!(matches!(got, Ok(Some(_))));
    let log = log.borrow();
    assert_eq!((log.lookups, log.pings, log.stopped), (1, 1, 1));
    let err = String::from_utf8(err).expect("utf8");
    assert!(err.contains(&wait_message("/w/holder")), "{err}");
}

#[test]
fn a_free_slot_never_looks_for_a_server() {
    let scratch = dx_test_scratch::scratch("dx-slots-nowarm-");
    let gate = Gate::new(scratch.path().to_path_buf(), 1, "/w/a".to_owned(), false);
    let log = std::rc::Rc::new(RefCell::new(FakeLog::default()));
    let warm = FakeWarm {
        server: Some((7, Some(90))),
        log: log.clone(),
    };
    let got = gate.take_warming(Command::Test, &mut Vec::new(), &mut Vec::new(), Some(warm));
    assert!(matches!(got, Ok(Some(_))));
    let log = log.borrow();
    assert_eq!((log.lookups, log.pings), (0, 0));
}

#[test]
fn from_args_builds_warmth_from_argv_and_rc_roots() {
    let scratch = dx_test_scratch::scratch("dx-slots-warmth-");
    let root = scratch.path().to_path_buf();
    let workspace = root.join("ws");
    std::fs::create_dir_all(&workspace).expect("workspace");
    let rc_root = root.join("r");
    std::fs::write(
        workspace.join(".bazelrc"),
        format!("startup --output_user_root={}\n", rc_root.display()),
    )
    .expect("rc");
    let cache = root.join("cache").to_string_lossy().into_owned();
    let get = move |name: &str| (name == "XDG_CACHE_HOME").then(|| cache.clone());
    let base = root.join("ob");
    let option = format!("--output_base={}", base.display());
    let args: Vec<OsString> = vec![
        "test".into(),
        format!("--bazel-startup-option={option}").into(),
        "//...".into(),
    ];
    let gate = Gate::from_args(&workspace, &get, false, &[], count(1), &args).expect("gate");
    let warm = gate.warm.expect("warmth from argv");
    assert_eq!(warm.startup_options, vec![option]);
    assert_eq!(warm.output_user_root, Some(rc_root));
    assert_eq!(warm.output_base, Some(base));
    assert_eq!(warm.workspace, workspace);

    let args: Vec<OsString> = vec!["slots::".into()];
    let gate = Gate::from_args(&workspace, &get, false, &[], count(1), &args).expect("gate");
    assert!(
        gate.warm.is_none(),
        "an argv that is not a dx command warms nothing"
    );
}

#[test]
fn ping_argv_carries_startup_options_before_the_info_verb() {
    let warmth = Warmth {
        workspace: PathBuf::from("/ws"),
        startup_options: vec!["--output_base=/ob".to_owned()],
        output_user_root: None,
        output_base: None,
        default_roots: Vec::new(),
    };
    let argv = warmth.ping_argv();
    let base = argv.iter().position(|arg| arg == "--output_base=/ob");
    let info = argv.iter().position(|arg| arg == "info");
    assert!(base.is_some() && base < info, "{argv:?}");
    assert_eq!(
        argv[argv.len() - 3..],
        ["--noblock_for_lock", "info", "server_pid"]
    );
}

#[cfg(unix)]
#[test]
fn a_real_ping_child_is_reaped_on_stop() {
    let warmth = Warmth {
        workspace: std::env::temp_dir(),
        startup_options: Vec::new(),
        output_user_root: None,
        output_base: None,
        default_roots: Vec::new(),
    };
    let mut child = Process::new("sleep")
        .arg("30")
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn");
    assert!(!warmth.finished(&mut child));
    let pid = i32::try_from(child.id()).expect("pid");
    warmth.stop(child);
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0);
}
