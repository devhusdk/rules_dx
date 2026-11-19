//! The source-built libjulia-internal stands in for the one Julia 1.12.5 ships.

use object::elf;
use object::read::elf::{Dyn, ElfFile64, FileHeader};
use object::{Object, ObjectSymbol};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const LIBRARY: &str = "libjulia-internal.so.1.12.5";

fn julia() -> PathBuf {
    let rel = std::env::var("DX_JULIA").expect("DX_JULIA must be set");
    dx_testing::resolve_runfiles(&rel)
}

/// The Julia tree root: the directory above the `bin/` holding `julia`.
fn julia_root() -> PathBuf {
    let exe = std::fs::canonicalize(julia()).expect("resolve julia");
    exe.parent()
        .and_then(Path::parent)
        .expect("julia lives in <root>/bin")
        .to_path_buf()
}

fn built() -> PathBuf {
    let files = std::env::var("DX_LIBJULIA_INTERNAL").expect("DX_LIBJULIA_INTERNAL must be set");
    let rel = files
        .split_whitespace()
        .find(|path| path.ends_with(LIBRARY))
        .expect("built library in DX_LIBJULIA_INTERNAL");
    dx_testing::resolve_runfiles(rel)
}

fn shipped() -> PathBuf {
    julia_root().join("lib/julia").join(LIBRARY)
}

struct Dynamic {
    soname: Option<String>,
    needed: BTreeSet<String>,
    exports: BTreeSet<String>,
}

fn dynamic(path: &Path) -> Dynamic {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let file = ElfFile64::<object::Endianness>::parse(&*data).expect("parse ELF");
    let endian = file.endian();
    let sections = file
        .elf_header()
        .sections(endian, &*data)
        .expect("section table");
    let mut soname = None;
    let mut needed = BTreeSet::new();
    if let Some((entries, link)) = sections.dynamic(endian, &*data).expect("dynamic section") {
        let strings = sections.strings(endian, &*data, link).expect("dynamic strings");
        for entry in entries {
            let tag = entry.tag32(endian);
            if tag != Some(elf::DT_NEEDED) && tag != Some(elf::DT_SONAME) {
                continue;
            }
            let name = entry.string(endian, strings).expect("dynamic string");
            let name = String::from_utf8_lossy(name).into_owned();
            if tag == Some(elf::DT_SONAME) {
                soname = Some(name);
            } else {
                needed.insert(name);
            }
        }
    }
    let exports = file
        .dynamic_symbols()
        .filter(|symbol| !symbol.is_undefined() && symbol.is_global())
        .map(|symbol| symbol.name().expect("symbol name").to_owned())
        .collect();
    Dynamic {
        soname,
        needed,
        exports,
    }
}

#[test]
fn links_like_the_shipped_library() {
    let ours = dynamic(&built());
    let theirs = dynamic(&shipped());
    assert_eq!(ours.soname, theirs.soname);
    assert_eq!(ours.needed, theirs.needed);
    let missing: Vec<_> = theirs.exports.difference(&ours.exports).collect();
    let extra: Vec<_> = ours.exports.difference(&theirs.exports).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "exported symbols differ: missing {missing:?}, extra {extra:?}"
    );
}

#[test]
fn julia_starts_on_the_built_library() {
    let root = julia_root();
    let scratch = dx_test_scratch::scratch("libjulia_internal").expect("scratch");
    let tree = scratch.path().join("julia");
    let copy = |from: &Path, to: &Path| {
        std::fs::create_dir_all(to.parent().expect("parent")).expect("mkdir");
        std::fs::copy(from, to).unwrap_or_else(|e| panic!("copy {}: {e}", from.display()));
    };
    for entry in std::fs::read_dir(root.join("bin")).expect("read bin") {
        let entry = entry.expect("bin entry");
        copy(&entry.path(), &tree.join("bin").join(entry.file_name()));
    }
    for entry in std::fs::read_dir(root.join("lib")).expect("read lib") {
        let entry = entry.expect("lib entry");
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("libjulia.so") {
            copy(&entry.path(), &tree.join("lib").join(&name));
        }
    }
    let private = tree.join("lib/julia");
    std::fs::create_dir_all(&private).expect("mkdir lib/julia");
    copy(&built(), &private.join(LIBRARY));
    for alias in ["libjulia-internal.so", "libjulia-internal.so.1.12"] {
        std::os::unix::fs::symlink(LIBRARY, private.join(alias)).expect("link library alias");
    }
    for entry in std::fs::read_dir(root.join("lib/julia")).expect("read lib/julia") {
        let entry = entry.expect("lib/julia entry");
        let target = private.join(entry.file_name());
        if !target.exists() {
            std::os::unix::fs::symlink(entry.path(), target).expect("link lib/julia entry");
        }
    }
    for entry in std::fs::read_dir(&root).expect("read root") {
        let entry = entry.expect("root entry");
        let target = tree.join(entry.file_name());
        if !target.exists() {
            std::os::unix::fs::symlink(entry.path(), target).expect("link root entry");
        }
    }
    let run = dx_testing::run(
        &tree.join("bin/julia"),
        &[
            "--startup-file=no",
            "-e",
            "using Libdl; println(1 + 1); foreach(println, filter(p -> occursin(\"libjulia-internal\", p), Libdl.dllist()))",
        ],
        &[],
    )
    .expect("spawn julia");
    assert!(run.status.success(), "julia failed: {}", run.stderr);
    let mut lines = run.stdout.lines();
    assert_eq!(lines.next(), Some("2"), "stdout: {}", run.stdout);
    let loaded: Vec<_> = lines.collect();
    let expected = std::fs::canonicalize(private.join(LIBRARY)).expect("resolve scratch library");
    assert!(
        loaded
            .iter()
            .any(|path| std::fs::canonicalize(path).ok().as_deref() == Some(expected.as_path())),
        "loaded {loaded:?}, expected {}",
        expected.display()
    );
}
