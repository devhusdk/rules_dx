//! Builds an AppDir, runs appimagetool and writes the identity file.

use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FIXED_ARGS: usize = 13;

#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    Lib(PathBuf),
    Notice(PathBuf),
    Data { rel: String, src: PathBuf },
}

#[derive(Debug)]
pub struct Request {
    pub tools: PathBuf,
    pub runtime: PathBuf,
    pub name: String,
    pub version: String,
    pub image: PathBuf,
    pub identity: PathBuf,
    pub binary: PathBuf,
    pub desktop: PathBuf,
    pub icon: PathBuf,
    pub arch: String,
    pub tool_version: String,
    pub runtime_version: String,
    pub squashfs_version: String,
    pub entries: Vec<Entry>,
}

pub fn parse(args: &[String]) -> Result<Request, String> {
    if args.len() < FIXED_ARGS {
        return Err(format!(
            "expected at least {FIXED_ARGS} arguments, got {}",
            args.len()
        ));
    }
    let mut entries = Vec::new();
    for entry in &args[FIXED_ARGS..] {
        entries.push(parse_entry(entry)?);
    }
    Ok(Request {
        tools: args[0].clone().into(),
        runtime: args[1].clone().into(),
        name: args[2].clone(),
        version: args[3].clone(),
        image: args[4].clone().into(),
        identity: args[5].clone().into(),
        binary: args[6].clone().into(),
        desktop: args[7].clone().into(),
        icon: args[8].clone().into(),
        arch: args[9].clone(),
        tool_version: args[10].clone(),
        runtime_version: args[11].clone(),
        squashfs_version: args[12].clone(),
        entries,
    })
}

fn parse_entry(entry: &str) -> Result<Entry, String> {
    if let Some(path) = entry.strip_prefix("lib:") {
        return Ok(Entry::Lib(path.into()));
    }
    if let Some(path) = entry.strip_prefix("notice:") {
        return Ok(Entry::Notice(path.into()));
    }
    if let Some(rest) = entry.strip_prefix("data:") {
        let (rel, src) = rest
            .split_once(':')
            .ok_or_else(|| format!("malformed data entry '{entry}'"))?;
        return Ok(Entry::Data {
            rel: rel.to_string(),
            src: src.into(),
        });
    }
    Err(format!("unknown entry '{entry}'"))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex(&hasher.finalize()))
}

pub fn highest_glibc(bytes: &[u8]) -> Option<Vec<u64>> {
    const MARK: &[u8] = b"GLIBC_";
    let mut best: Option<Vec<u64>> = None;
    let mut index = 0;
    while index + MARK.len() <= bytes.len() {
        if &bytes[index..index + MARK.len()] != MARK {
            index += 1;
            continue;
        }
        index += MARK.len();
        let start = index;
        while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b'.') {
            index += 1;
        }
        let text = String::from_utf8_lossy(&bytes[start..index]);
        let parts: Vec<u64> = text
            .trim_end_matches('.')
            .split('.')
            .map_while(|part| part.parse().ok())
            .collect();
        if parts.len() >= 2 && best.as_ref().is_none_or(|current| parts > *current) {
            best = Some(parts);
        }
    }
    best
}

pub fn interpreter(bytes: &[u8]) -> Option<String> {
    let mut found: Vec<String> = Vec::new();
    let mut index = 0;
    while let Some(offset) = find(&bytes[index..], b"/lib") {
        let start = index + offset;
        index = start + 4;
        let mut end = index;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if !bytes[end..].starts_with(b"/ld-linux") {
            continue;
        }
        end += b"/ld-linux".len();
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || b"-_.".contains(&bytes[end]))
        {
            end += 1;
        }
        found.push(String::from_utf8_lossy(&bytes[start..end]).into_owned());
        index = end;
    }
    found.sort();
    found.into_iter().next()
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn copy(src: &Path, dest: &Path) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    fs::copy(src, dest)
        .map(|_| ())
        .map_err(|e| format!("copy {} to {}: {e}", src.display(), dest.display()))?;
    let mode = fs::metadata(dest)
        .map_err(|e| format!("stat {}: {e}", dest.display()))?
        .permissions()
        .mode();
    let wanted = (mode | 0o644) & !0o022;
    fs::set_permissions(dest, fs::Permissions::from_mode(wanted))
        .map_err(|e| format!("chmod {}: {e}", dest.display()))
}

