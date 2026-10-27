use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use dx_process::lifecycle::{
    run_within_until, CapturePolicy, ChildOutcome, CleanupBudget, Descendants, EnvPolicy, SpawnSpec,
};

/// One running Bazel server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    pub pid: u32,
    pub rss_bytes: u64,
    pub workspace: Option<PathBuf>,
    pub output_base: Option<PathBuf>,
    pub output_user_root: Option<PathBuf>,
}

/// One Bazel output base and the workspace it was made for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
}

pub(crate) use dx_process::DiskSpace;

/// The machine facts the pool reads and the effects it has outside git.
pub trait Host {
    fn now(&self) -> u64;
    fn process_alive(&self, pid: u32) -> bool;
    fn disk_space(&self, path: &Path) -> Option<DiskSpace>;
    fn servers(&self) -> Vec<Server>;
    fn output_roots(&self) -> Vec<PathBuf>;
    /// Runs `bazel shutdown` in workspace; callers decide whether a server runs there.
    fn bazel_shutdown(&self, workspace: &Path);
    fn disk_bytes(&self, path: &Path) -> u64;
    fn disk_cache(&self) -> Option<PathBuf>;
    /// Where nested-Bazel tests keep their slots, if this host has such a root.
    fn nested_root(&self) -> Option<PathBuf>;
    fn interrupted(&self) -> Option<i32>;
    /// Deletes a directory tree, including read-only entries; true when it is gone afterwards.
    #[must_use]
    fn remove_tree(&self, path: &Path) -> bool;
}

/// Compares two paths the way Bazel prints them.
pub fn same_path(left: &Path, right: &Path) -> bool {
    normalize(&resolved(left)) == normalize(&resolved(right))
}

/// Resolves symlinks in the deepest existing ancestor and keeps the missing tail.
pub(crate) fn resolved(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(real) = std::fs::canonicalize(ancestor) {
            let tail = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return if tail.as_os_str().is_empty() {
                real
            } else {
                real.join(tail)
            };
        }
    }
    path.to_path_buf()
}

fn normalize(path: &Path) -> String {
    let text = dx_path::posix(path);
    let text = if let Some(rest) = text.strip_prefix("//?/UNC/") {
        format!("//{rest}")
    } else {
        text.strip_prefix("//?/").unwrap_or(&text).to_owned()
    };
    let text = text.trim_end_matches('/').to_owned();
    if cfg!(windows) {
        text.to_lowercase()
    } else {
        text
    }
}

fn flag_value(args: &str, flag: &str) -> Option<PathBuf> {
    let start = args.find(&format!(" {flag}="))? + flag.len() + 2;
    let rest = &args[start..];
    let end = rest.find(" --").unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        None
    } else {
        Some(PathBuf::from(value))
    }
}

/// Reads Bazel servers from `pid rss-kib args` lines.
pub(crate) fn parse_servers(text: &str) -> Vec<Server> {
    let mut servers = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let mut parts = line.splitn(3, char::is_whitespace);
        let (Some(pid), Some(rss), Some(args)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        let args = format!(" {}", args.trim_start());
        if !args.contains("A-server.jar") {
            continue;
        }
        let (Ok(pid), Ok(rss)) = (pid.parse::<u32>(), rss.parse::<u64>()) else {
            continue;
        };
        servers.push(Server {
            pid,
            rss_bytes: rss * 1024,
            workspace: flag_value(&args, "--workspace_directory"),
            output_base: flag_value(&args, "--output_base"),
            output_user_root: flag_value(&args, "--output_user_root"),
        });
    }
    servers
}

/// Finds the output bases under each root that record a workspace path.
pub(crate) fn scan_output_bases(roots: &[PathBuf]) -> Vec<OutputBase> {
    let mut bases: Vec<OutputBase> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            let Ok(text) = std::fs::read_to_string(dir.join("DO_NOT_BUILD_HERE")) else {
                continue;
            };
            let workspace = text.trim();
            if workspace.is_empty() || bases.iter().any(|base| base.dir == dir) {
                continue;
            }
            bases.push(OutputBase {
                dir,
                workspace: PathBuf::from(workspace),
            });
        }
    }
    bases.sort_by(|left, right| left.dir.cmp(&right.dir));
    bases
}

/// One entry of Bazel's repository cache download store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub dir: PathBuf,
    pub cache: PathBuf,
    pub used: u64,
    pub bytes: u64,
}

