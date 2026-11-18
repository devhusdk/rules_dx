use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

const VERSION: &str = "1.2.3";
const BINARY: &[u8] = b"pinned dx binary";

struct Fixture {
    _dir: tempfile::TempDir,
    workspace: PathBuf,
    cache: PathBuf,
    exe: PathBuf,
}

fn fixture(module: Option<&str>, pins: Option<&str>) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let workspace = dir.path().join("ws");
    std::fs::create_dir_all(workspace.join(".dx")).expect("workspace");
    if let Some(module) = module {
        std::fs::write(workspace.join("MODULE.bazel"), module).expect("module");
    }
    if let Some(pins) = pins {
        std::fs::write(workspace.join(protocol::VERSION_FILE), pins).expect("pins");
    }
    let exe = dir.path().join("dx");
    std::fs::write(&exe, BINARY).expect("exe");
    Fixture {
        cache: dir.path().join("cache"),
        workspace,
        exe,
        _dir: dir,
    }
}

fn module() -> String {
    format!("bazel_dep(name = \"rules_dx\", version = \"{VERSION}\")\n")
}

fn pins_for(hash: &str) -> String {
    format!("{VERSION} {hash} dx-linux-x86_64\n")
}

fn run(fixture: &Fixture, os: &str, arch: &str) -> Result<Cached, String> {
    let cache = fixture.cache.clone();
    let env_get = move |name: &str| (name == "DX_CACHE_DIR").then(|| OsString::from(&cache));
    ensure(&Host {
        workspace: &fixture.workspace,
        env_get: &env_get,
        os,
        arch,
        exe: &fixture.exe,
    })
}

fn cached_path(fixture: &Fixture) -> PathBuf {
    fixture
        .cache
        .join("bin")
        .join(VERSION)
        .join("dx-linux-x86_64")
}

#[test]
fn places_the_running_dx_when_it_matches_the_pin() {
    let pins = pins_for(&dx_digest::sha256_hex(BINARY));
    let fixture = fixture(Some(&module()), Some(&pins));
    let got = run(&fixture, "linux", "x86_64").expect("cached");
    let path = cached_path(&fixture);
    assert_eq!(
        got,
        Cached::Placed {
            version: VERSION.to_owned(),
            path: path.clone(),
        }
    );
    assert_eq!(std::fs::read(&path).expect("cached file"), BINARY);
    assert_eq!(
        run(&fixture, "linux", "x86_64").expect("again"),
        Cached::Present {
            version: VERSION.to_owned(),
            path,
        }
    );
}

#[test]
fn a_running_dx_that_is_not_the_pinned_binary_is_never_cached() {
    let pins = pins_for(&"a".repeat(64));
    let fixture = fixture(Some(&module()), Some(&pins));
    assert_eq!(
        run(&fixture, "linux", "x86_64").expect("checked"),
        Cached::Mismatch {
            version: VERSION.to_owned(),
            asset: "dx-linux-x86_64".to_owned(),
        }
    );
    assert!(!cached_path(&fixture).exists());
    assert_eq!(
        notice(&run(&fixture, "linux", "x86_64").expect("checked")).severity,
        "warning"
    );
}

#[test]
fn workspaces_without_a_pinned_release_go_through_bazel() {
    for (module, pins) in [
        (None, None),
        (Some("module(name = \"app\")\n".to_owned()), None),
        (Some(module()), None),
        (
            Some(module()),
            Some(format!("{VERSION} {} dx-macos-arm64\n", "b".repeat(64))),
        ),
    ] {
        let fixture = fixture(module.as_deref(), pins.as_deref());
        assert_eq!(
            run(&fixture, "linux", "x86_64").expect("checked"),
            Cached::NotPinned {
                asset: "dx-linux-x86_64".to_owned(),
            },
            "module: {module:?}, pins: {pins:?}"
        );
        assert!(!fixture.cache.exists());
    }
}

#[test]
fn unsupported_hosts_and_a_missing_home_are_reported() {
    let fixture = fixture(Some(&module()), None);
    assert_eq!(
        run(&fixture, "freebsd", "x86_64").expect("checked"),
        Cached::Unsupported {
            os: "freebsd".to_owned(),
            arch: "x86_64".to_owned(),
        }
    );
    let got = ensure(&Host {
        workspace: &fixture.workspace,
        env_get: &|_| None,
        os: "linux",
        arch: "x86_64",
        exe: &fixture.exe,
    })
    .expect("checked");
    assert_eq!(got, Cached::NoCacheDir { home: "HOME" });
    assert!(notice(&got).message.contains("DX_CACHE_DIR"));
}

#[test]
fn an_unreadable_running_binary_is_an_error() {
    let pins = pins_for(&dx_digest::sha256_hex(BINARY));
    let mut fixture = fixture(Some(&module()), Some(&pins));
    fixture.exe = Path::new(&fixture.exe).with_file_name("missing");
    let error = run(&fixture, "linux", "x86_64").expect_err("missing exe");
    assert!(error.contains("missing"), "{error}");
}

#[test]
fn json_output_carries_the_notice_and_errors_fail_the_step() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let ok = report(
        &Ok(Cached::NotPinned {
            asset: "dx-linux-x86_64".to_owned(),
        }),
        OutputMode::Json,
        &mut out,
        &mut err,
    )
    .expect("written");
    assert!(ok);
    let out = String::from_utf8(out).expect("utf8");
    assert!(out.contains("\"code\":\"dx_not_cached\""), "{out}");
    assert!(out.contains("\"severity\":\"info\""), "{out}");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let ok = report(
        &Err("disk full".to_owned()),
        OutputMode::Text { quiet: false },
        &mut out,
        &mut err,
    )
    .expect("written");
    assert!(!ok);
    assert!(out.is_empty());
    let err = String::from_utf8(err).expect("utf8");
    assert!(
        err.contains("dx: warning: dx_not_cached: could not cache dx: disk full"),
        "{err}"
    );
}
