use std::path::PathBuf;

#[test]
fn consumer_qualifies_without_installs_or_network() {
    let rel =
        std::env::var("DX_HERMETIC_MANIFEST").expect("DX_HERMETIC_MANIFEST names the manifest");
    let manifest_path = dx_testing::resolve_runfiles(&rel);
    let text = std::fs::read_to_string(&manifest_path).expect("manifest is readable");
    let base = std::env::current_dir().expect("runfiles cwd");
    let manifest = hermetic::parse_manifest(&text, &base).expect("manifest parses");
    let tmp = PathBuf::from(std::env::var("TEST_TMPDIR").expect("TEST_TMPDIR is set under Bazel"));
    let root = tmp.join("qualification");
    let out = std::env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| tmp.join("outputs"));
    hermetic::remove_root(&root);
    std::fs::create_dir_all(&out).expect("outputs dir");
    let evidence = hermetic::qualify(manifest, &root, &out);
    dx_testing::write_json(&out.join("evidence.json"), &evidence).expect("evidence written");
    println!("{evidence:#}");
    if std::env::var_os("DX_HERMETIC_KEEP").is_none() {
        hermetic::remove_root(&root);
    }
    assert_eq!(evidence["result"], "qualified", "{evidence:#}");
}