/// Where Bazel keeps downloads under an output root when `--repository_cache` is not set.
pub(crate) fn download_store(root: &Path) -> PathBuf {
    root.join("cache")
        .join("repos")
        .join("v1")
        .join("content_addressable")
}

/// Newest file modified time in a download entry, in epoch seconds.
pub(crate) fn download_used(dir: &Path) -> Option<u64> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut newest = None;
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        newest = newest.max(Some(seconds(&meta)));
    }
    newest
}

fn seconds(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Finds the download entries in each root's default repository cache, `<algo>/<key>/` under `content_addressable`.
pub(crate) fn scan_downloads(roots: &[PathBuf], stop: &dyn Fn() -> Option<i32>) -> Vec<Download> {
    let mut downloads: Vec<Download> = Vec::new();
    let mut seen: Vec<PathBuf> = Vec::new();
    for root in roots {
        let cache = download_store(root);
        if seen.contains(&cache) {
            continue;
        }
        seen.push(cache.clone());
        let Ok(algos) = std::fs::read_dir(&cache) else {
            continue;
        };
        for algo in algos.flatten() {
            if !algo.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let Ok(keys) = std::fs::read_dir(algo.path()) else {
                continue;
            };
            for key in keys.flatten() {
                if stop().is_some() {
                    return downloads;
                }
                if !key.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                let dir = key.path();
                let Some(used) = download_used(&dir) else {
                    continue;
                };
                downloads.push(Download {
                    bytes: walk_bytes(&dir, stop),
                    dir,
                    cache: cache.clone(),
                    used,
                });
            }
        }
    }
    downloads.sort_by(|left, right| left.dir.cmp(&right.dir));
    downloads.dedup_by(|left, right| left.dir == right.dir);
    downloads
}

fn walk_bytes(path: &Path, stop: &dyn Fn() -> Option<i32>) -> u64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        if stop().is_some() {
            break;
        }
        total += walk_bytes(&entry.path(), stop);
    }
    total
}

fn du_bytes(path: &Path, stop: &dyn Fn() -> Option<i32>) -> Option<u64> {
    let spec = SpawnSpec {
        argv: vec!["du".into(), "-sk".into(), path.as_os_str().to_owned()],
        cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
        env: EnvPolicy::Inherited { extra: Vec::new() },
        capture: CapturePolicy { max_bytes: 1 << 20 },
        timeout: Duration::from_secs(3600),
        descendants: Descendants::Owned,
    };
    let ChildOutcome::Finished { stdout, .. } =
        run_within_until(&spec, CleanupBudget::STANDARD, stop).ok()?
    else {
        return None;
    };
    String::from_utf8_lossy(&stdout)
        .split_whitespace()
        .next()
        .and_then(|first| first.parse::<u64>().ok())
        .map(|kib| kib * 1024)
}

/// The machine this process runs on.
pub struct SystemHost;

#[cfg(unix)]
fn pid_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    let rc = unsafe { libc::kill(pid, 0) };
    rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(not(unix))]
fn pid_alive(pid: u32) -> bool {
    dx_process::probe(
        "tasklist",
        ["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"],
    )
    .is_some_and(|text| text.contains(&format!("\"{pid}\"")))
}

#[cfg(unix)]
fn list_servers() -> String {
    dx_process::probe("ps", ["-eo", "pid=,rss=,args="]).unwrap_or_default()
}

#[cfg(not(unix))]
fn list_servers() -> String {
    let script = "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*A-server.jar*' } | ForEach-Object { \"$($_.ProcessId) $([int]($_.WorkingSetSize/1024)) $($_.CommandLine)\" }";
    dx_process::probe(
        "powershell",
        ["-NoProfile", "-NonInteractive", "-Command", script],
    )
    .unwrap_or_default()
}

fn home(env_get: &dyn Fn(&str) -> Option<String>) -> Option<PathBuf> {
    env_get("HOME")
        .or_else(|| env_get("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn user_name(env_get: &dyn Fn(&str) -> Option<String>) -> String {
    env_get("USER")
        .or_else(|| env_get("USERNAME"))
        .unwrap_or_default()
}

/// Where Bazel keeps output bases when nothing moves them.
pub fn default_output_roots(env_get: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let user = user_name(env_get);
    let mut roots = Vec::new();
    if let Some(cache) = dx_process::user_cache_dir(env_get) {
        roots.push(cache.join("bazel").join(format!("_bazel_{user}")));
    }
    if let Some(home) = home(env_get) {
        roots.push(home.join(format!("_bazel_{user}")));
    }
    roots.push(PathBuf::from(format!("/private/var/tmp/_bazel_{user}")));
    roots.push(PathBuf::from(format!("/var/tmp/_bazel_{user}")));
    roots
}

fn find_bazel() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["bazel", "bazelisk", "bazel.exe", "bazelisk.exe"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn make_writable(path: &Path) {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        return;
    }
    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        permissions.set_mode(permissions.mode() | 0o700);
    }
    #[cfg(not(unix))]
    {
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
    }
    let _ = std::fs::set_permissions(path, permissions);
    if metadata.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                make_writable(&entry.path());
            }
        }
    }
}

