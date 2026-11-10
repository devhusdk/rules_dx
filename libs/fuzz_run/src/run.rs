//! Argument parsing, runfiles lookup and the per-engine command lines.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const USAGE: &str =
    "usage: bazel run <name>_run -- [--time <seconds>] [--out <dir>] [--jobs <n>] [--repro <file>]";
const CRASH_EXIT: i32 = 77;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Engine {
    Libfuzzer,
    Jazzer,
    Go,
    Dotnet,
    Crowbar,
}

impl Engine {
    fn parse(name: &str) -> Option<Engine> {
        match name {
            "libfuzzer" => Some(Engine::Libfuzzer),
            "jazzer" => Some(Engine::Jazzer),
            "go" => Some(Engine::Go),
            "dotnet" => Some(Engine::Dotnet),
            "crowbar" => Some(Engine::Crowbar),
            _ => None,
        }
    }

    fn takes_target(self) -> bool {
        !matches!(self, Engine::Libfuzzer | Engine::Crowbar)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub seconds: String,
    pub out: Option<String>,
    pub jobs: Option<String>,
    pub repro: Option<String>,
}

pub fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        seconds: "60".to_owned(),
        out: None,
        jobs: None,
        repro: None,
    };
    let mut iter = args.iter();
    while let Some(flag) = iter.next() {
        let slot = match flag.as_str() {
            "--time" => &mut options.seconds,
            "--out" => options.out.get_or_insert_with(String::new),
            "--jobs" => options.jobs.get_or_insert_with(String::new),
            "--repro" => options.repro.get_or_insert_with(String::new),
            _ => return Err(USAGE.to_owned()),
        };
        *slot = iter.next().ok_or_else(|| USAGE.to_owned())?.clone();
    }
    Ok(options)
}

/// The command line for fuzzing, without the program.
pub fn fuzz_args(
    engine: Engine,
    target: &str,
    options: &Options,
    work: &Path,
    crashes: &Path,
) -> Vec<String> {
    let artifacts = format!("-artifact_prefix={}/", crashes.display());
    let fork = options.jobs.as_ref().map(|jobs| format!("-fork={jobs}"));
    let corpus = work.join("corpus").display().to_string();
    let seconds = &options.seconds;
    let mut args = Vec::new();
    match engine {
        Engine::Libfuzzer => {
            args.push(format!("-max_total_time={seconds}"));
            args.push(artifacts);
            args.extend(fork);
            args.push(corpus);
        }
        Engine::Dotnet => {
            args.push(format!("--target_path={target}"));
            args.push(format!("-max_total_time={seconds}"));
            args.push(artifacts);
            args.extend(fork);
            args.push(corpus);
        }
        Engine::Jazzer => {
            args.push(format!("--target_class={target}"));
            args.push(format!("-max_total_time={seconds}"));
            args.push(artifacts);
            args.extend(fork);
            args.push(corpus);
        }
        Engine::Crowbar => {}
        Engine::Go => {
            args.push(format!("-test.fuzz=^{target}$"));
            args.push("-test.run=^$".to_owned());
            args.push(format!("-test.fuzztime={seconds}s"));
            args.push(format!(
                "-test.fuzzcachedir={}",
                work.join("cache").display()
            ));
            args.extend(
                options
                    .jobs
                    .as_ref()
                    .map(|jobs| format!("-test.parallel={jobs}")),
            );
        }
    }
    args
}

/// The command line for replaying one crash file, without the program.
pub fn repro_args(engine: Engine, target: &str, repro: &Path) -> Vec<String> {
    let repro = repro.display().to_string();
    match engine {
        Engine::Libfuzzer => vec![repro],
        Engine::Dotnet => vec![format!("--target_path={target}"), repro],
        Engine::Jazzer => vec![format!("--target_class={target}"), repro],
        Engine::Go => vec![format!("-test.run=^{target}$")],
        Engine::Crowbar => crowbar_args(&fs::read_to_string(repro).unwrap_or_default()),
    }
}

const CROWBAR_RUNS: &str = "500";

/// The command line for one Crowbar round with the seed given as text.
pub fn crowbar_args(seed: &str) -> Vec<String> {
    vec![
        "-s".to_owned(),
        seed.trim().to_owned(),
        "-r".to_owned(),
        CROWBAR_RUNS.to_owned(),
    ]
}

fn random_seed() -> i64 {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u32(std::process::id());
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos()),
    );
    (hasher.finish() >> 1) as i64
}

