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
