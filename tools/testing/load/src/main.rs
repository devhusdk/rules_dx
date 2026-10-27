mod metrics;

use dx_process::RcPolicy;
use metrics::{Peaks, Phase, Setup};
use std::collections::BTreeSet;
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const USAGE: &str = "usage: parallel_worktrees [--worktrees N] [--target LABEL] [--out FILE] [--isolated-repo-cache] [--force]\n       parallel_worktrees --compare FIRST.json SECOND.json";
const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);
const TOLERANCE_PERCENT: f64 = 15.0;
const NAME_PREFIX: &str = "parallel-load-";
const MIN_AVAILABLE_KIB_PER_WORKTREE: u64 = 3 * 1024 * 1024;
const DX_ENV: &str = "DX_LOAD_DX";
const WAIT_LINE: &str = "waiting for a build slot";
const DX_CHILD_ENV_REMOVED: [&str; 5] = [
    "BUILD_WORKSPACE_DIRECTORY",
    "BUILD_WORKING_DIRECTORY",
    "DX_BUILD_SLOT",
    "DX_BUILD_SHARE",
    "BAZEL_TEST",
];
const SERVER_STOP_TIMEOUT: Duration = Duration::from_secs(10);
const PR_SET_PDEATHSIG: i32 = 1;
const SIGINT: i32 = 2;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn kill(pid: i32, sig: i32) -> i32;
}

extern "C" fn on_signal(_signum: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() {
    let handler = on_signal as extern "C" fn(i32) as usize;
    unsafe {
        signal(SIGINT, handler);
        signal(SIGTERM, handler);
    }
}

fn stop_with_parent() {
    let parent = std::os::unix::process::parent_id();
    unsafe {
        prctl(PR_SET_PDEATHSIG, SIGTERM as u64, 0, 0, 0);
    }
    if std::os::unix::process::parent_id() != parent {
        INTERRUPTED.store(true, Ordering::SeqCst);
    }
}

fn check_interrupted() -> Result<(), String> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        Err("interrupted; stopped its Bazel servers and released its worktrees".to_string())
    } else {
        Ok(())
    }
}

fn parse_mem_available(meminfo: &str) -> Option<u64> {
    meminfo
        .lines()
        .find_map(|line| line.strip_prefix("MemAvailable:"))
        .and_then(|rest| rest.split_whitespace().next()?.parse().ok())
}

fn available_kib() -> Option<u64> {
    fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|text| parse_mem_available(&text))
}

fn check_memory(available_kib: Option<u64>, worktrees: usize, force: bool) -> Result<(), String> {
    if force {
        return Ok(());
    }
    let gib = |kib: u64| kib as f64 / (1024.0 * 1024.0);
    let needed = MIN_AVAILABLE_KIB_PER_WORKTREE * worktrees as u64;
    match available_kib {
        Some(available) if available >= needed => Ok(()),
        Some(available) => Err(format!(
            "{worktrees} worktrees need {:.1} GiB of available memory ({:.0} GiB each), but only {:.1} GiB is available; pass --force to run anyway",
            gib(needed),
            gib(MIN_AVAILABLE_KIB_PER_WORKTREE),
            gib(available)
        )),
        None => Err(
            "cannot read MemAvailable from /proc/meminfo; pass --force to run anyway".to_string(),
        ),
    }
}

struct Options {
    worktrees: usize,
    target: String,
    out: Option<PathBuf>,
    isolated_repo_cache: bool,
    force: bool,
}

enum Mode {
    Run(Options),
    Compare(PathBuf, PathBuf),
}

fn parse_args(args: &[String]) -> Result<Mode, String> {
    let mut options = Options {
        worktrees: 4,
        target: "//...".to_string(),
        out: None,
        isolated_repo_cache: false,
        force: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |flag: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match arg.as_str() {
            "--worktrees" => {
                options.worktrees = value("--worktrees")?
                    .parse()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or("--worktrees needs a positive number")?;
            }
            "--target" => options.target = value("--target")?,
            "--out" => options.out = Some(PathBuf::from(value("--out")?)),
            "--isolated-repo-cache" => options.isolated_repo_cache = true,
            "--force" => options.force = true,
            "--compare" => {
                let first = PathBuf::from(value("--compare")?);
                let second = PathBuf::from(value("--compare")?);
                return Ok(Mode::Compare(first, second));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Mode::Run(options))
}

fn die_with_driver(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            prctl(PR_SET_PDEATHSIG, SIGINT as u64, 0, 0, 0);
            Ok(())
        });
    }
}