fn base(path: &Path) -> Result<&std::ffi::OsStr, String> {
    path.file_name()
        .ok_or_else(|| format!("{} has no file name", path.display()))
}

fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    for item in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
        let item = item.map_err(|e| format!("read {}: {e}", dir.display()))?;
        let kind = item
            .file_type()
            .map_err(|e| format!("stat {}: {e}", item.path().display()))?;
        if kind.is_dir() {
            walk(&item.path(), root, out)?;
        } else if kind.is_file() {
            let rel = item
                .path()
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned();
            out.insert(rel, item.path());
        }
    }
    Ok(())
}

pub fn pack(request: &Request) -> Result<(), String> {
    let work = PathBuf::from(format!("{}.work", request.image.display()));
    let _ = fs::remove_dir_all(&work);
    let result = pack_in(request, &work);
    let _ = fs::remove_dir_all(&work);
    result
}

fn pack_in(request: &Request, work: &Path) -> Result<(), String> {
    let name = &request.name;
    let app = work.join(format!("{name}.AppDir"));
    let share = app.join("usr/share").join(name);
    let licenses = app.join("usr/share/licenses").join(name);
    for dir in [
        app.join("usr/bin"),
        app.join("usr/lib"),
        share.clone(),
        licenses.clone(),
    ] {
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    }

    let exe = app.join("usr/bin").join(name);
    copy(&request.binary, &exe)?;
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("chmod {}: {e}", exe.display()))?;
    let desktop = app.join(format!("{name}.desktop"));
    copy(&request.desktop, &desktop)?;
    let extension = if request.icon.extension().is_some_and(|e| e == "svg") {
        "svg"
    } else {
        "png"
    };
    copy(&request.icon, &app.join(format!("{name}.{extension}")))?;
    std::os::unix::fs::symlink(format!("usr/bin/{name}"), app.join("AppRun"))
        .map_err(|e| format!("link AppRun: {e}"))?;

    let mut libs = Vec::new();
    for entry in &request.entries {
        match entry {
            Entry::Lib(src) => {
                let dest = app.join("usr/lib").join(base(src)?);
                copy(src, &dest)?;
                libs.push(dest);
            }
            Entry::Notice(src) => copy(src, &licenses.join(base(src)?))?,
            Entry::Data { rel, src } => copy(src, &share.join(rel))?,
        }
    }

    check_desktop(&desktop, name)?;
    run_tool(request, &app, &tool_path(request, work)?)?;

    let binary_bytes =
        fs::read(&request.binary).map_err(|e| format!("read {}: {e}", request.binary.display()))?;
    let mut glibc = highest_glibc(&binary_bytes);
    for lib in &libs {
        let bytes = fs::read(lib).map_err(|e| format!("read {}: {e}", lib.display()))?;
        glibc = glibc.max(highest_glibc(&bytes));
    }
    let min_glibc = glibc
        .map(|parts| {
            parts
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(".")
        })
        .unwrap_or_default();

    let mut files = BTreeMap::new();
    walk(&app, &app, &mut files)?;
    let mut digests = serde_json::Map::new();
    for (rel, path) in &files {
        digests.insert(rel.clone(), Value::String(hash_file(path)?));
    }
    let identity = json!({
        "name": name,
        "version": request.version,
        "arch": request.arch,
        "appimagetool": request.tool_version,
        "runtime": {
            "version": request.runtime_version,
            "sha256": hash_file(&request.runtime)?,
        },
        "mksquashfs": request.squashfs_version,
        "interpreter": interpreter(&binary_bytes).unwrap_or_default(),
        "min_glibc": min_glibc,
        "image_sha256": hash_file(&request.image)?,
        "files": Value::Object(digests),
    });
    let text = serde_json::to_string_pretty(&identity).map_err(|e| e.to_string())?;
    let temporary = PathBuf::from(format!("{}.tmp", request.identity.display()));
    fs::write(&temporary, text + "\n").map_err(|e| format!("write identity: {e}"))?;
    fs::rename(&temporary, &request.identity).map_err(|e| format!("write identity: {e}"))
}

