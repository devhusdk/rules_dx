#![deny(clippy::indexing_slicing, clippy::string_slice)]
#![expect(
    clippy::disallowed_methods,
    reason = "runs local tools that exit on their own"
)]

mod lcov;
mod plan;

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

use plan::Entry;

const COVERLET_FAILED: i32 = 101;

#[derive(Debug, Default, PartialEq, Eq)]
struct Config {
    workspace: String,
    upstream: String,
    coverlet: String,
    test_assembly: String,
    instrument: Vec<String>,
}

fn parse_config(text: &str) -> Result<Config, String> {
    let mut config = Config::default();
    for line in text.lines().filter(|l| !l.is_empty()) {
        let (key, value) = line
            .split_once('\t')
            .ok_or_else(|| format!("malformed config line: {line}"))?;
        let value = value.to_string();
        match key {
            "workspace" => config.workspace = value,
            "upstream" => config.upstream = value,
            "coverlet" => config.coverlet = value,
            "test_assembly" => config.test_assembly = value,
            "instrument" => config.instrument.push(value),
            _ => return Err(format!("unknown config key: {key}")),
        }
    }
    for (name, value) in [
        ("workspace", &config.workspace),
        ("upstream", &config.upstream),
        ("coverlet", &config.coverlet),
        ("test_assembly", &config.test_assembly),
    ] {
        if value.is_empty() {
            return Err(format!("config is missing {name}"));
        }
    }
    Ok(config)
}

fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
}

struct CoverletRun<'a> {
    test_assembly: &'a str,
    target: &'a str,
    include_dirs: &'a [String],
    include_names: &'a [String],
    include_test_assembly: bool,
    output: &'a str,
    target_args: &'a [String],
}

fn coverlet_args(run: &CoverletRun<'_>) -> Vec<String> {
    let mut args = vec![
        run.test_assembly.to_string(),
        "--target".to_string(),
        run.target.to_string(),
        "--format".to_string(),
        "lcov".to_string(),
        "--output".to_string(),
        run.output.to_string(),
    ];
    if !run.target_args.is_empty() {
        let joined: Vec<String> = run.target_args.iter().map(|a| quote_arg(a)).collect();
        args.push("--targetargs".to_string());
        args.push(joined.join(" "));
    }
    for dir in run.include_dirs {
        args.push("--include-directory".to_string());
        args.push(dir.clone());
    }
    for name in run.include_names {
        args.push("--include".to_string());
        args.push(format!("[{name}]*"));
    }
    if run.include_test_assembly {
        args.push("--include-test-assembly".to_string());
    }
    args
}

#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    Passed,
    TestsFailed(i32),
    CoverletFailed(i32),
}

fn outcome(code: i32) -> Outcome {
    match code {
        0 => Outcome::Passed,
        COVERLET_FAILED => Outcome::CoverletFailed(code),
        _ => Outcome::TestsFailed(code),
    }
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<Entry>) -> io::Result<()> {
    for item in fs::read_dir(dir)? {
        let path = item?.path();
        if path.is_dir() {
            walk(root, &path, out)?;
            continue;
        }
        let rel = path.strip_prefix(root).unwrap_or(&path);
        let rlocation = rel.to_string_lossy().replace('\\', "/");
        if rlocation == "MANIFEST" {
            continue;
        }
        out.push(Entry {
            rlocation,
            target: Some(path.to_string_lossy().into_owned()),
        });
    }
    Ok(())
}

fn runfiles_entries() -> Result<(Vec<Entry>, Option<PathBuf>), String> {
    let dir = env::var_os("RUNFILES_DIR")
        .or_else(|| env::var_os("TEST_SRCDIR"))
        .map(PathBuf::from);
    let manifest = env::var_os("RUNFILES_MANIFEST_FILE")
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .or_else(|| {
            dir.as_ref()
                .map(|d| d.join("MANIFEST"))
                .filter(|p| p.is_file())
        });
    if let Some(manifest) = manifest {
        let text = fs::read_to_string(&manifest)
            .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
        return Ok((plan::parse_manifest(&text), dir));
    }
    let Some(dir) = dir else {
        return Err("no runfiles: set RUNFILES_DIR or RUNFILES_MANIFEST_FILE".to_owned());
    };
    let mut entries = Vec::new();
    walk(&dir, &dir, &mut entries).map_err(|e| format!("cannot walk {}: {e}", dir.display()))?;
    Ok((entries, Some(dir)))
}

