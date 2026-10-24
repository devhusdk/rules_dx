use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One running Bazel server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Server {
    pub(crate) pid: u32,
    pub(crate) rss_bytes: u64,
    pub(crate) workspace: Option<PathBuf>,
    pub(crate) output_base: Option<PathBuf>,
    pub(crate) output_user_root: Option<PathBuf>,
}

/// One Bazel output base and the workspace it was made for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutputBase {
    pub(crate) dir: PathBuf,
    pub(crate) workspace: PathBuf,
}

/// The machine facts the pool reads and the effects it has outside git.
pub(crate) trait Host {
    fn now(&self) -> u64;
    fn process_alive(&self, pid: u32) -> bool;
    fn process_cwds(&self) -> Option<Vec<PathBuf>>;
    fn free_disk_percent(&self, path: &Path) -> Option<f64>;
    fn servers(&self) -> Vec<Server>;
    fn output_roots(&self) -> Vec<PathBuf>;
    fn bazel_shutdown(&self, workspace: &Path);
    fn disk_bytes(&self, path: &Path) -> u64;
}

/// Compares two paths the way Bazel prints them.
pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    normalize(left) == normalize(right)
}

fn normalize(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
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

fn walk_bytes(path: &Path) -> u64 {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| walk_bytes(&entry.path()))
        .sum()
}

/// The machine this process runs on.
pub(crate) struct SystemHost;

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
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains(&format!("\"{pid}\"")))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn list_cwds() -> Option<Vec<PathBuf>> {
    let entries = std::fs::read_dir("/proc").ok()?;
    Some(
        entries
            .flatten()
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.bytes().all(|byte| byte.is_ascii_digit()))
            })
            .filter_map(|entry| std::fs::read_link(entry.path().join("cwd")).ok())
            .collect(),
    )
}

/// Reads process working directories from `lsof -Fn` output.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parse_lsof_cwds(text: &str) -> Vec<PathBuf> {
    text.lines()
        .filter_map(|line| line.strip_prefix('n'))
        .filter(|name| !name.is_empty())
        .map(PathBuf::from)
        .collect()
}

