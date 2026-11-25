use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::{
    ACTIVE_PORT_FILE, Browser, BrowserError, Frame, Launch, active_port, firefox_executable,
    missing_libraries, wait_for_control,
};

fn running_child() -> std::process::Child {
    let (stdin_reader, args): (&str, &[&str]) = if cfg!(windows) {
        ("findstr", &["x"])
    } else {
        ("cat", &[])
    };
    Command::new(stdin_reader)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("a running child")
}

fn fake_firefox(dir: &Path, stderr: &[&str], code: u8) -> PathBuf {
    let (path, body) = if cfg!(windows) {
        let lines: String = stderr
            .iter()
            .map(|line| format!("1>&2 echo {line}\r\n"))
            .collect();
        (
            dir.join("firefox.cmd"),
            format!("@echo off\r\n{lines}exit /b {code}\r\n"),
        )
    } else {
        let lines: String = stderr
            .iter()
            .map(|line| format!("echo '{line}' >&2\n"))
            .collect();
        (
            dir.join("firefox"),
            format!("#!/bin/sh\n{lines}exit {code}\n"),
        )
    };
    dx_testing::executable_script(&path, &body);
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
    assert!(
        matches!(error, BrowserError::FrameByte { byte: b'x' }),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "browser: frame length holds the byte 'x'"
    );
}

#[test]
fn the_active_port_is_read_from_the_profile() {
    let scratch_tmp = dx_test_scratch::scratch("browser-active-port").unwrap();
    let home = scratch_tmp.path();
    let missing = active_port(home).expect_err("no file yet");
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    std::fs::write(home.join(ACTIVE_PORT_FILE), "not a port").expect("a port file");
    let garbled = active_port(home).expect_err("a garbled file");
    assert_eq!(garbled.kind(), std::io::ErrorKind::InvalidData);
    std::fs::write(home.join(ACTIVE_PORT_FILE), "40123\n").expect("a port file");
    assert_eq!(active_port(home).expect("a port"), 40123);
}

#[test]
fn a_browser_connects_to_the_port_its_profile_names() {
    let scratch_tmp = dx_test_scratch::scratch("browser-connects").unwrap();
    let home = scratch_tmp.path();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    std::fs::write(home.join(ACTIVE_PORT_FILE), port.to_string()).expect("a port file");
    let mut child = running_child();
    let result = wait_for_control(
        &mut child,
        home,
        &home.join("browser.log"),
        Instant::now() + Duration::from_secs(10),
    );
    drop(child.kill());
    drop(child.wait());
    let Ok((_stream, connected)) = result else {
        panic!("the browser must connect to the port its profile names");
    };
    assert_eq!(connected, port);
}

#[test]
fn the_acquired_firefox_layout_names_the_browser_binary() {
    let scratch = dx_test_scratch::scratch("dx-browser-tests").unwrap();
    let root = scratch.path().to_path_buf();
    assert_eq!(firefox_executable(&root), "");
    let browser = root.join("extracted/firefox/firefox");
    std::fs::create_dir_all(browser.parent().expect("a parent")).expect("a tree");
    std::fs::write(&browser, b"binary").expect("a browser file");
    assert_eq!(firefox_executable(&root), browser.to_string_lossy());
}

#[test]
fn a_missing_browser_tree_is_reported_as_empty() {
    let root = PathBuf::from("/nonexistent-dx-firefox-tree");
    assert_eq!(firefox_executable(&root), "");
}

#[test]
fn a_browser_that_exits_at_launch_fails_fast_with_its_log() {
    let scratch_tmp = dx_test_scratch::scratch("browser-exits").unwrap();
    let scratch = scratch_tmp.path().to_path_buf();
    let firefox = fake_firefox(
        &scratch,
        &["libfake.so.2: cannot open shared object file"],
        1,
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
    assert!(matches!(error, BrowserError::Exited { .. }), "{error:?}");
    let error = error.to_string();
    assert!(error.contains("exited"), "{error}");
    assert!(
        error.contains("browser: missing shared library libfake.so.2 "),
        "{error}"
    );
    assert_eq!(error.matches("libfake.so.2").count(), 2, "{error}");
    assert!(profile.join("attempt-0").is_dir());
    assert!(!profile.join("attempt-1").exists(), "{error}");
}

#[test]
fn a_browser_missing_system_libraries_names_each_one_once() {
    let scratch_tmp = dx_test_scratch::scratch("browser-libraries").unwrap();
    let scratch = scratch_tmp.path().to_path_buf();
    let firefox = fake_firefox(
        &scratch,
        &[
            "firefox: error while loading shared libraries: libasound.so.2: cannot open shared object file: No such file or directory",
            "libgtk-3.so.0: cannot open shared object file: No such file or directory",
            "libasound.so.2: cannot open shared object file: No such file or directory",
        ],
        127,
    );
    let profile = scratch.join("profile");
    let Err(error) = Browser::start(&firefox, &profile) else {
        panic!("a browser that exits must not start");
    };
    let error = error.to_string();
    assert!(
        error.contains("browser: missing shared library libasound.so.2 "),
        "{error}"
    );
    assert!(
        error.contains("browser: missing shared library libgtk-3.so.0 "),
        "{error}"
    );
    assert_eq!(
        error.matches("browser: missing shared library").count(),
        2,
        "{error}"
    );
}

#[test]
fn a_log_without_a_loader_error_names_no_library() {
    let scratch_tmp = dx_test_scratch::scratch("browser-no-loader").unwrap();
    let log = scratch_tmp.path().join("browser.log");
    std::fs::write(&log, "Segmentation fault\nexit\n").expect("a browser log");
    assert_eq!(missing_libraries(&log), "");
}

#[test]
fn a_browser_that_never_opens_its_port_reports_the_timeout() {
    let scratch_tmp = dx_test_scratch::scratch("browser-silent").unwrap();
    let scratch = scratch_tmp.path().to_path_buf();
    let log = scratch.join("browser.log");
    std::fs::write(&log, "still starting\n").expect("a browser log");
    let mut child = running_child();
    let result = wait_for_control(
        &mut child,
        &scratch,
        &log,
        Instant::now() + Duration::from_secs(1),
    );
    drop(child.kill());
    drop(child.wait());
    let Err(Launch::Retry(error)) = result else {
        panic!("a silent browser must time out as retryable");
    };
    assert!(
        matches!(error, BrowserError::NoControlPort { .. }),
        "{error:?}"
    );
    let error = error.to_string();
    assert!(error.contains("opened no control port"), "{error}");
    assert!(error.contains("still starting"), "{error}");
}