fn resolve(entries: &[Entry], rlocation: &str) -> Result<PathBuf, String> {
    entries
        .iter()
        .find(|e| e.rlocation == rlocation)
        .and_then(|e| e.target.as_ref())
        .map(PathBuf::from)
        .ok_or_else(|| format!("{rlocation} is not in the runfiles"))
}

#[cfg(unix)]
fn link(src: &Path, dst: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn link(src: &Path, dst: &Path) -> io::Result<()> {
    let linked = if src.is_dir() {
        std::os::windows::fs::symlink_dir(src, dst)
    } else {
        std::os::windows::fs::symlink_file(src, dst)
    };
    linked.or_else(|_| copy_tree(src, dst))
}

#[cfg(windows)]
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    if !src.is_dir() {
        return fs::copy(src, dst).map(|_| ());
    }
    fs::create_dir_all(dst)?;
    for item in fs::read_dir(src)? {
        let item = item?;
        copy_tree(&item.path(), &dst.join(item.file_name()))?;
    }
    Ok(())
}

fn writable_copy(src: &Path, dst: &Path) -> io::Result<()> {
    fs::copy(src, dst)?;
    let mut permissions = fs::metadata(dst)?.permissions();
    #[expect(
        clippy::permissions_set_readonly_false,
        reason = "the copy must be writable by the next step"
    )]
    permissions.set_readonly(false);
    fs::set_permissions(dst, permissions)
}

fn build_mirror(root: &Path, entries: &[Entry], copies: &BTreeSet<String>) -> io::Result<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    let plan = plan::plan(entries, copies);
    let place = |rlocation: &str| -> io::Result<PathBuf> {
        let dst = root.join(rlocation);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(dst)
    };
    for entry in &plan.copy {
        let src = PathBuf::from(entry.target.as_deref().unwrap_or_default());
        writable_copy(&src, &place(&entry.rlocation)?)?;
    }
    for entry in &plan.link {
        let src = PathBuf::from(entry.target.as_deref().unwrap_or_default());
        link(&src, &place(&entry.rlocation)?)?;
    }
    for rlocation in &plan.empty {
        fs::write(place(rlocation)?, b"")?;
    }
    let root_str = root.to_string_lossy();
    fs::write(
        root.join("MANIFEST"),
        plan::render_manifest(entries, &root_str),
    )
}

