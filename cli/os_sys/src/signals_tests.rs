use super::*;

#[test]
fn blocked_signals_come_back_when_dropped() {
    std::thread::spawn(|| {
        assert!(!blocked(libc::SIGUSR2));
        {
            let _blocked = Blocked::new(&[libc::SIGUSR2]);
            assert!(blocked(libc::SIGUSR2));
        }
        assert!(!blocked(libc::SIGUSR2));
    })
    .join()
    .expect("thread");
}

#[test]
fn hold_stop_holds_sigint_and_sigterm_until_dropped() {
    std::thread::spawn(|| {
        {
            let _held = hold_stop();
            assert!(blocked(libc::SIGINT));
            assert!(blocked(libc::SIGTERM));
        }
        assert!(!blocked(libc::SIGINT));
        assert!(!blocked(libc::SIGTERM));
    })
    .join()
    .expect("thread");
}

#[test]
fn background_tty_writes_block_sigttou_in_this_thread() {
    std::thread::spawn(|| {
        allow_background_tty_writes();
        assert!(blocked(libc::SIGTTOU));
    })
    .join()
    .expect("thread");
}

#[test]
fn raising_an_ignored_by_default_signal_returns() {
    raise_default(libc::SIGURG).expect("raise");
}

const UNLINK_CHILD: &str = "DX_OS_SYS_UNLINK_ON_STOP_CHILD";

#[test]
fn unlink_on_stop_child() {
    let Some(path) = std::env::var_os(UNLINK_CHILD) else {
        return;
    };
    unlink_on_stop(Path::new("first")).expect("watch the first path");
    unlink_on_stop(Path::new(&path)).expect("watch");
    let pid = i32::try_from(std::process::id()).expect("pid fits pid_t");
    crate::process::signal(pid, libc::SIGTERM).expect("signal self");
    park_forever();
}

fn park_forever() -> ! {
    loop {
        std::thread::park();
    }
}

#[test]
fn sigterm_removes_the_watched_path_and_exits_143() {
    let dir = std::env::var_os("TEST_TMPDIR").map_or_else(std::env::temp_dir, Into::into);
    let part = dir.join(format!("unlink-on-stop-{}.part", std::process::id()));
    std::fs::write(&part, b"partial").expect("write");
    let child = std::process::Command::new(std::env::current_exe().expect("exe"))
        .args(["--exact", "signals::signals_tests::unlink_on_stop_child"])
        .env(UNLINK_CHILD, &part)
        .spawn()
        .expect("run the child");
    let exit = dx_test_scratch::wait::exit_within(child, std::time::Duration::from_secs(30))
        .expect("wait for the child");
    let exited_143 =
        matches!(&exit, dx_test_scratch::wait::Exit::Exited(status) if status.code() == Some(143));
    assert!(exited_143, "{exit:?}");
    assert!(!part.exists());
}

#[test]
fn a_path_with_a_nul_byte_is_refused() {
    use std::os::unix::ffi::OsStrExt;
    let path = Path::new(std::ffi::OsStr::from_bytes(b"a\0b"));
    let error = unlink_on_stop(path).expect_err("NUL");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("NUL byte at offset 1"), "{error}");
}

#[test]
fn keep_on_stop_clears_the_path() {
    keep_on_stop();
    assert!(UNLINK_ON_STOP.load(Ordering::SeqCst).is_null());
}
