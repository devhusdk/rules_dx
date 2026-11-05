use super::*;
use std::collections::HashMap;

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let map: HashMap<String, OsString> = pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(*v)))
        .collect();
    move |name: &str| map.get(name).cloned()
}

fn module(version: &str) -> String {
    format!("module(name = \"app\")\nbazel_dep(name = \"rules_dx\", version = \"{version}\")\n")
}

fn workspace(module_text: &str, pins: Option<&str>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("MODULE.bazel"), module_text).expect("module");
    if let Some(pins) = pins {
        std::fs::create_dir_all(dir.path().join(".dx")).expect("mkdir");
        std::fs::write(dir.path().join(VERSION_FILE), pins).expect("pins");
    }
    dir
}

fn pinned(version: &str, pins: &str) -> Route {
    Route::Module {
        version: version.to_owned(),
        pins: Some(pins.to_owned()),
    }
}

#[test]
fn resolves_the_version_from_module_bazel_in_an_ancestor() {
    let dir = workspace(&module("1.2.3"), Some("1.2.3\r\n"));
    let nested = dir.path().join("a/b/c");
    std::fs::create_dir_all(&nested).expect("nested");
    let route = resolve_version(&env(&[]), &nested).expect("resolves");
    assert_eq!(route, pinned("1.2.3", "1.2.3\r\n"));
}

#[test]
fn nearest_module_wins() {
    let dir = workspace(&module("1.0.0"), None);
    let inner = dir.path().join("sub");
    std::fs::create_dir_all(&inner).expect("mkdir");
    std::fs::write(inner.join("MODULE.bazel"), module("2.0.0")).expect("module");
    let route = resolve_version(&env(&[]), &inner).expect("resolves");
    assert_eq!(
        route,
        Route::Module {
            version: "2.0.0".into(),
            pins: None
        }
    );
}

#[test]
fn unusable_modules_route_through_bazel() {
    let dep = module("1.2.3");
    let cases = [
        "module(name = \"app\")\n".to_owned(),
        "bazel_dep(name = \"rules_dx\", version = \"1.2.3\", dev_dependency = True)\n".to_owned(),
        "bazel_dep(name = \"rules_dx\", version = VERSION)\n".to_owned(),
        "bazel_dep(name = \"rules_dx\", version = \"\")\n".to_owned(),
        "bazel_dep(name = \"rules_dx\", version = \"../x\")\n".to_owned(),
        format!("{dep}{dep}"),
        format!("module(name=\"x\", module_name = \"rules_dx\")\n{dep}"),
    ];
    for text in cases {
        let dir = workspace(&text, Some("1.2.3\n"));
        let route = resolve_version(&env(&[]), dir.path()).expect("resolves");
        assert_eq!(route, Route::Bazel, "{text}");
    }
}

#[test]
fn no_module_routes_through_bazel() {
    let dir = tempfile::tempdir().expect("tempdir");
    if find_module_file(dir.path()).is_some() {
        return;
    }
    let route = resolve_version(&env(&[]), dir.path()).expect("resolves");
    assert_eq!(route, Route::Bazel);
}

#[test]
fn pinned_hash_reads_the_line_for_version_and_asset() {
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    let text = format!("1.2.3 {a} dx-linux-x86_64\r\n1.2.3 {b} dx-macos-arm64\n");
    let hash = |version, asset| pinned_hash(&text, version, asset);
    assert_eq!(hash("1.2.3", "dx-linux-x86_64"), Some(a));
    assert_eq!(hash("1.2.3", "dx-macos-arm64"), Some(b));
    assert_eq!(hash("1.2.3", "dx-windows-arm64"), None);
    assert_eq!(hash("1.2.4", "dx-linux-x86_64"), None);
}