#[cfg(target_os = "macos")]
fn list_cwds() -> Option<Vec<PathBuf>> {
    let output = Command::new("lsof")
        .args(["-n", "-P", "-w", "-d", "cwd", "-Fn"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let cwds = parse_lsof_cwds(&String::from_utf8_lossy(&output.stdout));
    if cwds.is_empty() {
        None
    } else {
        Some(cwds)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn list_cwds() -> Option<Vec<PathBuf>> {
    None
}

#[cfg(unix)]
fn list_servers() -> String {
    Command::new("ps")
        .args(["-eo", "pid=,rss=,args="])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

#[cfg(not(unix))]
fn list_servers() -> String {
    let script = "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -like '*A-server.jar*' } | ForEach-Object { \"$($_.ProcessId) $([int]($_.WorkingSetSize/1024)) $($_.CommandLine)\" }";
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// The percent free from `available total` byte counts.
pub(crate) fn percent_from_counts(available: &str, total: &str) -> Option<f64> {
    let available: f64 = available.trim().parse().ok()?;
    let total: f64 = total.trim().parse().ok()?;
    if total <= 0.0 {
        return None;
    }
    Some(available / total * 100.0)
}

/// The percent free from `df -Pk` output.
#[cfg_attr(not(unix), allow(dead_code))]
pub(crate) fn percent_from_df(text: &str) -> Option<f64> {
    let fields: Vec<&str> = text.lines().nth(1)?.split_whitespace().collect();
    percent_from_counts(fields.get(3)?, fields.get(1)?)
}

#[cfg(unix)]
fn free_percent_of(path: &Path) -> Option<f64> {
    let output = Command::new("df")
        .args(["-Pk"])
        .arg(path)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    percent_from_df(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(unix))]
fn free_percent_of(path: &Path) -> Option<f64> {
    let quoted = path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$d = [System.IO.DriveInfo]::new([System.IO.Path]::GetPathRoot('{quoted}')); \"$($d.AvailableFreeSpace) $($d.TotalSize)\""
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut fields = text.split_whitespace();
    percent_from_counts(fields.next()?, fields.next()?)
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn user_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_default()
}

/// Where Bazel keeps output bases when nothing moves them.
pub(crate) fn default_output_roots() -> Vec<PathBuf> {
    let user = user_name();
    let mut roots = Vec::new();
    if let Some(home) = home() {
        let cache = std::env::var_os("XDG_CACHE_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".cache"));
        roots.push(cache.join("bazel").join(format!("_bazel_{user}")));
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

impl Host for SystemHost {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0)
    }

    fn process_alive(&self, pid: u32) -> bool {
        pid_alive(pid)
    }

    fn process_cwds(&self) -> Option<Vec<PathBuf>> {
        list_cwds()
    }

    fn free_disk_percent(&self, path: &Path) -> Option<f64> {
        free_percent_of(path)
    }

    fn servers(&self) -> Vec<Server> {
        parse_servers(&list_servers())
    }

    fn output_roots(&self) -> Vec<PathBuf> {
        let mut roots = default_output_roots();
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
        if !self.servers().iter().any(|server| {
            server
                .workspace
                .as_deref()
                .is_some_and(|dir| same_path(dir, workspace))
        }) {
            return;
        }
        let Some(bazel) = find_bazel() else {
            return;
        };
        let _ = Command::new(bazel)
            .arg("shutdown")
            .current_dir(workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    fn disk_bytes(&self, path: &Path) -> u64 {
        if cfg!(unix) {
            if let Ok(output) = Command::new("du")
                .arg("-sk")
                .arg(path)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
            {
                let text = String::from_utf8_lossy(&output.stdout).into_owned();
                if let Some(kib) = text
                    .split_whitespace()
                    .next()
                    .and_then(|first| first.parse::<u64>().ok())
                {
                    return kib * 1024;
                }
            }
        }
        walk_bytes(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn free_space_is_read_from_df_lines_and_byte_counts() {
        let df = "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/x 1000 900 100 90% /\n";
        assert_eq!(percent_from_df(df), Some(10.0));
        assert_eq!(percent_from_counts("25", "100"), Some(25.0));
        assert_eq!(percent_from_counts("1", "0"), None);
        assert_eq!(percent_from_counts("x", "5"), None);
        assert_eq!(percent_from_df(""), None);
    }

    #[test]
    fn paths_compare_across_separators_and_trailing_slashes() {
        assert!(same_path(Path::new("/a/b/"), Path::new("/a/b")));
        assert!(same_path(Path::new("C:\\a\\b"), Path::new("C:/a/b")));
        assert!(!same_path(Path::new("/a/b"), Path::new("/a/c")));
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
        assert!(walk_bytes(&dir) >= 150);
        assert_eq!(walk_bytes(&dir.join("missing")), 0);
    }

    #[test]
    fn working_directories_are_read_from_lsof_fields() {
        let text = "p1\nfcwd\nn/\np42\nfcwd\nn/Users/u/my repo/slot\nn\n";
        assert_eq!(
            parse_lsof_cwds(text),
            vec![PathBuf::from("/"), PathBuf::from("/Users/u/my repo/slot")]
        );
        assert!(parse_lsof_cwds("").is_empty());
    }

    #[test]
    fn a_running_child_shows_its_working_directory() {
        if !cfg!(any(target_os = "linux", target_os = "macos")) {
            assert_eq!(SystemHost.process_cwds(), None);
            return;
        }
        let scratch = dx_test_scratch::scratch("dx-wt-cwd-");
        let dir = std::fs::canonicalize(scratch.path()).expect("canonical");
        let mut child = Command::new("sleep")
            .arg("30")
            .current_dir(&dir)
            .spawn()
            .expect("spawn");
        let cwds = SystemHost.process_cwds();
        child.kill().expect("kill");
        child.wait().expect("wait");
        assert!(cwds.expect("cwds").contains(&dir));
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
