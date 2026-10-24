mod metrics;

use metrics::{Peaks, Phase};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const USAGE: &str = "usage: parallel_worktrees [--worktrees N] [--target LABEL] [--out FILE]\n       parallel_worktrees --compare FIRST.json SECOND.json";
const SAMPLE_INTERVAL: Duration = Duration::from_millis(500);
const TOLERANCE_PERCENT: f64 = 15.0;
const NAME_PREFIX: &str = "parallel-load-";

static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

extern "C" fn on_signal(_signum: i32) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

fn install_signal_handlers() {
    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;
    let handler = on_signal as extern "C" fn(i32) as usize;
    unsafe {
        signal(SIGINT, handler);
        signal(SIGTERM, handler);
    }
}

fn check_interrupted() -> Result<(), String> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        Err("interrupted".to_string())
    } else {
        Ok(())
    }
}

struct Options {
    worktrees: usize,
    target: String,
    out: Option<PathBuf>,
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

fn bazel(dir: &Path) -> Command {
    let mut command = Command::new("bazel");
    command
        .current_dir(dir)
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .env_remove("BUILD_WORKING_DIRECTORY")
        .stdin(Stdio::null());
    command
}

fn bazel_output(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = bazel(dir)
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

fn git(workspace: &Path, args: &[&str]) -> Result<(), String> {
    let status = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .env_remove("BUILD_WORKSPACE_DIRECTORY")
        .status()
        .map_err(|err| format!("git {}: {err}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git {} failed", args.join(" ")))
    }
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
    if fs::remove_dir_all(path).is_ok() {
        return;
    }
    let _ = Command::new("chmod")
        .args(["-R", "u+rwX"])
        .arg(path)
        .status();
    let _ = fs::remove_dir_all(path);
}

struct Tree {
    dir: PathBuf,
    output_base: Option<PathBuf>,
}

struct Trees {
    workspace: PathBuf,
    trees: Vec<Tree>,
}

impl Trees {
    fn remove(&self, tree: &Tree) {
        let _ = bazel(&tree.dir).args(["clean", "--expunge"]).output();
        let _ = bazel(&tree.dir).arg("shutdown").output();
        if let Some(base) = &tree.output_base {
            remove_tree(base);
        }
        let _ = git(
            &self.workspace,
            &["worktree", "remove", "--force", &tree.dir.to_string_lossy()],
        );
        remove_tree(&tree.dir);
    }
}

impl Drop for Trees {
    fn drop(&mut self) {
        for tree in &self.trees {
            self.remove(tree);
        }
        let _ = git(&self.workspace, &["worktree", "prune"]);
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

fn sweep_stale(workspace: &Path, root: &Path) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let stale = Trees {
        workspace: workspace.to_path_buf(),
        trees: entries
            .flatten()
            .filter(|e| is_stale(&e.file_name().to_string_lossy()))
            .map(|e| Tree {
                dir: e.path(),
                output_base: None,
            })
            .collect(),
    };
    drop(stale);
}

fn create_trees(workspace: &Path, root: &Path, count: usize) -> Result<Trees, String> {
    fs::create_dir_all(root).map_err(|err| format!("{}: {err}", root.display()))?;
    let mut trees = Trees {
        workspace: workspace.to_path_buf(),
        trees: Vec::new(),
    };
    for index in 0..count {
        let dir = root.join(format!("{NAME_PREFIX}{}-{index}", std::process::id()));
        git(
            workspace,
            &[
                "worktree",
                "add",
                "--detach",
                &dir.to_string_lossy(),
                "HEAD",
            ],
        )?;
        trees.trees.push(Tree {
            dir: dir.clone(),
            output_base: None,
        });
        check_interrupted()?;
        let base = bazel_output(&dir, &["info", "output_base"])?;
        if let Some(tree) = trees.trees.last_mut() {
            tree.output_base = Some(PathBuf::from(base));
        }
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

fn run_phase(trees: &Trees, target: &str, label: &str) -> Result<Phase, String> {
    let bases: Vec<PathBuf> = trees
        .trees
        .iter()
        .filter_map(|tree| tree.output_base.clone())
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
                let base = tree
                    .output_base
                    .as_ref()
                    .ok_or_else(|| format!("{}: no output base", tree.dir.display()))?;
                let repo_cache = format!(
                    "--repository_cache={}",
                    base.join("load-repo-cache").display()
                );
                let bep = format!(
                    "--build_event_json_file={}",
                    tree.dir.join(format!("{label}.bep.json")).display()
                );
                bazel(&tree.dir)
                    .args([
                        "test",
                        "--keep_going",
                        "--disk_cache=",
                        "--remote_cache=",
                        &repo_cache,
                        &bep,
                        target,
                    ])
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
                "bazel exited {code} in {label} phase:\n{}",
                tail.into_iter().rev().collect::<Vec<_>>().join("\n")
            );
        }
        exit_codes.push(code);
    }
    let wall_seconds = start.elapsed().as_secs_f64();
    stop.store(true, Ordering::Relaxed);
    let peaks = sampler.join().map_err(|_| "sampler panicked".to_string())?;
    if let Some(err) = spawn_error {
        return Err(format!("could not start bazel: {err}"));
    }
    if let Some(code) = exit_codes.iter().find(|code| !matches!(code, 0 | 3)) {
        return Err(format!("bazel test exited {code} in {label} phase"));
    }
    let jvm_peak_heap_bytes = trees
        .trees
        .iter()
        .map(|tree| {
            fs::read_to_string(tree.dir.join(format!("{label}.bep.json")))
                .map(|text| metrics::parse_peak_heap(&text))
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    Ok(Phase {
        wall_seconds,
        peaks,
        jvm_peak_heap_bytes,
        output_base_bytes: bases.iter().map(|base| dir_size(base)).sum(),
        oom_kills: oom_kills().saturating_sub(oom_before),
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
    let root = workspace.join(".claude/worktrees");
    install_signal_handlers();
    sweep_stale(&workspace, &root);
    let trees = create_trees(&workspace, &root, options.worktrees)?;
    let cold = run_phase(&trees, &options.target, "cold")?;
    let warm = run_phase(&trees, &options.target, "warm")?;
    drop(trees);
    fs::write(
        &out,
        metrics::to_json(options.worktrees, &options.target, &cold, &warm),
    )
    .map_err(|err| format!("{}: {err}", out.display()))?;
    print!("{}", metrics::to_table(&cold, &warm));
    println!("wrote {}", out.display());
    Ok(())
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = parse_args(&args).and_then(|command| match command {
        Mode::Run(options) => run(&options).map(|()| true),
        Mode::Compare(first, second) => compare(&first, &second),
    });
    match result {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("some metrics differ by more than {TOLERANCE_PERCENT}%");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("{err}\n{USAGE}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn sizes_directories() {
        let dir = std::env::temp_dir().join(format!("pw-size-{}", std::process::id()));
        fs::create_dir_all(dir.join("a")).unwrap();
        fs::write(dir.join("a/f"), b"12345").unwrap();
        fs::write(dir.join("g"), b"123").unwrap();
        assert!(dir_size(&dir) >= 8);
        fs::remove_dir_all(&dir).unwrap();
    }
}