fn owner_only_dir(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

const DIAG_SHIM: &str = r##"#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>
static void lg(const char *fmt, ...) {
  char buf[600]; va_list ap; va_start(ap, fmt); int n = vsnprintf(buf, sizeof buf, fmt, ap); va_end(ap);
  const char *p = getenv("DX_SHIM_LOG"); if (!p || n <= 0) return; if (n >= (int)sizeof buf) n = sizeof buf - 1;
  int fd = syscall(SYS_openat, AT_FDCWD, p, O_WRONLY | O_APPEND | O_CREAT, 0644); if (fd < 0) return;
  syscall(SYS_write, fd, buf, n); syscall(SYS_close, fd);
}
static int hot(const char *p) { return p && (strstr(p, "/tmp") || strstr(p, ".dotnet")) && !strstr(p, "/_tmp/"); }
#define NEXT(T, name) static __typeof__(T) r; if (!r) r = (__typeof__(T))dlsym(RTLD_NEXT, name)
int mkdir(const char *p, mode_t m) { NEXT(int (*)(const char *, mode_t), "mkdir"); int x = r(p, m); int e = errno; if (hot(p)) lg("pid=%d mkdir %s %o = %d errno=%d\n", getpid(), p, m, x, x ? e : 0); errno = e; return x; }
char *mkdtemp(char *t) { NEXT(char *(*)(char *), "mkdtemp"); char *x = r(t); int e = errno; lg("pid=%d mkdtemp %s = %p errno=%d\n", getpid(), t, (void *)x, x ? 0 : e); errno = e; return x; }
int rename(const char *a, const char *b) { NEXT(int (*)(const char *, const char *), "rename"); int x = r(a, b); int e = errno; if (hot(a) || hot(b)) lg("pid=%d rename %s %s = %d errno=%d\n", getpid(), a, b, x, x ? e : 0); errno = e; return x; }
int chmod(const char *p, mode_t m) { NEXT(int (*)(const char *, mode_t), "chmod"); int x = r(p, m); int e = errno; if (hot(p)) lg("pid=%d chmod %s %o = %d errno=%d\n", getpid(), p, m, x, x ? e : 0); errno = e; return x; }
int stat(const char *restrict p, struct stat *restrict s) { NEXT(int (*)(const char *, struct stat *), "stat"); int x = r(p, s); int e = errno; if (hot(p)) lg("pid=%d stat %s = %d errno=%d mode=%o uid=%d\n", getpid(), p, x, x ? e : 0, x ? 0 : s->st_mode, x ? -1 : (int)s->st_uid); errno = e; return x; }
static int do_open(const char *name, const char *p, int f, mode_t m) { NEXT(int (*)(const char *, int, ...), name); int x = r(p, f, m); int e = errno; if (hot(p)) lg("pid=%d %s %s 0x%x %o = %d errno=%d\n", getpid(), name, p, f, m, x, x < 0 ? e : 0); errno = e; return x; }
int open(const char *p, int f, ...) { mode_t m = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list ap; va_start(ap, f); m = va_arg(ap, int); va_end(ap); } return do_open("open", p, f, m); }
int open64(const char *p, int f, ...) { mode_t m = 0; if (f & (O_CREAT | O_TMPFILE)) { va_list ap; va_start(ap, f); m = va_arg(ap, int); va_end(ap); } return do_open("open64", p, f, m); }
int flock(int fd, int op) { NEXT(int (*)(int, int), "flock"); int x = r(fd, op); int e = errno; lg("pid=%d flock %d %d = %d errno=%d\n", getpid(), fd, op, x, x ? e : 0); errno = e; return x; }
int ftruncate(int fd, off_t l) { NEXT(int (*)(int, off_t), "ftruncate"); int x = r(fd, l); int e = errno; lg("pid=%d ftruncate %d %ld = %d errno=%d\n", getpid(), fd, (long)l, x, x ? e : 0); errno = e; return x; }
int pthread_mutex_init(pthread_mutex_t *mu, const pthread_mutexattr_t *a) { NEXT(int (*)(pthread_mutex_t *, const pthread_mutexattr_t *), "pthread_mutex_init"); int x = r(mu, a); if (x) lg("pid=%d pthread_mutex_init = %d\n", getpid(), x); return x; }
int pthread_mutex_trylock(pthread_mutex_t *mu) { NEXT(int (*)(pthread_mutex_t *), "pthread_mutex_trylock"); int x = r(mu); if (x && x != EBUSY) lg("pid=%d pthread_mutex_trylock = %d\n", getpid(), x); return x; }
pid_t getsid(pid_t p) { NEXT(pid_t (*)(pid_t), "getsid"); pid_t x = r(p); int e = errno; lg("pid=%d getsid(%d) = %d errno=%d\n", getpid(), p, x, x < 0 ? e : 0); errno = e; return x; }
"##;

fn diag_shim(dir: &Path) -> Option<PathBuf> {
    let cc = ["/usr/bin/cc", "/usr/bin/gcc", "/usr/bin/clang"].into_iter().find(|p| Path::new(p).exists());
    eprintln!("DIAG cc={cc:?}");
    let cc = cc?;
    let src = dir.join("shim.c");
    let so = dir.join("shim.so");
    fs::write(&src, DIAG_SHIM).ok()?;
    let out = Command::new(cc).arg("-shared").arg("-fPIC").arg("-o").arg(&so).arg(&src).arg("-ldl").output().ok()?;
    eprintln!("DIAG cc status={:?} {}", out.status, String::from_utf8_lossy(&out.stderr));
    out.status.success().then_some(so)
}

