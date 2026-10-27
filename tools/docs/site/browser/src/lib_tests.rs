use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::{control_port, firefox_executable, wait_for_control, Browser, Frame, Launch};

fn script(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("firefox");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a fake browser");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("an executable");
    path
}

#[test]
fn a_frame_reports_completion_once_the_declared_length_arrives() {
    let mut frame = Frame::default();
    assert_eq!(frame.push(b'2').expect("the length digit"), None);
    assert_eq!(frame.push(b':').expect("a colon"), None);
    assert_eq!(frame.push(b'h').expect("the first body byte"), None);
    assert_eq!(
        frame.push(b'i').expect("the last body byte"),
        Some("hi".to_string())
    );
}

#[test]
fn a_frame_reports_an_empty_body_and_resets_for_the_next_one() {
    let mut frame = Frame::default();
    assert_eq!(frame.push(b'0').expect("a zero length digit"), None);
    assert_eq!(frame.push(b':').expect("a colon"), Some(String::new()));
    assert_eq!(frame.push(b'3').expect("the next length digit"), None);
    assert_eq!(frame.push(b':').expect("the next colon"), None);
    for byte in "ab".bytes() {
        assert_eq!(frame.push(byte).expect("a body byte"), None);
    }
    assert_eq!(
        frame.push(b'c').expect("the last body byte"),
        Some("abc".to_string())
    );
}

#[test]
fn a_frame_counts_body_bytes_so_multibyte_text_finishes_at_its_declared_length() {
    let mut frame = Frame::default();
    assert_eq!(frame.push(b'5').expect("the length digit"), None);
    assert_eq!(frame.push(b':').expect("a colon"), None);
    let mut last = None;
    for byte in "h\u{2192}i".bytes() {
        last = frame.push(byte).expect("a body byte");
    }
    assert_eq!(last, Some("h\u{2192}i".to_string()));
    assert_eq!(frame.push(b'2').expect("the next length digit"), None);
    assert_eq!(frame.push(b':').expect("the next colon"), None);
    assert_eq!(frame.push(b'o').expect("a body byte"), None);
    assert_eq!(
        frame.push(b'k').expect("the last body byte"),
        Some("ok".to_string())
    );
}

#[test]
fn a_frame_refuses_a_length_that_is_not_a_number() {
    let mut frame = Frame::default();
    let error = frame.push(b'x').expect_err("a non-digit length");
    assert!(error.contains("frame length"), "{error}");
}

#[test]
fn the_control_port_is_a_bound_loopback_port() {
    let port = control_port();
    assert!(port > 1024, "{port} is a privileged port");
    assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok());
}

#[test]
fn the_acquired_firefox_layout_names_the_browser_binary() {
    let root = std::env::temp_dir().join(format!("dx-browser-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(firefox_executable(&root), "");
    let browser = root.join("extracted/firefox/firefox");
    std::fs::create_dir_all(browser.parent().expect("a parent")).expect("a tree");
    std::fs::write(&browser, b"binary").expect("a browser file");
    assert_eq!(firefox_executable(&root), browser.to_string_lossy());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_browser_tree_is_reported_as_empty() {
    let root = std::path::PathBuf::from("/nonexistent-dx-firefox-tree");
    assert_eq!(firefox_executable(&root), "");
}

#[test]
fn a_browser_that_exits_at_launch_fails_fast_with_its_log() {
    let scratch_tmp = dx_test_scratch::scratch("browser-exits");
    let scratch = scratch_tmp.path().to_path_buf();
    let firefox = script(
        &scratch,
        "echo 'libfake.so.2: cannot open shared object file' >&2\nexit 1",
    );
    let profile = scratch.join("profile");
    let started = Instant::now();
    let Err(error) = Browser::start(&firefox, &profile) else {
        panic!("a browser that exits must not start");
    };
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert!(error.contains("exited"), "{error}");
    assert_eq!(error.matches("libfake.so.2").count(), 1, "{error}");
    assert!(profile.join("attempt-0").is_dir());
    assert!(!profile.join("attempt-1").exists(), "{error}");
}

#[test]
fn a_browser_that_never_opens_its_port_reports_the_timeout() {
    let scratch_tmp = dx_test_scratch::scratch("browser-silent");
    let scratch = scratch_tmp.path().to_path_buf();
    let log = scratch.join("browser.log");
    std::fs::write(&log, "still starting\n").expect("a browser log");
    let mut child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a silent child");
    let port = control_port();
    let result = wait_for_control(
        &mut child,
        &log,
        port,
        Instant::now() + Duration::from_secs(1),
    );
    let _ = child.kill();
    let _ = child.wait();
    let Err(Launch::Retry(error)) = result else {
        panic!("a silent browser must time out as retryable");
    };
    assert!(error.contains("opened no control port"), "{error}");
    assert!(error.contains("still starting"), "{error}");
}
