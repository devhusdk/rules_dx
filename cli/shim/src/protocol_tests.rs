use super::*;
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    move |name: &str| map.get(name).cloned()
}

fn workspace(pin: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".dx")).expect("mkdir");
    std::fs::write(dir.path().join(VERSION_FILE), pin).expect("pin");
    dir
}

#[test]
fn resolves_from_nested_cwd_past_git() {
    let dir = workspace("1.2.3\n");
    let nested = dir.path().join("a/.git/b/c");
    std::fs::create_dir_all(&nested).expect("nested");
    std::fs::create_dir_all(dir.path().join("a/.git")).expect("git");
    let (version, source) = resolve_version(&env(&[]), &nested).expect("resolves");
    assert_eq!(version, "1.2.3");
    assert_eq!(source, VersionSource::File(dir.path().join(VERSION_FILE)));
}

#[test]
fn nearest_pin_wins() {
    let dir = workspace("1.0.0\n");
    let inner = dir.path().join("sub");
    std::fs::create_dir_all(inner.join(".dx")).expect("mkdir");
    std::fs::write(inner.join(VERSION_FILE), "2.0.0\n").expect("pin");
    let (version, _) = resolve_version(&env(&[]), &inner.join("x")).expect("resolves");
    assert_eq!(version, "2.0.0");
}

#[test]
fn pin_strips_crlf_and_spaces() {
    assert_eq!(parse_pin("1.2.3\r\n").as_deref(), Some("1.2.3"));
    assert_eq!(parse_pin(" 1.2.3 \r\nignored\n").as_deref(), Some("1.2.3"));
    assert_eq!(parse_pin("1.2.3").as_deref(), Some("1.2.3"));
    let dir = workspace("1.2.3 \r\n");
    let (version, _) = resolve_version(&env(&[]), dir.path()).expect("resolves");
    assert_eq!(version, "1.2.3");
}

#[test]
fn env_beats_file() {
    let dir = workspace("1.2.3\n");
    let (version, source) =
        resolve_version(&env(&[("DX_VERSION", "4.5.6")]), dir.path()).expect("resolves");
    assert_eq!(version, "4.5.6");
    assert_eq!(source, VersionSource::Env);
}

#[test]
fn bad_env_version_is_a_usage_error() {
    let dir = workspace("1.2.3\n");
    for bad in ["v1.2.3", "1.2", "../x", "1.2.3/../../x", "latest"] {
        let err = resolve_version(&env(&[("DX_VERSION", bad)]), dir.path()).expect_err(bad);
        assert_eq!(err.code, EXIT_USAGE, "{bad}");
        assert!(err.message.contains(bad), "{}", err.message);
    }
}

#[test]
fn bad_pin_is_refused() {
    for bad in ["../x\n", "", "\n1.2.3\n", "v1.2.3\n"] {
        let dir = workspace(bad);
        let err = resolve_version(&env(&[]), dir.path()).expect_err(bad);
        assert_eq!(err.code, EXIT_FAILURE, "{bad:?}");
        assert!(err.message.contains(".dx"), "{}", err.message);
    }
}

#[test]
fn outside_a_workspace_names_cwd() {
    let dir = tempfile::tempdir().expect("tempdir");
    if find_version_file(dir.path()).is_some() {
        return;
    }
    let err = resolve_version(&env(&[]), dir.path()).expect_err("no workspace");
    assert_eq!(err.code, EXIT_FAILURE);
    assert_eq!(
        err.to_string(),
        format!(
            "dx: not in a dx workspace (no .dx/version above {}); run 'dx init' from a Bazel workspace with bazel run @rules_dx//:dx -- init --apply, or set DX_VERSION",
            dir.path().display()
        )
    );
}

#[test]
fn platforms_map_to_the_five_assets() {
    let cases = [
        ("linux", "x86_64", Some("dx-linux-x86_64")),
        ("linux", "aarch64", Some("dx-linux-arm64")),
        ("macos", "aarch64", Some("dx-macos-arm64")),
        ("windows", "x86_64", Some("dx-windows-x86_64")),
        ("windows", "aarch64", Some("dx-windows-arm64")),
        ("macos", "x86_64", None),
        ("linux", "riscv64", None),
        ("freebsd", "x86_64", None),
    ];
    for (os, arch, want) in cases {
        assert_eq!(
            platform(os, arch).map(asset_name).as_deref(),
            want,
            "{os} {arch}"
        );
    }
}