fn diag_trace(dir: &Path) {
    if let Ok(text) = fs::read_to_string(dir.join("shim.log")) {
        for line in text.lines().take(300) {
            eprintln!("DIAG shim {line}");
        }
    }
}

fn run() -> Result<i32, String> {
    let target_args: Vec<String> = env::args().skip(1).collect();
    let (entries, runfiles_dir) = runfiles_entries()?;
    let config_rlocation = env::var("DX_COVERLET_CONFIG")
        .map_err(|e| format!("DX_COVERLET_CONFIG is not set: {e}"))?;
    let config_path = resolve(&entries, &config_rlocation)?;
    let config_text = fs::read_to_string(&config_path)
        .map_err(|e| format!("cannot read {}: {e}", config_path.display()))?;
    let config = parse_config(&config_text)?;

    let coverage_dir = env::var_os("COVERAGE_DIR").map(PathBuf::from);
    let Some(coverage_dir) = coverage_dir.filter(|_| !config.instrument.is_empty()) else {
        let upstream = resolve(&entries, &config.upstream)?;
        let status = Command::new(&upstream)
            .args(&target_args)
            .status()
            .map_err(|e| format!("cannot run {}: {e}", upstream.display()))?;
        return Ok(status.code().unwrap_or(1));
    };

    let tmp = env::var_os("TEST_TMPDIR")
        .map_or_else(env::temp_dir, PathBuf::from)
        .join("dx_coverlet");
    let root = tmp.join("runfiles");
    let mut assemblies = config.instrument.clone();
    assemblies.push(config.test_assembly.clone());
    let copies = plan::copy_set(&entries, &assemblies);
    build_mirror(&root, &entries, &copies)
        .map_err(|e| format!("cannot mirror runfiles into {}: {e}", root.display()))?;

    let in_mirror = |rlocation: &str| root.join(rlocation).to_string_lossy().into_owned();
    let mut include_dirs = BTreeSet::new();
    let mut include_names = BTreeSet::new();
    for assembly in &config.instrument {
        include_dirs.insert(in_mirror(plan::parent(assembly)));
        include_names.insert(plan::assembly_name(assembly).to_string());
    }
    let include_dirs: Vec<String> = include_dirs.into_iter().collect();
    let include_names: Vec<String> = include_names.into_iter().collect();
    let output = tmp.join("coverage.info");
    let output_str = output.to_string_lossy().into_owned();
    let test_assembly = in_mirror(&config.test_assembly);
    let target = in_mirror(&config.upstream);
    let args = coverlet_args(&CoverletRun {
        test_assembly: &test_assembly,
        target: &target,
        include_dirs: &include_dirs,
        include_names: &include_names,
        include_test_assembly: config.instrument.contains(&config.test_assembly),
        output: &output_str,
        target_args: &target_args,
    });

    let mut args = args;
    if env::var_os("VERBOSE_COVERAGE").is_some() {
        args.extend(["--verbosity".to_string(), "detailed".to_string()]);
    }
    let coverlet = in_mirror(&config.coverlet);
    let manifest = root.join("MANIFEST");
    let test_tmp = tmp.join("tmp");
    owner_only_dir(&test_tmp).map_err(|e| format!("cannot create {}: {e}", test_tmp.display()))?;
    let mut command = Command::new(&coverlet);
    if let Some(so) = diag_shim(&test_tmp) {
        command.env("LD_PRELOAD", so).env("DX_SHIM_LOG", test_tmp.join("shim.log"));
    }
    let status = command
        .args(&args)
        .current_dir(root.join(&config.workspace))
        .env("RUNFILES_DIR", &root)
        .env("TEST_SRCDIR", &root)
        .env("JAVA_RUNFILES", &root)
        .env("RUNFILES_MANIFEST_FILE", &manifest)
        .env("TMPDIR", &test_tmp)
        .env("TMP", &test_tmp)
        .env("TEMP", &test_tmp)
        .status()
        .map_err(|e| format!("cannot run coverlet {coverlet}: {e}"))?;
    let Some(code) = status.code() else {
        return Err("coverlet was killed by a signal".to_owned());
    };
    diag_trace(&test_tmp);
    let code = match outcome(code) {
        Outcome::CoverletFailed(code) => {
            eprintln!("dx: coverlet failed");
            return Ok(code);
        }
        Outcome::TestsFailed(code) => code,
        Outcome::Passed => 0,
    };

    let report = fs::read_to_string(&output).unwrap_or_default();
    let mut prefixes = vec![in_mirror(&config.workspace)];
    if let Some(dir) = runfiles_dir {
        prefixes.push(dir.join(&config.workspace).to_string_lossy().into_owned());
    }
    let (normalized, kept) = lcov::normalize(&report, &prefixes);
    fs::create_dir_all(&coverage_dir)
        .map_err(|e| format!("cannot create {}: {e}", coverage_dir.display()))?;
    let dat = coverage_dir.join("coverlet.dat");
    fs::write(&dat, normalized).map_err(|e| format!("cannot write {}: {e}", dat.display()))?;
    if code == 0 && kept == 0 {
        return Err(format!(
            "coverlet wrote no SF: records for {}",
            config.instrument.join(", ")
        ));
    }
    Ok(code)
}

