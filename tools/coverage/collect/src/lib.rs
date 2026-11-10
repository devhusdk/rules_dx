//! Collects LLVM coverage for tests with one llvm-profdata and llvm-cov pair.

#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod apple;
mod collect;
mod gcov;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

macro_rules! debug_log {
    ($($arg:tt)*) => {
        if env::var("VERBOSE_COVERAGE").is_ok() {
            eprintln!($($arg)*);
        }
    };
}

struct Context {
    coverage_dir: PathBuf,
    execroot: PathBuf,
    runfiles_dir: Option<PathBuf>,
}

impl Context {
    fn tool(&self, path: &str) -> PathBuf {
        let in_execroot = self.execroot.join(path);
        match self.runfiles_dir {
            Some(ref rd) if !in_execroot.exists() => {
                debug_log!("Tool not in execroot, falling back to runfiles: {}", path);
                rd.join(path)
            }
            _ => in_execroot,
        }
    }

    fn test_binary(&self) -> PathBuf {
        let binary = required("TEST_BINARY");
        match self.runfiles_dir {
            Some(ref rd) => {
                let in_runfiles = rd.join(required("TEST_WORKSPACE")).join(&binary);
                if in_runfiles.exists() {
                    return in_runfiles;
                }
                let configuration: PathBuf = rd
                    .strip_prefix(&self.execroot)
                    .unwrap_or(rd)
                    .components()
                    .take(3)
                    .collect();
                self.execroot.join(configuration).join(binary)
            }
            None => {
                let rel = self
                    .coverage_dir
                    .strip_prefix(&self.execroot)
                    .unwrap_or(&self.coverage_dir);
                let config: PathBuf = rel.components().take(2).collect();
                self.execroot.join(config).join("bin").join(binary)
            }
        }
    }

    fn manifest_objects(&self) -> Vec<PathBuf> {
        let Ok(manifest) = env::var("COVERAGE_MANIFEST") else {
            return Vec::new();
        };
        let text = read_text(&collect::resolve(&self.execroot, &manifest));
        let mut objects: Vec<PathBuf> = Vec::new();
        for list in collect::runtime_object_lists(&text) {
            let list_text = read_text(&collect::resolve(&self.execroot, &list));
            for line in collect::object_lines(&list_text) {
                let Some(object) = self.object(&line) else {
                    debug_log!("Skipping missing object {}", line);
                    continue;
                };
                if !objects.contains(&object) {
                    objects.push(object);
                }
            }
        }
        objects
    }

    fn object(&self, line: &str) -> Option<PathBuf> {
        let in_execroot = collect::resolve(&self.execroot, line);
        if in_execroot.exists() {
            return Some(in_execroot);
        }
        let rd = self.runfiles_dir.as_ref()?;
        let in_runfiles = rd.join(env::var("TEST_WORKSPACE").ok()?).join(line);
        in_runfiles.exists().then_some(in_runfiles)
    }
}

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| fail(&format!("{name} is not set")))
}

fn fail(message: &str) -> ! {
    eprintln!("dx coverage collector: {message}");
    process::exit(1);
}

fn read_text(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| fail(&format!("cannot read {}: {e}", path.display())))
}

fn merge(profdata: &Path, inputs: &[PathBuf], output: &Path) -> Result<(), String> {
    let mut cmd = Command::new(profdata);
    cmd.arg("merge")
        .arg("--sparse")
        .args(inputs)
        .arg("--output")
        .arg(output);
    debug_log!("Spawning {:#?}", cmd);
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run {}: {e}", profdata.display()))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn export(ctx: &Context, cov: &Path, objects: &[PathBuf], profile: &Path) -> String {
    let mut cmd = Command::new(cov);
    cmd.args(collect::export_args(profile, &ctx.execroot, objects))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    debug_log!("Spawning {:#?}", cmd);
    let out = cmd
        .output()
        .unwrap_or_else(|e| fail(&format!("cannot run {}: {e}", cov.display())));
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("no coverage data found") {
            eprintln!("dx coverage collector: llvm-cov found no coverage data in {objects:?}");
            return String::new();
        }
        fail(&format!("llvm-cov export failed:\n{stderr}"));
    }
    let report =
        String::from_utf8(out.stdout).unwrap_or_else(|_| fail("llvm-cov export wrote non-UTF-8"));
    let report = collect::rewrite_paths(&report, &ctx.execroot);
    for source in report.lines().filter(|line| line.starts_with("SF:")) {
        debug_log!("{source}");
    }
    report
}

