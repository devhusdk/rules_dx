//! Checks the files and the manifest of the signed Flutter fixture APK.

use std::io::Read;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

fn signed_apk() -> String {
    let paths = env("DX_APK");
    paths
        .split_whitespace()
        .find(|path| path.ends_with(".apk") && !path.ends_with("_unsigned.apk"))
        .unwrap_or_else(|| panic!("no signed apk in: {paths}"))
        .to_owned()
}

fn archive() -> zip::ZipArchive<std::fs::File> {
    let path = signed_apk();
    let file = std::fs::File::open(&path).unwrap_or_else(|error| panic!("open {path}: {error}"));
    zip::ZipArchive::new(file).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

fn names() -> Vec<String> {
    archive()
        .file_names()
        .map(|name| name.expect("entry name").into_owned())
        .collect()
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

#[test]
fn apk_packages_the_engine_the_app_and_its_assets() {
    let abi = env("DX_APK_ABI");
    let names = names();
    for want in [
        format!("lib/{abi}/libflutter.so"),
        format!("lib/{abi}/libapp.so"),
        "classes.dex".to_owned(),
        "assets/flutter_assets/packages/flutter_app/assets/items.json".to_owned(),
        "assets/flutter_assets/packages/flutter_app/assets/fonts/DxSquare.ttf".to_owned(),
    ] {
        assert!(names.contains(&want), "apk misses {want}: {names:?}");
    }
}

#[test]
fn apk_carries_only_the_requested_abi() {
    let prefix = format!("lib/{}/", env("DX_APK_ABI"));
    let other: Vec<String> = names()
        .into_iter()
        .filter(|name| name.starts_with("lib/") && !name.starts_with(&prefix))
        .collect();
    assert!(other.is_empty(), "apk has other ABIs: {other:?}");
}

#[test]
fn manifest_names_the_package_and_the_activity() {
    let mut manifest = Vec::new();
    archive()
        .by_name("AndroidManifest.xml")
        .expect("AndroidManifest.xml")
        .read_to_end(&mut manifest)
        .expect("read AndroidManifest.xml");
    manifest.retain(|byte| *byte != 0);
    for want in [env("DX_APK_APP_ID"), env("DX_APK_ACTIVITY")] {
        assert!(contains(&manifest, &want), "manifest misses {want}");
    }
}
