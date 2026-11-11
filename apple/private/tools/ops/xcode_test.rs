//! Tests the Xcode lookup and the tool runner.

mod fake;

use std::path::PathBuf;

use dx_apple_ops::xcode::{
    CODESIGN, DITTO, PLUTIL, SECURITY, Tools, XCRUN, developer_dir, for_test_runner, short_version,
    version_matches,
};
use dx_test_scratch::{scratch, write};
use fake::{Fake, env, ok};

const PLIST: &str = "<plist><dict>\n\t<key>CFBundleShortVersionString</key>\n\t<string>15.2</string>\n</dict></plist>";

fn install(root: &std::path::Path, app: &str, version: &str) -> PathBuf {
    let text = PLIST.replace("15.2", version);
    write(root, &format!("{app}/Contents/version.plist"), &text);
    root.join(app).join("Contents/Developer")
}

#[test]
fn the_short_version_is_read_from_the_plist() {
    assert_eq!(short_version(PLIST), Some("15.2"));
    assert_eq!(short_version("<plist></plist>"), None);
}

#[test]
fn a_configured_version_matches_the_installed_short_version() {
    assert!(version_matches("15.2.0.15C500b", "15.2"));
    assert!(version_matches("16", "16.0.0"));
    assert!(version_matches("15.0.1", "15.0.1"));
    assert!(!version_matches("15.2.0.15C500b", "15.3"));
    assert!(!version_matches("15.2.1", "15.2"));
    assert!(!version_matches("", "15.2"));
}

#[test]
fn the_developer_directory_is_the_installed_xcode_of_that_version() {
    let root = scratch("xcode-find");
    let wanted = install(root.path(), "Xcode-15.2.app", "15.2");
    install(root.path(), "Xcode-16.app", "16.0");
    write(root.path(), "Notes.txt", "x");
    assert_eq!(
        developer_dir("15.2.0.15C500b", None, root.path()),
        Ok(wanted)
    );
}

#[test]
fn a_preset_developer_directory_wins_only_for_the_same_version() {
    let root = scratch("xcode-preset");
    let other = install(root.path(), "Other.app", "15.2");
    let first = install(root.path(), "A.app", "15.2");
    let preset = other.to_string_lossy().into_owned();
    assert_eq!(developer_dir("15.2", Some(&preset), root.path()), Ok(other));
    assert_eq!(
        developer_dir("16.0", Some(&preset), root.path()).map_err(|f| f.code),
        Err(127)
    );
    assert_eq!(developer_dir("15.2", None, root.path()), Ok(first));
}

#[test]
fn a_missing_or_unset_version_fails_instead_of_guessing() {
    let root = scratch("xcode-missing");
    install(root.path(), "Xcode.app", "15.2");
    let missing = developer_dir("14.0", None, root.path()).unwrap_err();
    assert_eq!(missing.code, 127);
    assert!(missing.message.contains("Xcode 14.0 is not installed"));
    let unset = developer_dir("", None, root.path()).unwrap_err();
    assert_eq!(unset.code, 2);
}

#[test]
fn the_test_runner_needs_a_version_or_a_developer_directory() {
    let neither = for_test_runner(&env(&[])).unwrap_err();
    assert_eq!(neither.code, 2);
    assert!(
        neither
            .message
            .contains("neither XCODE_VERSION_OVERRIDE nor DEVELOPER_DIR")
    );
    assert_eq!(
        for_test_runner(&env(&[("DEVELOPER_DIR", "/x/Developer")])),
        Ok(PathBuf::from("/x/Developer"))
    );
}

#[test]
fn tools_start_the_absolute_paths_with_the_developer_directory() {
    let fake = Fake::new(|_| ok(""));
    let tools = Tools {
        developer_dir: Some(PathBuf::from("/x/Developer")),
        runner: &fake,
    };
    tools.xcrun(&["simctl", "list"]);
    tools.tool(DITTO, &["-x"]);
    tools.tool(PLUTIL, &[]);
    let calls = fake.calls.borrow();
    let programs: Vec<&str> = calls.iter().map(|cmd| cmd.program.as_str()).collect();
    assert_eq!(programs, [XCRUN, DITTO, PLUTIL]);
    for program in [XCRUN, CODESIGN, SECURITY, PLUTIL, DITTO] {
        assert!(program.starts_with('/'), "{program}");
    }
    for cmd in calls.iter() {
        assert_eq!(
            cmd.env,
            [("DEVELOPER_DIR".to_owned(), "/x/Developer".to_owned())]
        );
    }
}
