//! Runs one .NET test under coverlet and writes its LCOV report for Bazel coverage.

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

fn coverlet_args(run: &CoverletRun) -> Vec<String> {
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

fn fail(message: &str) -> ! {
    eprintln!("dx: {message}");
    process::exit(1);
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

fn runfiles_entries() -> (Vec<Entry>, Option<PathBuf>) {
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
            .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", manifest.display())));
        return (plan::parse_manifest(&text), dir);
    }
    let Some(dir) = dir else {
        fail("no runfiles: set RUNFILES_DIR or RUNFILES_MANIFEST_FILE");
    };
    let mut entries = Vec::new();
    walk(&dir, &dir, &mut entries)
        .unwrap_or_else(|e| fail(&format!("cannot walk {}: {e}", dir.display())));
    (entries, Some(dir))
}

fn resolve(entries: &[Entry], rlocation: &str) -> PathBuf {
    entries
        .iter()
        .find(|e| e.rlocation == rlocation)
        .and_then(|e| e.target.as_ref())
        .map(PathBuf::from)
        .unwrap_or_else(|| fail(&format!("{rlocation} is not in the runfiles")))
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
    #[allow(clippy::permissions_set_readonly_false)]
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

fn exit_with(status: io::Result<process::ExitStatus>, what: &str) -> ! {
    match status {
        Ok(status) => process::exit(status.code().unwrap_or(1)),
        Err(e) => fail(&format!("cannot run {what}: {e}")),
    }
}

fn main() {
    let target_args: Vec<String> = env::args().skip(1).collect();
    let (entries, runfiles_dir) = runfiles_entries();
    let config_rlocation =
        env::var("DX_COVERLET_CONFIG").unwrap_or_else(|_| fail("DX_COVERLET_CONFIG is not set"));
    let config_path = resolve(&entries, &config_rlocation);
    let config_text = fs::read_to_string(&config_path)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", config_path.display())));
    let config = parse_config(&config_text).unwrap_or_else(|e| fail(&e));

    let coverage_dir = env::var_os("COVERAGE_DIR").map(PathBuf::from);
    let Some(coverage_dir) = coverage_dir.filter(|_| !config.instrument.is_empty()) else {
        let upstream = resolve(&entries, &config.upstream);
        exit_with(
            Command::new(&upstream).args(&target_args).status(),
            &upstream.to_string_lossy(),
        );
    };

    let tmp = env::var_os("TEST_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
        .join("dx_coverlet");
    let root = tmp.join("runfiles");
    let mut assemblies = config.instrument.clone();
    assemblies.push(config.test_assembly.clone());
    let copies = plan::copy_set(&entries, &assemblies);
    build_mirror(&root, &entries, &copies).unwrap_or_else(|e| {
        fail(&format!(
            "cannot mirror runfiles into {}: {e}",
            root.display()
        ))
    });

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
    let test_tmp = env::var_os("TEST_TMPDIR").unwrap_or_else(|| tmp.clone().into_os_string());
    let status = Command::new(&coverlet)
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
        .unwrap_or_else(|e| fail(&format!("cannot run coverlet {coverlet}: {e}")));
    let Some(code) = status.code() else {
        fail("coverlet was killed by a signal");
    };
    let code = match outcome(code) {
        Outcome::CoverletFailed(code) => {
            eprintln!("dx: coverlet failed");
            process::exit(code);
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
    fs::create_dir_all(&coverage_dir).ok();
    let dat = coverage_dir.join("coverlet.dat");
    fs::write(&dat, normalized)
        .unwrap_or_else(|e| fail(&format!("cannot write {}: {e}", dat.display())));
    if code == 0 && kept == 0 {
        fail(&format!(
            "coverlet wrote no SF: records for {}",
            config.instrument.join(", ")
        ));
    }
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

    #[test]
    fn maps_exit_codes() {
        assert_eq!(outcome(0), Outcome::Passed);
        assert_eq!(outcome(1), Outcome::TestsFailed(1));
        assert_eq!(outcome(2), Outcome::TestsFailed(2));
        assert_eq!(outcome(101), Outcome::CoverletFailed(101));
    }
}