// LCOV_EXCL_START - reason: process orchestration over env and the gcc gcov tool, issue: 5889
fn collect_gcov(ctx: &Context, gcov: &str) -> String {
    let manifest = env::var("COVERAGE_MANIFEST")
        .map(|m| read_text(&collect::resolve(&ctx.execroot, &m)))
        .unwrap_or_default();
    let notes = gcov::gcno_by_key(&manifest);
    let mut gcdas = Vec::new();
    gcov::gcda_files(&ctx.coverage_dir, &mut gcdas);
    let tool = ctx.tool(gcov);
    let mut report = gcov::Report::new();
    for gcda in gcdas {
        let Some(key) = gcov::object_key(&gcda.to_string_lossy()) else {
            continue;
        };
        let Some(note) = notes.get(&key).and_then(|n| ctx.object(n)) else {
            debug_log!("No .gcno for {}", gcda.display());
            continue;
        };
        let dir = gcda.parent().unwrap_or(&ctx.coverage_dir);
        let copied = gcda.with_extension("gcno");
        fs::copy(&note, &copied)
            .unwrap_or_else(|e| fail(&format!("cannot copy {}: {e}", note.display())));
        // The toolchain gcov wrapper drops stdout while COVERAGE_DIR is set.
        let out = Command::new(&tool)
            .args(gcov::gcov_args(dir, &gcda))
            .current_dir(&ctx.execroot)
            .env_remove("COVERAGE_DIR")
            .output()
            .unwrap_or_else(|e| fail(&format!("cannot run {}: {e}", tool.display())));
        if !out.status.success() {
            fail(&format!(
                "gcov failed:\n{}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        if let Some(problem) = gcov::gcov_problem(&String::from_utf8_lossy(&out.stderr)) {
            fail(&format!("gcov rejected {}: {problem}", gcda.display()));
        }
        gcov::add_gcov_json(&mut report, &String::from_utf8_lossy(&out.stdout))
            .unwrap_or_else(|e| fail(&e));
    }
    gcov::render(&report)
}

// LCOV_EXCL_STOP

// LCOV_EXCL_START - reason: process orchestration over env and external llvm tools, covered by native_coverage_test, issue: 3460, policy: docs/cli/commands/build-test-coverage.md
/// Collects coverage for the test that just ran and returns the exit code.
pub fn run_process(_args: Vec<String>) -> i32 {
    let coverage_dir = PathBuf::from(required("COVERAGE_DIR"));
    let execroot = PathBuf::from(required("ROOT"));
    let runfiles_dir = env::var("RUNFILES_DIR").ok().map(|d| {
        let p = PathBuf::from(d);
        if p.is_absolute() { p } else { execroot.join(p) }
    });
    let ctx = Context {
        coverage_dir,
        execroot,
        runfiles_dir,
    };
    let output = ctx.coverage_dir.join("coverage.dat");
    if let Ok(reported) = env::var("COVERAGE_OUTPUT_FILE") {
        let reported = collect::resolve(&ctx.execroot, &reported);
        let written = fs::metadata(&reported)
            .map(|m| m.len() > 0)
            .unwrap_or(false);
        if let Some(kept) =
            collect::preserved_report(&reported, &ctx.coverage_dir).filter(|_| written)
        {
            fs::copy(&reported, &kept)
                .unwrap_or_else(|e| fail(&format!("cannot keep the test report: {e}")));
        }
    }

    if let Ok(gcov) = env::var("GCOV") {
        let report = collect_gcov(&ctx, &gcov);
        fs::write(&output, report).unwrap_or_else(|e| fail(&format!("cannot write report: {e}")));
        return 0;
    }

    let mut profraw: Vec<PathBuf> = fs::read_dir(&ctx.coverage_dir)
        .unwrap_or_else(|e| fail(&format!("cannot list COVERAGE_DIR: {e}")))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "profraw"))
        .collect();
    profraw.sort();
    if profraw.is_empty() {
        eprintln!(
            "dx coverage collector: no .profraw files in {}; writing an empty report",
            ctx.coverage_dir.display()
        );
        fs::write(&output, "").unwrap_or_else(|e| fail(&format!("cannot write report: {e}")));
        return 0;
    }

    let profdata = ctx.tool(&required("LLVM_PROFDATA"));
    let cov = ctx.tool(&required("LLVM_COV"));
    let mut objects = ctx.manifest_objects();
    if env::var("RUST_LLVM_COV").is_ok()
        || env::var("DX_COVERAGE_CLANG_OBJECTS").as_deref() == Ok("test_binary")
    {
        let binary = ctx.test_binary();
        if !objects.contains(&binary) {
            objects.push(binary);
        }
    }

    let merged = ctx.coverage_dir.join("_dx_merged.profdata");
    merge(&profdata, &profraw, &merged).unwrap_or_else(|stderr| {
        fail(&format!(
            "llvm-profdata merge failed ({}):\n{stderr}",
            profdata.display()
        ))
    });
    let report = if objects.is_empty() {
        eprintln!(
            "dx coverage collector: no instrumented objects to export; writing an empty report"
        );
        String::new()
    } else {
        export(&ctx, &cov, &objects, &merged)
    };

    debug_log!("Writing output to {}", output.display());
    fs::write(&output, report).unwrap_or_else(|e| fail(&format!("cannot write report: {e}")));
    fs::remove_file(&merged).ok();
    0
}
// LCOV_EXCL_STOP
