//! Adopts the example in a scratch workspace with `dx init` and `dx generate`, then tests it.

use std::collections::BTreeMap;
use std::path::Path;

use dx_nested::compose::rules_dx_override;
use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, read_text, resolve_runfiles, rules_dx_source, write_text, Run};

fn ok(run: &Run, what: &str) {
    assert_eq!(run.status.code(), Some(0), "{what}: {}", run.combined());
}

/// Copies the example sources, without the BUILD files that `dx generate` writes.
fn copy_sources(from: &Path, to: &Path) {
    let entries = std::fs::read_dir(from).unwrap_or_else(|error| panic!("read {from:?}: {error}"));
    for entry in entries {
        let entry = entry.unwrap_or_else(|error| panic!("read {from:?}: {error}"));
        let name = entry.file_name();
        let path = entry.path();
        if path.is_dir() {
            copy_sources(&path, &to.join(&name));
        } else if name != "BUILD.bazel" && name != "README.md" {
            let target = to.join(&name);
            std::fs::create_dir_all(to).unwrap_or_else(|error| panic!("mkdir {to:?}: {error}"));
            std::fs::copy(&path, &target)
                .unwrap_or_else(|error| panic!("copy {path:?} to {target:?}: {error}"));
        }
    }
}

/// Every file of the workspace by relative path, leaving out the Bazel output links.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|error| panic!("read {dir:?}: {error}"));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("read {dir:?}: {error}"))
                .path();
            let name = path.file_name().map(|name| name.to_string_lossy());
            if name.is_some_and(|name| name.starts_with("bazel-")) {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let bytes =
                    std::fs::read(&path).unwrap_or_else(|error| panic!("read {path:?}: {error}"));
                files.insert(rel.to_string_lossy().into_owned(), bytes);
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(root, root, &mut files);
    files
}

fn dx(case: &Case, args: &[&str]) -> Run {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    let workspace = case.root().to_string_lossy().into_owned();
    let home = mkscratch("adopt-polyglot-home").unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = home.to_string_lossy().into_owned();
    let mut argv: Vec<&str> = vec![args[0], "--workspace", workspace.as_str()];
    argv.extend_from_slice(&args[1..]);
    case.run(
        &resolve_runfiles(&rel),
        &argv,
        &[("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")],
    )
}

#[test]
fn the_polyglot_example_adopts_and_tests_green() {
    let source = rules_dx_source();
    let case = nested::case("adopt_polyglot");
    let root = case.root().to_path_buf();
    let example = resolve_runfiles("examples/adopt-polyglot/Cargo.toml");
    copy_sources(example.parent().unwrap_or(&example), &root);
    write_text(
        &root.join(".bazelversion"),
        &read_text(&source.join(".bazelversion")),
    );
    write_text(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"adopt_polyglot\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n{}",
            rules_dx_override(&source)
        ),
    );

    ok(&dx(&case, &["init", "--apply"]), "dx init --apply");
    ok(&dx(&case, &["generate", "--apply"]), "dx generate --apply");
    ok(&dx(&case, &["test", "//..."]), "dx test //...");

    let before = snapshot(&root);
    ok(&dx(&case, &["init", "--apply"]), "second dx init --apply");
    ok(
        &dx(&case, &["generate", "--apply"]),
        "second dx generate --apply",
    );
    assert!(
        snapshot(&root) == before,
        "init and generate changed the adopted tree"
    );

    let module = read_text(&root.join("MODULE.bazel"));
    for call in ["crate.from_cargo", "uv.project", "npm_translate_lock"] {
        assert_eq!(
            module.matches(call).count(),
            1,
            "MODULE.bazel must hold one {call}:\n{module}"
        );
    }
}