fn bazel(dir: &Path, output_base: &Path, policy: &RcPolicy) -> Command {
    let argv = dx_process::startup_argv_with(
        policy,
        &[format!("--output_base={}", output_base.display())],
    );
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(dir)
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .env_remove("BUILD_WORKING_DIRECTORY")
        .stdin(Stdio::null());
    die_with_driver(&mut command);
    command
}

fn dx(dir: &Path, binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command.current_dir(dir).stdin(Stdio::null());
    for name in DX_CHILD_ENV_REMOVED {
        command.env_remove(name);
    }
    die_with_driver(&mut command);
    command
}

/// The rc policy dx resolves in a dx child started from `tree`.
fn rc_policy(
    env_get: &dyn Fn(&str) -> Option<String>,
    tree: &Path,
    read_file: &dyn Fn(&Path) -> Option<String>,
) -> RcPolicy {
    let child_env = |name: &str| {
        if DX_CHILD_ENV_REMOVED.contains(&name) {
            None
        } else {
            env_get(name)
        }
    };
    RcPolicy::from_env(&child_env, tree, read_file)
}

/// The disk cache dx test uses: the last --disk_cache value, or None when it is empty.
fn disk_cache(policy: &RcPolicy, test_args: &[String]) -> Option<String> {
    policy
        .verb_options("test", None, &[])
        .iter()
        .chain(test_args)
        .filter_map(|arg| arg.strip_prefix("--disk_cache="))
        .next_back()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn dx_test_args(options: &Options, output_base: &Path) -> Vec<String> {
    let mut args = vec![
        "test".to_string(),
        format!(
            "--bazel-startup-option=--output_base={}",
            output_base.display()
        ),
        "--output=text".to_string(),
        options.target.clone(),
        "--".to_string(),
        "--keep_going".to_string(),
        "--disk_cache=".to_string(),
        "--remote_cache=".to_string(),
    ];
    if options.isolated_repo_cache {
        args.push(format!(
            "--repository_cache={}",
            output_base.join("load-repo-cache").display()
        ));
    }
    args
}

fn waited_for_slot(log: &str) -> bool {
    log.lines().any(|line| line.contains(WAIT_LINE))
}

fn bazel_output(
    dir: &Path,
    output_base: &Path,
    policy: &RcPolicy,
    args: &[&str],
) -> Result<String, String> {
    let output = bazel(dir, output_base, policy)
        .args(args)
        .output()
        .map_err(|err| format!("bazel {}: {err}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "bazel {} failed in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn captured(mut command: Command, what: &str) -> Result<String, String> {
    let output = command
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .env_remove("BUILD_WORKING_DIRECTORY")
        .stdin(Stdio::null())
        .output()
        .map_err(|err| format!("{what}: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "{what} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git(workspace: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(workspace).args(args);
    captured(command, &format!("git {}", args.join(" ")))
}

fn dx_tool(workspace: &Path, binary: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = dx(workspace, binary);
    command.args(args);
    captured(command, &format!("dx {}", args.join(" ")))
}

fn cache_dir() -> Result<PathBuf, String> {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .ok_or_else(|| "set HOME or XDG_CACHE_HOME".to_string())
}

fn dir_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if meta.is_dir() {
        fs::read_dir(path)
            .map(|entries| entries.flatten().map(|e| dir_size(&e.path())).sum())
            .unwrap_or(0)
    } else {
        meta.len()
    }
}

fn remove_tree(path: &Path) {
    if fs::remove_dir_all(path).is_ok() || !path.exists() {
        return;
    }
    let _ = Command::new("chmod")
        .args(["-R", "u+rwX"])
        .arg(path)
        .status();
    let _ = fs::remove_dir_all(path);
}

fn alive(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(')')
                .map(|(_, rest)| !rest.trim_start().starts_with('Z'))
        })
        .unwrap_or(false)
}

fn serves(pid: u32, output_base: &Path) -> bool {
    fs::read(format!("/proc/{pid}/cmdline"))
        .map(|cmdline| {
            String::from_utf8_lossy(&cmdline).contains(output_base.to_string_lossy().as_ref())
        })
        .unwrap_or(false)
}

fn stop_server(output_base: &Path) {
    let Some(pid) = server_pid(output_base) else {
        return;
    };
    if !alive(pid) || !serves(pid, output_base) {
        return;
    }
    unsafe {
        kill(pid as i32, SIGTERM);
    }
    let deadline = Instant::now() + SERVER_STOP_TIMEOUT;
    while alive(pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(100));
    }
    if alive(pid) {
        unsafe {
            kill(pid as i32, SIGKILL);
        }
    }
}

fn slot_path(json_lines: &str) -> Option<String> {
    json_lines
        .lines()
        .filter(|line| line.contains(r#""event":"slot""#))
        .find_map(|line| {
            let rest = line.split_once(r#""path":""#)?.1;
            Some(rest[..rest.find('"')?].to_string())
        })
}

fn release_branch(workspace: &Path, binary: &Path, branch: &str) {
    if let Some(path) = dx_tool(
        workspace,
        binary,
        &["worktree", "path", branch, "--output=json"],
    )
    .ok()
    .as_deref()
    .and_then(slot_path)
    {
        let _ = dx_tool(
            workspace,
            binary,
            &["worktree", "release", &path, "--force"],
        );
    }
    let _ = git(workspace, &["branch", "-D", branch]);
}

struct Tree {
    dir: PathBuf,
    output_base: PathBuf,
    branch: Option<String>,
}

struct Trees {
    workspace: PathBuf,
    binary: PathBuf,
    trees: Vec<Tree>,
}

impl Trees {
    fn remove(&self, tree: &Tree) {
        stop_server(&tree.output_base);
        remove_tree(&tree.output_base);
        match &tree.branch {
            Some(branch) => release_branch(&self.workspace, &self.binary, branch),
            None => remove_tree(&tree.dir),
        }
    }
}

impl Drop for Trees {
    fn drop(&mut self) {
        for tree in &self.trees {
            self.remove(tree);
        }
    }
}

fn is_stale(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(NAME_PREFIX) else {
        return false;
    };
    match rest
        .split('-')
        .next()
        .and_then(|pid| pid.parse::<u32>().ok())
    {
        Some(pid) => !Path::new(&format!("/proc/{pid}")).exists(),
        None => true,
    }
}

fn sweep_stale(workspace: &Path, binary: &Path, bases: &Path) {
    let pattern = format!("refs/heads/{NAME_PREFIX}*");
    let branches = git(
        workspace,
        &["for-each-ref", "--format=%(refname:short)", &pattern],
    )
    .unwrap_or_default();
    for branch in branches.lines().filter(|name| is_stale(name)) {
        release_branch(workspace, binary, branch);
    }
    let names: BTreeSet<String> = fs::read_dir(bases)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .filter(|name| is_stale(name))
                .collect()
        })
        .unwrap_or_default();
    for name in names {
        let base = bases.join(name);
        stop_server(&base);
        remove_tree(&base);
    }
}

fn create_trees(
    workspace: &Path,
    binary: &Path,
    bases: &Path,
    count: usize,
) -> Result<Trees, String> {
    fs::create_dir_all(bases).map_err(|err| format!("{}: {err}", bases.display()))?;
    let head = git(workspace, &["rev-parse", "HEAD"])?;
    let create = format!("--create={head}");
    let mut trees = Trees {
        workspace: workspace.to_path_buf(),
        binary: binary.to_path_buf(),
        trees: Vec::new(),
    };
    for index in 0..count {
        check_interrupted()?;
        let name = format!("{NAME_PREFIX}{}-{index}", std::process::id());
        let added = dx_tool(
            workspace,
            binary,
            &["worktree", "add", &name, &create, "--output=path"],
        )
        .and_then(|path| {
            path.lines()
                .last()
                .map(PathBuf::from)
                .ok_or_else(|| format!("dx worktree add {name} printed no path"))
        });
        let dir = match added {
            Ok(dir) => dir,
            Err(err) => {
                release_branch(workspace, binary, &name);
                return Err(err);
            }
        };
        trees.trees.push(Tree {
            dir: dir.clone(),
            output_base: bases.join(&name),
            branch: Some(name),
        });
    }
    Ok(trees)
}

fn server_pid(output_base: &Path) -> Option<u32> {
    fs::read_to_string(output_base.join("server/server.pid.txt"))
        .ok()?
        .trim()
        .parse()
        .ok()
}

fn start_servers(trees: &Trees, policy: &RcPolicy) -> Result<Vec<u32>, String> {
    let mut pids = Vec::new();
    for tree in &trees.trees {
        check_interrupted()?;
        bazel_output(&tree.dir, &tree.output_base, policy, &["version"])?;
        match server_pid(&tree.output_base) {
            Some(pid) if alive(pid) => pids.push(pid),
            _ => {
                return Err(format!(
                    "no running Bazel server after start in {}",
                    tree.dir.display()
                ))
            }
        }
    }
    Ok(pids)
}

/// True when dx replaced the server: the pid changed and the new server is alive.
/// A server that idled out or stopped under low memory is not a restart.
fn restarted(before: u32, after: Option<u32>, after_alive: bool) -> bool {
    matches!(after, Some(pid) if pid != before) && after_alive
}

fn sample(bases: &[PathBuf], peaks: &mut Peaks) {
    if let Some(psi) = fs::read_to_string("/proc/pressure/memory")
        .ok()
        .and_then(|text| metrics::parse_psi(&text))
    {
        peaks.record_psi(psi);
    }
    let rss: Vec<(u32, u64)> = bases
        .iter()
        .filter_map(|base| server_pid(base))
        .filter_map(|pid| {
            let status = fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
            Some((pid, metrics::parse_rss_kb(&status)?))
        })
        .collect();
    peaks.record_servers(rss.len(), &rss);
}

fn oom_kills() -> u64 {
    fs::read_to_string("/proc/vmstat")
        .ok()
        .and_then(|text| metrics::parse_oom_kills(&text))
        .unwrap_or(0)
}

fn run_phase(
    trees: &Trees,
    options: &Options,
    binary: &Path,
    policy: &RcPolicy,
    label: &str,
) -> Result<Phase, String> {
    let bases: Vec<PathBuf> = trees
        .trees
        .iter()
        .map(|tree| tree.output_base.clone())
        .collect();
    let stop = Arc::new(AtomicBool::new(false));
    let sampler = {
        let stop = Arc::clone(&stop);
        let bases = bases.clone();
        thread::spawn(move || {
            let mut peaks = Peaks::default();
            while !stop.load(Ordering::Relaxed) {
                sample(&bases, &mut peaks);
                thread::sleep(SAMPLE_INTERVAL);
            }
            sample(&bases, &mut peaks);
            peaks
        })
    };
    let oom_before = oom_kills();
    let start = Instant::now();
    let mut children: Vec<(PathBuf, Child)> = Vec::new();
    let mut spawn_error = None;
    for tree in &trees.trees {
        let log = tree.dir.join(format!("{label}.log"));
        let spawned = fs::File::create(&log)
            .map_err(|err| err.to_string())
            .and_then(|file| {
                let err_file = file.try_clone().map_err(|err| err.to_string())?;
                dx(&tree.dir, binary)
                    .args(dx_test_args(options, &tree.output_base))
                    .stdout(file)
                    .stderr(err_file)
                    .spawn()
                    .map_err(|err| err.to_string())
            });
        match spawned {
            Ok(child) => children.push((log, child)),
            Err(err) => {
                spawn_error = Some(err);
                break;
            }
        }
    }
    let mut exit_codes = Vec::new();
    for index in 0..children.len() {
        let status = loop {
            if INTERRUPTED.load(Ordering::SeqCst) {
                for (_, other) in &mut children {
                    let _ = other.kill();
                    let _ = other.wait();
                }
                stop.store(true, Ordering::Relaxed);
                let _ = sampler.join();
                return Err("interrupted".to_string());
            }
            match children[index]
                .1
                .try_wait()
                .map_err(|err| err.to_string())?
            {
                Some(status) => break status,
                None => thread::sleep(Duration::from_millis(200)),
            }
        };
        let code = status.code().unwrap_or(-1);
        let log = &children[index].0;
        if !matches!(code, 0 | 3) {
            let text = fs::read_to_string(log).unwrap_or_default();
            let tail: Vec<&str> = text.lines().rev().take(20).collect();
            eprintln!(
                "dx test exited {code} in {label} phase:\n{}",
                tail.into_iter().rev().collect::<Vec<_>>().join("\n")
            );
        }
        exit_codes.push(code);
    }
    let wall_seconds = start.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let peaks = sampler.join().map_err(|_| "sampler panicked".to_string())?;
    if let Some(err) = spawn_error {
        return Err(format!("could not start dx: {err}"));
    }
    if let Some(code) = exit_codes.iter().find(|code| !matches!(code, 0 | 3)) {
        return Err(format!("dx test exited {code} in {label} phase"));
    }
    let mut jvm_heap_after_gc_bytes = 0;
    for tree in &trees.trees {
        let Some(pid) = server_pid(&tree.output_base) else {
            continue;
        };
        if !alive(pid) || !serves(pid, &tree.output_base) {
            continue;
        }
        let heap = bazel_output(
            &tree.dir,
            &tree.output_base,
            policy,
            &["info", "used-heap-size-after-gc"],
        )
        .ok()
        .and_then(|text| metrics::parse_heap_size(&text));
        if server_pid(&tree.output_base) != Some(pid) {
            return Err(format!(
                "bazel info restarted the Bazel server in {}; its startup flags differ from dx",
                tree.dir.display()
            ));
        }
        jvm_heap_after_gc_bytes = jvm_heap_after_gc_bytes.max(heap.unwrap_or(0));
    }
    let slot_waits = children
        .iter()
        .filter(|(log, _)| waited_for_slot(&fs::read_to_string(log).unwrap_or_default()))
        .count();
    Ok(Phase {
        wall_seconds,
        peaks,
        jvm_heap_after_gc_bytes,
        output_base_bytes: bases.iter().map(|base| dir_size(base)).sum(),
        oom_kills: oom_kills().saturating_sub(oom_before),
        slot_waits,
        exit_codes,
    })
}

fn run(options: &Options) -> Result<(), String> {
    let workspace =
        PathBuf::from(std::env::var("BUILD_WORKSPACE_DIRECTORY").map_err(|_| {
            "run with bazel run //tools/testing/load:parallel_worktrees".to_string()
        })?);
    let out = match &options.out {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => base_dir(&workspace).join(path),
        None => base_dir(&workspace).join("parallel_worktrees.json"),
    };
    let binary = dx_binary()?;
    install_signal_handlers();
    stop_with_parent();
    let bases = cache_dir()?.join("dx/parallel-load");
    sweep_stale(&workspace, &binary, &bases);
    check_memory(available_kib(), options.worktrees, options.force)?;
    check_interrupted()?;
    let trees = create_trees(&workspace, &binary, &bases, options.worktrees)?;
    let first = trees
        .trees
        .first()
        .ok_or_else(|| "no worktrees".to_string())?;
    let policy = rc_policy(&|name| std::env::var(name).ok(), &first.dir, &|path| {
        fs::read_to_string(path).ok()
    });
    let setup = Setup {
        user_rc: policy.read_user_rc,
        disk_cache: disk_cache(&policy, &dx_test_args(options, &first.output_base)),
    };
    let started = start_servers(&trees, &policy)?;
    let cold = run_phase(&trees, options, &binary, &policy, "cold")?;
    for (tree, before) in trees.trees.iter().zip(&started) {
        let after = server_pid(&tree.output_base);
        if restarted(*before, after, after.is_some_and(alive)) {
            return Err(format!(
                "dx restarted the pre-started Bazel server in {}; its startup flags differ from the harness",
                tree.dir.display()
            ));
        }
    }
    let warm = run_phase(&trees, options, &binary, &policy, "warm")?;
    drop(trees);
    check_interrupted()?;
    fs::write(
        &out,
        metrics::to_json(options.worktrees, &options.target, &setup, &cold, &warm),
    )
    .map_err(|err| format!("{}: {err}", out.display()))?;
    println!("{}", setup.line());
    print!("{}", metrics::to_table(&cold, &warm));
    println!("wrote {}", out.display());
    Ok(())
}

fn dx_binary() -> Result<PathBuf, String> {
    let path = std::env::var(DX_ENV).map_err(|_| format!("{DX_ENV} is not set"))?;
    let path = std::env::current_dir()
        .map_err(|err| err.to_string())?
        .join(path);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("no dx at {}", path.display()))
    }
}

fn base_dir(workspace: &Path) -> PathBuf {
    std::env::var("BUILD_WORKING_DIRECTORY")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace.to_path_buf())
}

fn resolve(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match std::env::var("BUILD_WORKING_DIRECTORY") {
        Ok(dir) => PathBuf::from(dir).join(path),
        Err(_) => path.to_path_buf(),
    }
}

fn compare(first: &Path, second: &Path) -> Result<bool, String> {
    let read = |path: &Path| {
        let path = resolve(path);
        fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))
    };
    let deltas = metrics::compare(&read(first)?, &read(second)?)?;
    print!("{}", metrics::compare_table(&deltas, TOLERANCE_PERCENT));
    Ok(deltas
        .iter()
        .all(|delta| delta.percent <= TOLERANCE_PERCENT))
}

