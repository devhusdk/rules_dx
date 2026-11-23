use std::collections::BTreeSet;
use std::path::Path;

use dx_module_bazel::ModuleFile;

const SKIPPED_DIRS: &[&str] = &["node_modules", "target", ".git"];

const UNLISTED_OK_DIRS: &[&str] = &["testdata", "fixtures", "examples"];

fn walk(dir: &Path, root: &Path, found: &mut BTreeSet<String>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read {}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("entry in {}: {error}", dir.display()));
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let kind = entry
            .file_type()
            .unwrap_or_else(|error| panic!("type of {}: {error}", path.display()));
        if kind.is_dir() {
            if name.starts_with("bazel-") || SKIPPED_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(&path, root, found);
        } else if name == "Cargo.toml" {
            let rel = path.strip_prefix(root).expect("under root");
            found.insert(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn declared(source: &str) -> BTreeSet<String> {
    let module = ModuleFile::parse(source).expect("MODULE.bazel parses");
    module
        .cargo_hubs()
        .expect("literal cargo hubs")
        .iter()
        .flat_map(|hub| hub.manifests.iter())
        .map(|label| {
            let label = label.value.strip_prefix("//").expect("root label");
            label.replacen(':', "/", 1)
        })
        .collect()
}

#[test]
fn module_manifests_match_cargo_tomls_on_disk() {
    let root = dx_testing::rules_dx_source().unwrap();
    let mut on_disk = BTreeSet::new();
    walk(&root, &root, &mut on_disk);
    let in_module = declared(&dx_testing::read_runfiles("MODULE.bazel"));
    let unlisted: Vec<_> = on_disk
        .difference(&in_module)
        .filter(|path| !path.split('/').any(|part| UNLISTED_OK_DIRS.contains(&part)))
        .collect();
    let missing: Vec<_> = in_module.difference(&on_disk).collect();
    assert!(
        unlisted.is_empty() && missing.is_empty(),
        "Cargo.toml not in MODULE.bazel manifests: {unlisted:?}; manifests with no file: {missing:?}"
    );
}

#[test]
fn declared_reads_every_hub() {
    let source = "crate = use_extension(\"@rules_rust//crate_universe:extensions.bzl\", \"crate\")\ncrate.from_cargo(name = \"a\", manifests = [\"//x:Cargo.toml\", \"//y/z:Cargo.toml\"])\n";
    let want: BTreeSet<String> = ["x/Cargo.toml", "y/z/Cargo.toml"]
        .map(String::from)
        .into_iter()
        .collect();
    assert_eq!(declared(source), want);
}
