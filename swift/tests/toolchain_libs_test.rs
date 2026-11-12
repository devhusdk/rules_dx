//! Every shared library a Swift toolchain binary needs resolves inside the toolchain's RUNPATH
//! directories; only the glibc runtime comes from the host. Skipped: lldb and repl_swift.

use std::collections::{BTreeSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};

use dx_testing::{env_file, run};

const HOST_LIBS: &[&str] = &[
    "libc.so.6",
    "libm.so.6",
    "libdl.so.2",
    "libpthread.so.0",
    "librt.so.1",
    "libutil.so.1",
    "libresolv.so.2",
];

const RUNPATH: &[&str] = &[
    "usr/lib",
    "usr/lib/swift/linux",
    "usr/lib/swift/host",
    "usr/lib/swift/host/compiler",
];

fn host_lib(name: &str) -> bool {
    name.starts_with("ld-linux-") || HOST_LIBS.contains(&name)
}

fn is_elf(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok()
        && magic == *b"\x7fELF"
}

fn needed(objdump: &Path, binary: &Path) -> Vec<String> {
    let path = binary.to_string_lossy();
    let out = run(objdump, &["-p", &path], &[]).expect("llvm-objdump runs");
    assert!(out.status.success(), "{path}: {}", out.combined());
    out.stdout
        .lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            (words.next() == Some("NEEDED")).then(|| words.next().map(str::to_owned))?
        })
        .collect()
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

#[test]
fn toolchain_binaries_need_only_toolchain_or_glibc_libraries() {
    let objdump = env_file("LLVM_OBJDUMP");
    let toolchain = objdump
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("llvm-objdump sits in usr/bin");
    let dirs: Vec<PathBuf> = RUNPATH.iter().map(|dir| toolchain.join(dir)).collect();

    let mut queue = VecDeque::new();
    let bin = toolchain.join("usr/bin");
    for entry in std::fs::read_dir(&bin).unwrap_or_else(|error| panic!("{}: {error}", bin.display())) {
        let path = entry.expect("usr/bin entry").path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        if name.starts_with("lldb") || name == "repl_swift" || !path.is_file() {
            continue;
        }
        let path = canonical(&path);
        if is_elf(&path) {
            queue.push_back(path);
        }
    }

    let mut seen = BTreeSet::new();
    let mut failures = Vec::new();
    while let Some(binary) = queue.pop_front() {
        if !seen.insert(binary.clone()) {
            continue;
        }
        for lib in needed(&objdump, &binary) {
            if host_lib(&lib) {
                continue;
            }
            match dirs.iter().map(|dir| dir.join(&lib)).find(|path| path.exists()) {
                Some(path) => queue.push_back(canonical(&path)),
                None => failures.push(format!(
                    "{} needs {lib}, which is outside the toolchain",
                    binary.file_name().unwrap_or_default().to_string_lossy()
                )),
            }
        }
    }

    assert!(!seen.is_empty(), "no toolchain binaries found under {}", bin.display());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