/// Removes a directory tree, including read-only entries, and reports whether it is gone.
pub(crate) fn remove_tree(path: &Path) -> bool {
    if std::fs::remove_dir_all(path).is_err() {
        make_writable(path);
        let _ = std::fs::remove_dir_all(path);
    }
    !path.exists()
}

impl Host for SystemHost {
    fn remove_tree(&self, path: &Path) -> bool {
        remove_tree(path)
    }

    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0)
    }

    fn process_alive(&self, pid: u32) -> bool {
        pid_alive(pid)
    }

    fn disk_space(&self, path: &Path) -> Option<DiskSpace> {
        dx_process::disk_space(path)
    }

    fn servers(&self) -> Vec<Server> {
        parse_servers(&list_servers())
    }

    fn output_roots(&self) -> Vec<PathBuf> {
        let mut roots = default_output_roots(&|name| std::env::var(name).ok());
        for server in self.servers() {
            if let Some(root) = server.output_user_root {
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
        roots
    }

    fn bazel_shutdown(&self, workspace: &Path) {
        let Some(bazel) = find_bazel() else {
            return;
        };
        let _ = Command::new(bazel)
            .args(dx_process::current().startup_opts())
            .arg("shutdown")
            .current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    fn disk_bytes(&self, path: &Path) -> u64 {
        let stop: &dyn Fn() -> Option<i32> = &dx_process::interrupted;
        if cfg!(unix) {
            if let Some(bytes) = du_bytes(path, stop) {
                return bytes;
            }
        }
        if stop().is_some() {
            return 0;
        }
        walk_bytes(path, stop)
    }

    fn disk_cache(&self) -> Option<PathBuf> {
        dx_process::current().bazel_disk_cache.clone()
    }

    fn nested_root(&self) -> Option<PathBuf> {
        let env = std::env::var_os(dx_path::nested_bazel::ROOT_ENV);
        let profile = std::env::var_os("USERPROFILE").map(PathBuf::from);
        Some(dx_path::nested_bazel::root(
            env.as_deref(),
            current_uid(),
            profile.as_deref(),
        ))
    }

    fn interrupted(&self) -> Option<i32> {
        dx_process::interrupted()
    }
}

#[cfg(unix)]
fn current_uid() -> Option<u32> {
    Some(unsafe { libc::getuid() })
}

#[cfg(not(unix))]
fn current_uid() -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn remove_tree_deletes_read_only_trees() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = tempfile::tempdir().expect("tempdir");
        let build = |name: &str| {
            let dir = scratch.path().join(name).join("ro");
            std::fs::create_dir_all(&dir).expect("dir");
            let file = dir.join("f");
            std::fs::write(&file, "x").expect("file");
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).expect("file");
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).expect("dir");
            scratch.path().join(name)
        };
        let plain = build("plain");
        let tree = build("tree");
        let blocked = std::fs::remove_dir_all(&plain).is_err();
        if !blocked {
            // Running as root: permission bits are ignored, so only the result is checked.
            assert!(remove_tree(&tree));
            assert!(!tree.exists());
            return;
        }
        assert!(remove_tree(&tree));
        assert!(!tree.exists());
        make_writable(&plain);
    }

    #[test]
    fn default_output_roots_follow_the_user_cache_dir() {
        let roots = |pairs: &'static [(&'static str, &'static str)]| {
            default_output_roots(&|name| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            })
        };
        let paths = |items: &[&str]| items.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(
            roots(&[("USER", "u"), ("XDG_CACHE_HOME", "/x"), ("HOME", "/h")]),
            paths(&[
                "/x/bazel/_bazel_u",
                "/h/_bazel_u",
                "/private/var/tmp/_bazel_u",
                "/var/tmp/_bazel_u",
            ])
        );
        assert_eq!(
            roots(&[("USER", "u"), ("HOME", "/h")])[0],
            PathBuf::from("/h/.cache/bazel/_bazel_u")
        );
        assert_eq!(
            roots(&[("USER", "u"), ("XDG_CACHE_HOME", "/x")]),
            paths(&[
                "/x/bazel/_bazel_u",
                "/private/var/tmp/_bazel_u",
                "/var/tmp/_bazel_u",
            ])
        );
        assert_eq!(
            roots(&[("USER", "u")]),
            paths(&["/private/var/tmp/_bazel_u", "/var/tmp/_bazel_u"])
        );
    }

    #[test]
    fn servers_are_read_from_ps_lines_with_paths_that_hold_spaces() {
        let text = "  10 2048 /usr/bin/other --workspace_directory=/x\n\
 4242 1024 bazel(app) -Xmx4g -jar /i/A-server.jar --max_idle_secs=300 --output_user_root=/home/u/.cache/bazel/_bazel_u --output_base=/home/u/.cache/bazel/_bazel_u/abc --workspace_directory=/home/u/my repo/slot --default_system_javabase=/jdk\n\
 77 notanumber bazel(app) A-server.jar\n";
        let servers = parse_servers(text);
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].pid, 4242);
        assert_eq!(servers[0].rss_bytes, 1024 * 1024);
        assert_eq!(
            servers[0].workspace,
            Some(PathBuf::from("/home/u/my repo/slot"))
        );
        assert_eq!(
            servers[0].output_base,
            Some(PathBuf::from("/home/u/.cache/bazel/_bazel_u/abc"))
        );
        assert_eq!(
            servers[0].output_user_root,
            Some(PathBuf::from("/home/u/.cache/bazel/_bazel_u"))
        );
    }

    #[test]
    fn paths_compare_across_separators_and_trailing_slashes() {
        assert!(same_path(Path::new("/a/b/"), Path::new("/a/b")));
        assert!(same_path(Path::new("C:\\a\\b"), Path::new("C:/a/b")));
        assert!(!same_path(Path::new("/a/b"), Path::new("/a/c")));
        assert!(same_path(Path::new("\\\\?\\C:\\a\\b"), Path::new("C:/a/b")));
        assert_eq!(
            normalize(Path::new("\\\\?\\UNC\\srv\\s\\x")),
            normalize(Path::new("//srv/s/x"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn paths_compare_through_symlinks_even_when_the_tail_is_gone() {
        let scratch = dx_test_scratch::scratch("dx-wt-links-");
        let real = scratch.path().join("real");
        let link = scratch.path().join("link");
        std::fs::create_dir_all(real.join("x")).expect("real");
        std::os::unix::fs::symlink(&real, &link).expect("link");
        assert!(same_path(&link.join("x"), &real.join("x")));
        std::fs::remove_dir(real.join("x")).expect("remove");
        assert!(same_path(&link.join("x"), &real.join("x")));
        assert!(same_path(&link.join("x").join("y"), &real.join("x/y")));
        assert!(!same_path(&link.join("x"), &real.join("y")));
        assert!(same_path(Path::new("/gone/a/b"), Path::new("/gone/a/b/")));
        assert!(!same_path(Path::new("/gone/a/b"), Path::new("/gone/a/c")));
    }

    #[test]
    fn output_bases_are_found_by_their_workspace_marker() {
        let scratch = dx_test_scratch::scratch("dx-wt-bases-");
        let root = scratch.path().join("root");
        std::fs::create_dir_all(root.join("install")).expect("install");
        std::fs::create_dir_all(root.join("aaa")).expect("base");
        std::fs::write(root.join("aaa").join("DO_NOT_BUILD_HERE"), "/gone/ws\n").expect("marker");
        let bases = scan_output_bases(&[root.clone(), root.clone(), scratch.path().join("none")]);
        assert_eq!(
            bases,
            vec![OutputBase {
                dir: root.join("aaa"),
                workspace: PathBuf::from("/gone/ws"),
            }]
        );
    }

    #[test]
    fn walked_size_counts_files_and_skips_symlink_targets() {
        let scratch = dx_test_scratch::scratch("dx-wt-size-");
        let dir = scratch.path().join("d");
        std::fs::create_dir_all(dir.join("sub")).expect("dirs");
        std::fs::write(dir.join("a"), vec![0u8; 100]).expect("a");
        std::fs::write(dir.join("sub").join("b"), vec![0u8; 50]).expect("b");
        assert!(walk_bytes(&dir, &|| None) >= 150);
        assert_eq!(walk_bytes(&dir.join("missing"), &|| None), 0);
    }

    #[test]
    fn walk_bytes_returns_early_when_stopped() {
        let scratch = dx_test_scratch::scratch("dx-wt-stop-walk-");
        let dir = scratch.path().join("d");
        for sub in ["one", "two"] {
            std::fs::create_dir_all(dir.join(sub)).expect("dir");
            std::fs::write(dir.join(sub).join("f"), vec![0u8; 100]).expect("file");
        }
        let calls = std::cell::Cell::new(0);
        let stop = || {
            calls.set(calls.get() + 1);
            (calls.get() > 1).then_some(15)
        };
        let full = walk_bytes(&dir, &|| None);
        assert_eq!(full, 200);
        assert!(walk_bytes(&dir, &stop) < full);
    }

    fn aged(path: &Path, secs: u64) {
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open");
        file.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(secs))
            .expect("set time");
    }

    fn download(root: &Path, key: &str, files: &[(&str, u64)]) -> PathBuf {
        let dir = download_store(root).join("sha256").join(key);
        std::fs::create_dir_all(&dir).expect("entry");
        for (name, secs) in files {
            std::fs::write(dir.join(name), vec![0u8; 10]).expect("file");
            aged(&dir.join(name), *secs);
        }
        dir
    }

    #[test]
    fn downloads_are_found_with_their_newest_file_time() {
        let scratch = dx_test_scratch::scratch("dx-wt-downloads-");
        let root = scratch.path().join("root");
        let old = download(&root, "aaa", &[("file", 100)]);
        let reading = download(&root, "bbb", &[("file", 100), ("id-x", 500)]);
        let writing = download(&root, "ccc", &[("file", 100), ("tmp-1", 900)]);
        std::fs::create_dir_all(download_store(&root).join("sha256").join("empty")).expect("dir");
        let found = scan_downloads(
            &[root.clone(), root.clone(), scratch.path().join("none")],
            &|| None,
        );
        let cache = download_store(&root);
        assert_eq!(
            found,
            vec![
                Download {
                    dir: old,
                    cache: cache.clone(),
                    used: 100,
                    bytes: 10,
                },
                Download {
                    dir: reading.clone(),
                    cache: cache.clone(),
                    used: 500,
                    bytes: 20,
                },
                Download {
                    dir: writing,
                    cache,
                    used: 900,
                    bytes: 20,
                },
            ]
        );
        aged(&reading.join("file"), 700);
        assert_eq!(download_used(&reading), Some(700));
        assert_eq!(download_used(&reading.join("gone")), None);
    }

    #[test]
    fn a_root_without_a_repository_cache_has_no_downloads() {
        let scratch = dx_test_scratch::scratch("dx-wt-no-downloads-");
        let root = scratch.path().join("root");
        std::fs::create_dir_all(root.join("cache").join("repos").join("v1").join("contents"))
            .expect("contents");
        assert!(scan_downloads(&[root], &|| None).is_empty());
    }

    #[test]
    fn scan_downloads_stops_when_the_stop_probe_fires() {
        let scratch = dx_test_scratch::scratch("dx-wt-stop-downloads-");
        let root = scratch.path().join("root");
        download(&root, "aaa", &[("file", 100)]);
        download(&root, "bbb", &[("file", 100)]);
        assert_eq!(
            scan_downloads(std::slice::from_ref(&root), &|| None).len(),
            2
        );
        assert!(scan_downloads(&[root], &|| Some(15)).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn du_stops_when_the_stop_probe_fires() {
        let scratch = dx_test_scratch::scratch("dx-wt-stop-du-");
        assert_eq!(du_bytes(scratch.path(), &|| Some(15)), None);
    }

    #[cfg(unix)]
    #[test]
    fn du_measures_a_tree_when_not_stopped() {
        let scratch = dx_test_scratch::scratch("dx-wt-du-");
        std::fs::write(scratch.path().join("f"), vec![1u8; 64 * 1024]).expect("file");
        assert!(du_bytes(scratch.path(), &|| None).is_some_and(|bytes| bytes > 0));
    }

    #[test]
    fn this_process_is_alive_and_a_reaped_child_is_not() {
        let host = SystemHost;
        assert!(host.process_alive(std::process::id()));
        let mut child = if cfg!(windows) {
            Command::new("cmd").args(["/C", "exit 0"]).spawn()
        } else {
            Command::new("true").spawn()
        }
        .expect("spawn");
        let pid = child.id();
        child.wait().expect("wait");
        assert!(!host.process_alive(pid));
    }
}