fn check_desktop(desktop: &Path, name: &str) -> Result<(), String> {
    let text =
        fs::read_to_string(desktop).map_err(|e| format!("read {}: {e}", desktop.display()))?;
    let exec = format!("Exec={name}");
    let ok = text.lines().any(|line| {
        line.strip_prefix(&exec)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
    });
    if ok {
        Ok(())
    } else {
        Err(format!("{name}.desktop must have Exec={name}"))
    }
}

pub const FILE_COMMAND: &str = "file";

fn tool_path(request: &Request, work: &Path) -> Result<PathBuf, String> {
    let tools = std::path::absolute(&request.tools).map_err(|e| e.to_string())?;
    let dir = work.join("path");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let entries = fs::read_dir(&tools).map_err(|e| format!("read {}: {e}", tools.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(entry.path(), dir.join(entry.file_name()))
            .map_err(|e| format!("link {}: {e}", entry.path().display()))?;
    }
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    std::os::unix::fs::symlink(me, dir.join(FILE_COMMAND))
        .map_err(|e| format!("link {FILE_COMMAND}: {e}"))?;
    Ok(dir)
}

fn run_tool(request: &Request, app: &Path, path: &Path) -> Result<(), String> {
    let output = Command::new(path.join("appimagetool"))
        .arg("--runtime-file")
        .arg(&request.runtime)
        .arg("--no-appstream")
        .arg(app)
        .arg(&request.image)
        .env_clear()
        .env("PATH", path)
        .env("ARCH", &request.arch)
        .env("LC_ALL", "C")
        .env("SOURCE_DATE_EPOCH", "0")
        .output()
        .map_err(|e| format!("run appimagetool: {e}"))?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "appimagetool failed with {}\n{}{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glibc_floor_is_the_highest_version() {
        let bytes = b"x GLIBC_2.2.5\0GLIBC_2.34\0GLIBC_2.4\0GLIBC_PRIVATE\0";
        assert_eq!(highest_glibc(bytes), Some(vec![2, 34]));
        assert_eq!(highest_glibc(b"nothing"), None);
        assert_eq!(highest_glibc(b"GLIBC_2.17.1 ").unwrap(), vec![2, 17, 1]);
    }

    #[test]
    fn interpreter_names_the_loader_path() {
        assert_eq!(
            interpreter(b"\0/lib64/ld-linux-x86-64.so.2\0").as_deref(),
            Some("/lib64/ld-linux-x86-64.so.2")
        );
        assert_eq!(
            interpreter(b"\0/lib/ld-linux-aarch64.so.1\0/lib/libc.so").as_deref(),
            Some("/lib/ld-linux-aarch64.so.1")
        );
        assert_eq!(interpreter(b"/lib/libc.so.6"), None);
    }

    #[test]
    fn digest_matches_the_known_value() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn entries_parse_and_reject_unknown_kinds() {
        assert_eq!(
            parse_entry("data:a/b.txt:/src/b.txt"),
            Ok(Entry::Data {
                rel: "a/b.txt".into(),
                src: "/src/b.txt".into()
            })
        );
        assert_eq!(
            parse_entry("lib:/x/liby.so"),
            Ok(Entry::Lib("/x/liby.so".into()))
        );
        assert!(parse_entry("other:x").is_err());
        assert!(parse_entry("data:nocolon").is_err());
        assert!(parse(&[]).is_err());
    }

    #[test]
    fn desktop_exec_must_match_the_name() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("app.desktop");
        fs::write(&file, "[Desktop Entry]\nExec=app %U\n").unwrap();
        assert!(check_desktop(&file, "app").is_ok());
        assert!(check_desktop(&file, "ap").is_err());
        fs::write(&file, "[Desktop Entry]\nExec=application\n").unwrap();
        assert!(check_desktop(&file, "app").is_err());
    }

    #[test]
    fn copy_makes_files_readable_and_not_writable_by_others() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::write(&src, "x").unwrap();
        fs::set_permissions(&src, fs::Permissions::from_mode(0o400)).unwrap();
        let dest = dir.path().join("sub/dest");
        copy(&src, &dest).unwrap();
        let mode = fs::metadata(&dest).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }
}