#[test]
fn unsupported_platform_text() {
    assert_eq!(
        unsupported_platform("1.2.3", "linux", "riscv64").to_string(),
        "dx: no dx 1.2.3 binary for linux riscv64; supported: linux x86_64/arm64, macos arm64, windows x86_64/arm64; use bazel run @rules_dx//:dx -- ..."
    );
}

#[test]
fn windows_cache_file_has_exe_but_asset_does_not() {
    let win = platform("windows", "x86_64").expect("win");
    assert_eq!(asset_name(win), "dx-windows-x86_64");
    assert_eq!(cache_file_name(win), "dx-windows-x86_64.exe");
    let linux = platform("linux", "x86_64").expect("linux");
    assert_eq!(cache_file_name(linux), "dx-linux-x86_64");
    assert_eq!(
        bin_path(Path::new("/c"), "1.2.3", win),
        Path::new("/c/bin/1.2.3/dx-windows-x86_64.exe")
    );
}

#[test]
fn cache_root_precedence() {
    let all = [
        ("DX_CACHE_DIR", "/dx"),
        ("XDG_CACHE_HOME", "/xdg"),
        ("HOME", "/home/u"),
        ("USERPROFILE", "/profile"),
    ];
    type Case<'a> = (&'a [(&'a str, &'a str)], bool, Option<&'a str>);
    let cases: [Case<'_>; 8] = [
        (&all, false, Some("/dx")),
        (&all[1..], false, Some("/xdg/dx")),
        (&all[2..], false, Some("/home/u/.cache/dx")),
        (&all[2..], true, Some("/profile/.cache/dx")),
        (
            &[("DX_CACHE_DIR", ""), ("XDG_CACHE_HOME", ""), ("HOME", "/h")],
            false,
            Some("/h/.cache/dx"),
        ),
        (&[("USERPROFILE", "/p")], false, None),
        (&[("HOME", "/h")], true, None),
        (&[], false, None),
    ];
    for (pairs, windows, want) in cases {
        assert_eq!(
            cache_root(&env(pairs), windows),
            want.map(PathBuf::from),
            "{pairs:?} windows={windows}"
        );
    }
}

#[test]
fn release_url_layout() {
    assert_eq!(
        release_url(DEFAULT_RELEASE_BASE, "1.2.3", "dx-linux-x86_64"),
        "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-linux-x86_64"
    );
    assert_eq!(
        release_url("http://127.0.0.1:9/r/", "1.2.3", "a"),
        "http://127.0.0.1:9/r/v1.2.3/a"
    );
}

#[test]
fn release_base_allows_https_and_loopback_http_only() {
    assert_eq!(
        release_base(&env(&[])).expect("default"),
        DEFAULT_RELEASE_BASE
    );
    for ok in [
        "https://mirror.example/r",
        "http://127.0.0.1:8080/r",
        "http://localhost/r",
        "http://[::1]:1/r",
    ] {
        assert!(
            release_base(&env(&[("DX_RELEASE_BASE_URL", ok)])).is_ok(),
            "{ok}"
        );
    }
    for bad in [
        "http://example.com/r",
        "http://127.0.0.1.evil.com/r",
        "http://localhost@evil.com/r",
        "ftp://127.0.0.1/r",
        "https://",
        "file:///tmp",
    ] {
        let err = release_base(&env(&[("DX_RELEASE_BASE_URL", bad)])).expect_err(bad);
        assert_eq!(err.code, EXIT_USAGE);
    }
}

#[test]
fn pin_rule_matches_tools_dx() {
    for input in [
        "1.2.3",
        "0.0.0",
        "1.2.3-rc.1",
        "1.2.3+build",
        "v1.2.3",
        "1.2",
        "",
        "../x",
        "1.2.3 4",
        "01.2.3",
    ] {
        assert_eq!(
            version_is_valid(input),
            dx_adopt::pin_is_valid(input),
            "{input:?}"
        );
    }
}

#[test]
fn shim_version_tracks_dx_version() {
    assert_eq!(SHIM_VERSION, dx_adopt::DX_VERSION);
}