/// Runs Crowbar rounds with fresh seeds until the time is up; a failing seed is written to `crashes`.
fn crowbar_rounds(
    spawn: &dyn Fn(Vec<String>, Option<&Path>) -> Result<i32, Failure>,
    options: &Options,
    crashes: &Path,
) -> Result<i32, Failure> {
    let seconds: u64 = options.seconds.parse().map_err(|_| usage())?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    loop {
        let seed = random_seed();
        let status = spawn(crowbar_args(&seed.to_string()), None)?;
        if status != 0 {
            let name = format!("{:016x}", seed.unsigned_abs());
            fs::write(crashes.join(name), seed.to_string())
                .map_err(|e| (1, format!("fuzz runner: seed: {e}")))?;
            return Ok(status);
        }
        if std::time::Instant::now() >= deadline {
            return Ok(0);
        }
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A path given on the command line, or a runfiles key.
fn locate(arg: &str) -> PathBuf {
    let path = Path::new(arg);
    if path.is_absolute() || path.exists() {
        return absolute(path);
    }
    runfiles::Runfiles::create()
        .ok()
        .and_then(|runfiles| runfiles.rlocation(arg))
        .filter(|found| found.exists())
        .map_or_else(|| absolute(path), |found| absolute(&found))
}

/// A path the user gave, read from the workspace the command was run in.
fn from_user(arg: &str) -> PathBuf {
    let path = Path::new(arg);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let base = std::env::var_os("BUILD_WORKSPACE_DIRECTORY")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    base.join(path)
}

fn make_work_dir() -> std::io::Result<PathBuf> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let dir = std::env::temp_dir().join(format!("dx-fuzz-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn exit_code(status: std::process::ExitStatus) -> i32 {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    status.code().unwrap_or(1)
}

pub fn main_code(args: Vec<String>) -> i32 {
    match run(&args) {
        Ok(code) => code,
        Err((code, message)) => {
            eprintln!("{message}");
            code
        }
    }
}

type Failure = (i32, String);

fn usage() -> Failure {
    (2, USAGE.to_owned())
}

fn run(args: &[String]) -> Result<i32, Failure> {
    let (engine_name, rest) = args.split_first().ok_or_else(usage)?;
    let (bin, rest) = rest.split_first().ok_or_else(usage)?;
    let engine = Engine::parse(engine_name)
        .ok_or_else(|| (2, format!("unknown fuzz engine: {engine_name}")))?;
    let (target, rest) = if engine.takes_target() {
        let (target, rest) = rest.split_first().ok_or_else(usage)?;
        (target.as_str(), rest)
    } else {
        ("", rest)
    };
    let bin = locate(bin);
    let target = if engine == Engine::Dotnet {
        locate(target).display().to_string()
    } else {
        target.to_owned()
    };
    let options = parse_options(rest).map_err(|message| (2, message))?;

    let work = make_work_dir().map_err(|error| (1, format!("fuzz runner: temp dir: {error}")))?;
    let result = execute(engine, &bin, &target, &options, &work);
    let _ = fs::remove_dir_all(&work);
    result
}

fn execute(
    engine: Engine,
    bin: &Path,
    target: &str,
    options: &Options,
    work: &Path,
) -> Result<i32, Failure> {
    let io_error = |what: &str, error: std::io::Error| (1, format!("fuzz runner: {what}: {error}"));
    let mut crashes = work.join("artifacts");
    if engine == Engine::Go {
        crashes = work.join("testdata").join("fuzz").join(target);
    } else {
        fs::create_dir_all(work.join("corpus")).map_err(|e| io_error("corpus", e))?;
    }
    fs::create_dir_all(&crashes).map_err(|e| io_error("artifacts", e))?;

    let in_work = matches!(engine, Engine::Go | Engine::Jazzer);
    let spawn = |args: Vec<String>, cwd: Option<&Path>| -> Result<i32, Failure> {
        let mut command = Command::new(bin);
        command.args(args);
        if engine == Engine::Dotnet && std::env::var_os("RUNFILES_DIR").is_none() {
            if let Some(parent) = std::env::current_dir()
                .ok()
                .and_then(|cwd| cwd.parent().map(Path::to_path_buf))
            {
                command.env("RUNFILES_DIR", parent);
            }
        }
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        match command.status() {
            Ok(status) => Ok(exit_code(status)),
            Err(error) => Err((127, format!("fuzz runner: {}: {error}", bin.display()))),
        }
    };

    if let Some(repro) = &options.repro {
        let repro = from_user(repro);
        if engine == Engine::Go {
            let name = repro.file_name().unwrap_or_default();
            fs::copy(&repro, crashes.join(name)).map_err(|e| io_error("repro", e))?;
        }
        let cwd = (engine == Engine::Go).then_some(work);
        return spawn(repro_args(engine, target, &repro), cwd);
    }

    let out = options.out.as_deref().ok_or_else(usage)?;
    let out = from_user(out);
    fs::create_dir_all(&out).map_err(|e| io_error("out", e))?;
    let status = if engine == Engine::Crowbar {
        crowbar_rounds(&spawn, options, &crashes)?
    } else {
        spawn(
            fuzz_args(engine, target, options, work, &crashes),
            in_work.then_some(work),
        )?
    };

    let mut found = false;
    for entry in fs::read_dir(&crashes)
        .map_err(|e| io_error("artifacts", e))?
        .flatten()
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let mut name = std::ffi::OsString::from("crash-");
        name.push(entry.file_name());
        fs::copy(&path, out.join(name)).map_err(|e| io_error("crash copy", e))?;
        found = true;
    }
    Ok(if found { CRASH_EXIT } else { status })
}