#[test]
fn pinned_hash_rejects_bad_lines() {
    let a = "a".repeat(64);
    assert_eq!(pinned_hash("1.2.3\n", "1.2.3", "dx-linux-x86_64"), None);
    assert_eq!(
        pinned_hash("1.2.3 abc dx-linux-x86_64\n", "1.2.3", "dx-linux-x86_64"),
        None
    );
    let extra = format!("1.2.3 {a} dx-linux-x86_64 extra\n");
    assert_eq!(pinned_hash(&extra, "1.2.3", "dx-linux-x86_64"), None);
}

#[test]
fn env_beats_module() {
    let dir = workspace(&module("1.2.3"), None);
    let route = resolve_version(&env(&[("DX_VERSION", "4.5.6")]), dir.path()).expect("resolves");
    assert_eq!(route, Route::Env("4.5.6".into()));
}

#[test]
fn bad_env_version_is_a_usage_error() {
    let dir = workspace(&module("1.2.3"), None);
    for bad in ["v1.2.3", "1.2", "../x", "1.2.3/../../x", "latest"] {
        let err = resolve_version(&env(&[("DX_VERSION", bad)]), dir.path()).expect_err(bad);
        assert_eq!(err.code, EXIT_USAGE, "{bad}");
        assert!(err.message.contains(bad), "{}", err.message);
    }
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

#[cfg(unix)]
mod non_utf8 {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    fn bad() -> OsString {
        OsString::from_vec(vec![b'/', 0xff, 0xfe])
    }

    fn env_of(name: &'static str) -> impl Fn(&str) -> Option<OsString> {
        move |key: &str| (key == name).then(bad)
    }

    #[test]
    fn cache_dir_is_used_as_is() {
        let root = cache_root(&env_of("DX_CACHE_DIR"), false).expect("root");
        assert_eq!(root, PathBuf::from(bad()));
    }

    #[test]
    fn home_is_not_reported_unset() {
        let root = cache_root(&env_of("HOME"), false).expect("root");
        assert_eq!(root, PathBuf::from(bad()).join(".cache").join("dx"));
    }

    #[test]
    fn release_base_and_version_name_the_variable() {
        let dir = workspace(&module("1.2.3"), None);
        let err = release_base(&env_of("DX_RELEASE_BASE_URL")).expect_err("base");
        assert_eq!(err.code, EXIT_USAGE);
        assert!(
            err.message.contains("DX_RELEASE_BASE_URL"),
            "{}",
            err.message
        );
        let err = resolve_version(&env_of("DX_VERSION"), dir.path()).expect_err("version");
        assert_eq!(err.code, EXIT_USAGE);
        assert!(err.message.contains("DX_VERSION"), "{}", err.message);
    }
}

fn sysctl_with<'a>(set: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |key| {
        set.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_owned())
    }
}

#[test]
fn rosetta_on_apple_silicon_resolves_the_arm64_asset() {
    let dir = workspace(&module("1.2.3"), None);
    let cache = tempfile::tempdir().expect("cache");
    let cache_dir = cache.path().to_str().expect("utf8");
    for set in [
        &[("hw.optional.arm64", "1\n")][..],
        &[("hw.optional.arm64", "0"), ("sysctl.proc_translated", "1")][..],
    ] {
        let arch = effective_arch("macos", "x86_64", &sysctl_with(set));
        let resolved = crate::run::resolve_on(
            &env(&[("DX_CACHE_DIR", cache_dir), ("DX_VERSION", "1.2.3")]),
            dir.path(),
            ("macos", arch),
        )
        .expect("resolves")
        .expect("downloads");
        assert_eq!(asset_name(resolved.platform), "dx-macos-arm64");
        assert!(resolved.bin.ends_with("dx-macos-arm64"));
    }
}

#[test]
fn intel_mac_is_still_unsupported() {
    for set in [
        &[][..],
        &[("hw.optional.arm64", "0"), ("sysctl.proc_translated", "0")][..],
    ] {
        let arch = effective_arch("macos", "x86_64", &sysctl_with(set));
        assert_eq!(platform("macos", arch), None);
    }
}
