use super::{control_port, firefox_executable, Frame};

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