pub fn main() -> std::process::ExitCode {
    let code = run().unwrap_or_else(|message| {
        eprintln!("dx: {message}");
        1
    });
    process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_config() {
        let config = parse_config(
            "workspace\t_main\nupstream\t_main/p/t.sh\ncoverlet\tpaket/tool.sh\ntest_assembly\t_main/p/t.dll\ninstrument\t_main/p/a.dll\ninstrument\t_main/p/b.dll\n",
        )
        .unwrap();
        assert_eq!(config.workspace, "_main");
        assert_eq!(config.instrument, vec!["_main/p/a.dll", "_main/p/b.dll"]);
    }

    #[test]
    fn rejects_incomplete_or_unknown_config() {
        assert!(parse_config("workspace\t_main\n").is_err());
        assert!(parse_config("bogus\tx\n").is_err());
        assert!(parse_config("no tab\n").is_err());
    }

    #[test]
    fn builds_coverlet_args_with_spaces() {
        let dirs = vec!["/m/_main/p/lib dir".to_string()];
        let names = vec!["greeter_lib".to_string()];
        let target_args = vec![
            "--filter".to_string(),
            "a b".to_string(),
            "q\"x".to_string(),
        ];
        let args = coverlet_args(&CoverletRun {
            test_assembly: "/m/_main/p/t.dll",
            target: "/m/_main/p/t.sh",
            include_dirs: &dirs,
            include_names: &names,
            include_test_assembly: true,
            output: "/tmp/x/coverage.info",
            target_args: &target_args,
        });
        assert_eq!(
            args,
            vec![
                "/m/_main/p/t.dll",
                "--target",
                "/m/_main/p/t.sh",
                "--format",
                "lcov",
                "--output",
                "/tmp/x/coverage.info",
                "--targetargs",
                "--filter \"a b\" \"q\\\"x\"",
                "--include-directory",
                "/m/_main/p/lib dir",
                "--include",
                "[greeter_lib]*",
                "--include-test-assembly",
            ]
        );
    }

    #[test]
    fn omits_optional_coverlet_args() {
        let args = coverlet_args(&CoverletRun {
            test_assembly: "t.dll",
            target: "t.sh",
            include_dirs: &[],
            include_names: &[],
            include_test_assembly: false,
            output: "o.info",
            target_args: &[],
        });
        assert_eq!(args.len(), 7);
        assert_eq!(quote_arg(""), "\"\"");
    }

    #[cfg(unix)]
    #[test]
    fn owner_only_dir_drops_group_and_other_access() {
        use std::os::unix::fs::PermissionsExt;
        let dir = PathBuf::from(env::var_os("TEST_TMPDIR").unwrap()).join("owner_only/tmp");
        owner_only_dir(&dir).unwrap();
        owner_only_dir(&dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o7777, 0o700);
    }

    #[test]
    fn maps_exit_codes() {
        assert_eq!(outcome(0), Outcome::Passed);
        assert_eq!(outcome(1), Outcome::TestsFailed(1));
        assert_eq!(outcome(2), Outcome::TestsFailed(2));
        assert_eq!(outcome(101), Outcome::CoverletFailed(101));
    }
}