fn finish(result: Result<bool, String>) -> ! {
    match result {
        Ok(true) => std::process::exit(0),
        Ok(false) => {
            eprintln!("some metrics differ by more than {TOLERANCE_PERCENT}%");
            std::process::exit(1);
        }
        Err(err) if INTERRUPTED.load(Ordering::SeqCst) => {
            eprintln!("{err}");
            std::process::exit(130);
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(2);
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = match parse_args(&args) {
        Ok(mode) => mode,
        Err(err) => {
            eprintln!("{err}\n{USAGE}");
            std::process::exit(2);
        }
    };
    finish(match mode {
        Mode::Run(options) => run(&options).map(|()| true),
        Mode::Compare(first, second) => compare(&first, &second),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    const CHILD_ENV: &str = "PARALLEL_WORKTREES_DRIVER_CHILD";
    const WAIT: Duration = Duration::from_secs(30);

    #[test]
    fn driver_child_process() {
        let Some(root) = std::env::var_os(CHILD_ENV).map(PathBuf::from) else {
            return;
        };
        install_signal_handlers();
        stop_with_parent();
        let trees = Trees {
            workspace: root.clone(),
            binary: root.join("dx"),
            trees: vec![Tree {
                dir: root.join("tree"),
                output_base: root.join("base"),
                branch: None,
            }],
        };
        fs::write(root.join("ready"), b"").unwrap();
        let err = loop {
            if let Err(err) = check_interrupted() {
                break err;
            }
            thread::sleep(Duration::from_millis(20));
        };
        drop(trees);
        finish(Err(err));
    }

    fn fixture(name: &str) -> (PathBuf, Child) {
        let root = std::env::temp_dir().join(format!("pw-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("tree")).unwrap();
        fs::create_dir_all(root.join("base/server")).unwrap();
        let server = Command::new("sh")
            .args(["-c", "while :; do sleep 0.1; done"])
            .arg(root.join("base"))
            .spawn()
            .unwrap();
        fs::write(
            root.join("base/server/server.pid.txt"),
            server.id().to_string(),
        )
        .unwrap();
        (root, server)
    }

    fn driver(root: &Path) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "tests::driver_child_process", "--test-threads=1"])
            .env(CHILD_ENV, root)
            .stdout(Stdio::null());
        command
    }

    fn wait_child(child: &mut Child) -> std::process::ExitStatus {
        let deadline = Instant::now() + WAIT;
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return status;
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                panic!("process {} did not exit", child.id());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn wait_ready(root: &Path) {
        let deadline = Instant::now() + WAIT;
        while !root.join("ready").exists() {
            assert!(Instant::now() < deadline, "driver never got ready");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn assert_cleaned_up(root: &Path, server: &mut Child) {
        assert_eq!(wait_child(server).signal(), Some(SIGTERM));
        let deadline = Instant::now() + WAIT;
        while root.join("tree").exists() || root.join("base").exists() {
            assert!(
                Instant::now() < deadline,
                "worktree or output base left behind"
            );
            thread::sleep(Duration::from_millis(20));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn sigterm_stops_servers_removes_worktrees_and_fails() {
        let (root, mut server) = fixture("sigterm");
        let mut child = driver(&root).spawn().unwrap();
        wait_ready(&root);
        unsafe {
            kill(child.id() as i32, SIGTERM);
        }
        assert_eq!(wait_child(&mut child).code(), Some(130));
        assert_cleaned_up(&root, &mut server);
    }

    #[test]
    fn parent_death_stops_driver() {
        let (root, mut server) = fixture("parent");
        let script = r#""$0" --exact tests::driver_child_process --test-threads=1 >/dev/null &
while [ ! -e "$1/ready" ]; do sleep 0.05; done"#;
        let mut parent = Command::new("sh")
            .arg("-c")
            .arg(script)
            .arg(std::env::current_exe().unwrap())
            .arg(&root)
            .env(CHILD_ENV, &root)
            .spawn()
            .unwrap();
        assert!(wait_child(&mut parent).success());
        assert_cleaned_up(&root, &mut server);
    }

    #[test]
    fn refuses_low_memory_unless_forced() {
        let need = MIN_AVAILABLE_KIB_PER_WORKTREE * 2;
        assert!(check_memory(Some(need), 2, false).is_ok());
        let err = check_memory(Some(need - 1), 2, false).unwrap_err();
        assert!(err.contains("2 worktrees need 6.0 GiB"), "{err}");
        assert!(err.contains("--force"), "{err}");
        assert!(check_memory(Some(0), 2, true).is_ok());
        assert!(check_memory(None, 1, false).is_err());
        assert!(check_memory(None, 1, true).is_ok());
    }

    #[test]
    fn reads_mem_available() {
        let meminfo = "MemTotal:       16000000 kB\nMemFree:  100 kB\nMemAvailable:    2048 kB\n";
        assert_eq!(parse_mem_available(meminfo), Some(2048));
        assert_eq!(parse_mem_available("MemTotal: 1 kB\n"), None);
    }

    #[test]
    fn reads_slot_path() {
        let json = concat!(
            r#"{"command":"worktree","event":"command_started"}"#,
            "\n",
            r#"{"action":"path","branch":"b","event":"slot","path":"/cache/slots/25","slot":25}"#,
            "\n",
            r#"{"event":"command_finished","exit_code":0}"#,
        );
        assert_eq!(slot_path(json), Some("/cache/slots/25".to_string()));
        assert_eq!(slot_path(r#"{"event":"command_finished"}"#), None);
    }

    #[test]
    fn parses_force() {
        let Ok(Mode::Run(options)) = parse_args(&args(&["--force"])) else {
            panic!("expected run");
        };
        assert!(options.force);
    }

    #[test]
    fn spots_leftovers_of_dead_runs() {
        assert!(!is_stale(&format!("{NAME_PREFIX}{}-0", std::process::id())));
        assert!(is_stale(&format!("{NAME_PREFIX}{}-0", u32::MAX)));
        assert!(is_stale(&format!("{NAME_PREFIX}x-0")));
        assert!(!is_stale("feature-x"));
    }

    fn args(items: &[&str]) -> Vec<String> {
        items.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn defaults_to_four_worktrees() {
        let Ok(Mode::Run(options)) = parse_args(&[]) else {
            panic!("expected run");
        };
        assert_eq!(options.worktrees, 4);
        assert_eq!(options.target, "//...");
        assert!(!options.isolated_repo_cache);
        assert!(!options.force);
    }

    #[test]
    fn parses_isolated_repo_cache() {
        let Ok(Mode::Run(options)) = parse_args(&args(&["--isolated-repo-cache"])) else {
            panic!("expected run");
        };
        assert!(options.isolated_repo_cache);
    }

    #[test]
    fn parses_flags() {
        let Ok(Mode::Run(options)) = parse_args(&args(&[
            "--worktrees",
            "2",
            "--target",
            "//a:b",
            "--out",
            "x.json",
        ])) else {
            panic!("expected run");
        };
        assert_eq!(options.worktrees, 2);
        assert_eq!(options.target, "//a:b");
        assert_eq!(options.out, Some(PathBuf::from("x.json")));
    }

    #[test]
    fn parses_compare() {
        let Ok(Mode::Compare(a, b)) = parse_args(&args(&["--compare", "a", "b"])) else {
            panic!("expected compare");
        };
        assert_eq!((a, b), (PathBuf::from("a"), PathBuf::from("b")));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_args(&args(&["--worktrees", "0"])).is_err());
        assert!(parse_args(&args(&["--worktrees"])).is_err());
        assert!(parse_args(&args(&["--nope"])).is_err());
        assert!(parse_args(&args(&["--compare", "a"])).is_err());
    }

    #[test]
    fn runs_dx_test_in_its_own_output_base_without_caches() {
        let Ok(Mode::Run(options)) = parse_args(&args(&["--target", "//cli/..."])) else {
            panic!("expected run");
        };
        assert_eq!(
            dx_test_args(&options, Path::new("/base")),
            args(&[
                "test",
                "--bazel-startup-option=--output_base=/base",
                "--output=text",
                "//cli/...",
                "--",
                "--keep_going",
                "--disk_cache=",
                "--remote_cache=",
            ])
        );
        let Ok(Mode::Run(options)) = parse_args(&args(&["--isolated-repo-cache"])) else {
            panic!("expected run");
        };
        assert_eq!(
            dx_test_args(&options, Path::new("/base")).last().unwrap(),
            "--repository_cache=/base/load-repo-cache"
        );
    }

    #[test]
    fn dx_runs_outside_the_callers_slot() {
        let command = dx(Path::new("/tree"), Path::new("/bin/dx"));
        let removed: Vec<_> = command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect();
        for name in DX_CHILD_ENV_REMOVED {
            assert!(removed.iter().any(|key| key == name), "{name} kept");
        }
        assert_eq!(command.get_current_dir(), Some(Path::new("/tree")));
    }

    fn bazel_argv(policy: &RcPolicy) -> (String, Vec<String>) {
        let command = bazel(Path::new("/tree"), Path::new("/base"), policy);
        let argv = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        (command.get_program().to_string_lossy().into_owned(), argv)
    }

    #[test]
    fn restart_needs_a_new_live_server() {
        assert!(!restarted(5, Some(5), true));
        assert!(restarted(5, Some(6), true));
        assert!(!restarted(5, Some(6), false));
        assert!(!restarted(5, None, false));
    }

    #[test]
    fn bazel_uses_the_startup_flags_dx_uses() {
        assert_eq!(
            bazel_argv(&RcPolicy::hermetic()),
            (
                "bazel".to_string(),
                args(&["--nohome_rc", "--nosystem_rc", "--output_base=/base"])
            )
        );
        let local = RcPolicy {
            read_user_rc: true,
            disk_cache: None,
            bazel_disk_cache: None,
            local_cpu: true,
            local_memory: true,
            disk_cache_gc: false,
            gc_idle_delay_secs: None,
        };
        assert_eq!(
            bazel_argv(&local),
            ("bazel".to_string(), args(&["--output_base=/base"]))
        );
    }

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn rc_policy_matches_the_dx_child() {
        let local = rc_policy(
            &env_of(&[("BAZEL_TEST", "1"), ("HOME", "/h")]),
            Path::new("/tree"),
            &|_| None,
        );
        assert_eq!(
            local,
            RcPolicy {
                read_user_rc: true,
                disk_cache: Some("/h/.cache/dx/disk-cache".to_string()),
                bazel_disk_cache: Some("/h/.cache/dx/disk-cache".into()),
                local_cpu: true,
                local_memory: true,
                disk_cache_gc: true,
                gc_idle_delay_secs: None,
            }
        );
        let ci = rc_policy(
            &env_of(&[("CI", "true"), ("HOME", "/h")]),
            Path::new("/tree"),
            &|_| None,
        );
        assert_eq!(ci, RcPolicy::hermetic());
    }

    #[test]
    fn disk_cache_follows_the_last_flag() {
        let shared = RcPolicy {
            read_user_rc: true,
            disk_cache: Some("/h/disk".to_string()),
            bazel_disk_cache: Some("/h/disk".into()),
            local_cpu: true,
            local_memory: true,
            disk_cache_gc: true,
            gc_idle_delay_secs: None,
        };
        let Ok(Mode::Run(options)) = parse_args(&[]) else {
            panic!("expected run");
        };
        assert_eq!(
            disk_cache(&shared, &dx_test_args(&options, Path::new("/base"))),
            None
        );
        assert_eq!(
            disk_cache(&shared, &args(&["test", "--keep_going"])),
            Some("/h/disk".to_string())
        );
        assert_eq!(disk_cache(&RcPolicy::hermetic(), &args(&["test"])), None);
    }

    #[test]
    fn spots_slot_waits_in_logs() {
        assert!(waited_for_slot(
            "Loading\ndx: waiting for a build slot (held by /w/a)\nINFO: done"
        ));
        assert!(!waited_for_slot("INFO: Build completed successfully"));
    }

    #[test]
    fn sizes_directories() {
        let dir = std::env::temp_dir().join(format!("pw-size-{}", std::process::id()));
        fs::create_dir_all(dir.join("a")).unwrap();
        fs::write(dir.join("a/f"), b"12345").unwrap();
        fs::write(dir.join("g"), b"123").unwrap();
        assert!(dir_size(&dir) >= 8);
        fs::remove_dir_all(&dir).unwrap();
    }
}
