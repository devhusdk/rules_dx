//! Every Mach-O image of the macOS Python interpreter loads only system libraries and dylibs in the runfiles.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const REPO: &str = "python_interpreters+python_3_12_aarch64_apple_darwin";
const MACHO_MAGIC: [[u8; 4]; 3] = [
    [0xcf, 0xfa, 0xed, 0xfe],
    [0xfe, 0xed, 0xfa, 0xcf],
    [0xca, 0xfe, 0xba, 0xbe],
];
const MODULES: &str = "import ctypes, decimal, hashlib, lzma, readline, sqlite3, ssl, zlib";

fn runfiles() -> PathBuf {
    PathBuf::from(std::env::var_os("RUNFILES_DIR").expect("RUNFILES_DIR"))
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read runfiles").flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn is_macho(path: &Path) -> bool {
    let mut magic = [0u8; 4];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut magic))
        .is_ok_and(|()| MACHO_MAGIC.contains(&magic))
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir if out.file_name().is_some() => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn otool(flag: &str, image: &Path) -> String {
    let llvm_otool = std::env::var_os("LLVM_OTOOL").expect("LLVM_OTOOL");
    let done = Command::new(runfiles().join(llvm_otool))
        .arg(flag)
        .arg(image)
        .output()
        .expect("run otool");
    assert!(done.status.success(), "otool {flag} {}", image.display());
    String::from_utf8_lossy(&done.stdout).into_owned()
}

fn rpaths(image: &Path) -> Vec<String> {
    let text = otool("-l", image);
    let mut found = Vec::new();
    let mut want = false;
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            ["cmd", "LC_RPATH"] => want = true,
            ["path", path, ..] if want => {
                found.push((*path).to_owned());
                want = false;
            }
            _ => {}
        }
    }
    found
}

fn install_id(image: &Path) -> Option<String> {
    otool("-D", image)
        .lines()
        .find(|line| !line.ends_with(':'))
        .map(|line| line.trim().to_owned())
}

fn dependencies(image: &Path) -> BTreeSet<String> {
    otool("-L", image)
        .lines()
        .filter(|line| line.starts_with('\t'))
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_owned)
        .collect()
}

fn resolve(image: &Path, dep: &str, rpaths: &[String]) -> Option<PathBuf> {
    let dir = image.parent().expect("image directory");
    let candidate = if let Some(rest) = dep.strip_prefix("@rpath/") {
        rpaths.iter().find_map(|rpath| {
            let base = match rpath
                .strip_prefix("@loader_path")
                .or_else(|| rpath.strip_prefix("@executable_path"))
            {
                Some(rest) => dir.join(rest.trim_start_matches('/')),
                None => PathBuf::from(rpath),
            };
            let path = base.join(rest);
            path.exists().then_some(path)
        })
    } else {
        dep.strip_prefix("@loader_path/")
            .or_else(|| dep.strip_prefix("@executable_path/"))
            .map(|rest| dir.join(rest))
            .filter(|path| path.exists())
    };
    candidate.map(|path| normalize(&path))
}

fn interpreter_images() -> Vec<PathBuf> {
    let mut files = Vec::new();
    walk(&runfiles(), &mut files);
    let images: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| path.to_string_lossy().contains(REPO) && is_macho(path))
        .collect();
    assert!(!images.is_empty(), "no Mach-O image of {REPO} in the runfiles");
    images
}

#[test]
fn interpreter_dylibs_are_runfiles_inputs() {
    let mut queue = interpreter_images();
    let mut seen = BTreeSet::new();
    let mut problems = Vec::new();
    while let Some(image) = queue.pop() {
        if !seen.insert(image.clone()) {
            continue;
        }
        let id = install_id(&image);
        let paths = rpaths(&image);
        for dep in dependencies(&image) {
            if dep.starts_with("/usr/lib/")
                || dep.starts_with("/System/")
                || id.as_ref() == Some(&dep)
            {
                continue;
            }
            match resolve(&image, &dep, &paths) {
                Some(resolved) => queue.push(resolved),
                None => problems.push(format!(
                    "{} loads {dep}, which is not in its runfiles",
                    image.display()
                )),
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn interpreter_imports_native_modules() {
    let python = interpreter_images()
        .into_iter()
        .find(|path| path.ends_with("bin/python3.12"))
        .expect("bin/python3.12 is not in the runfiles");
    let done = Command::new(python)
        .args(["-I", "-c", MODULES])
        .output()
        .expect("run python");
    assert!(
        done.status.success(),
        "{}",
        String::from_utf8_lossy(&done.stderr)
    );
}
