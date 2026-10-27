//! Collects LLVM coverage for tests whose profraw files come from more than one LLVM version.

mod collect;

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

macro_rules! debug_log {
    ($($arg:tt)*) => {
        if env::var("VERBOSE_COVERAGE").is_ok() {
            eprintln!($($arg)*);
        }
    };
}

enum Objects {
    TestBinary,
    Manifest,
}

struct PairSpec {
    name: &'static str,
    profdata_env: &'static str,
    cov_env: &'static str,
    objects: Objects,
}

const PAIRS: &[PairSpec] = &[
    PairSpec {
        name: "rust",
        profdata_env: "RUST_LLVM_PROFDATA",
        cov_env: "RUST_LLVM_COV",
        objects: Objects::TestBinary,
    },
    PairSpec {
        name: "clang",
        profdata_env: "LLVM_PROFDATA",
        cov_env: "LLVM_COV",
        objects: Objects::Manifest,
    },
];

struct Pair {
    name: &'static str,
    profdata: PathBuf,
    cov: PathBuf,
    objects: Vec<PathBuf>,
    accepted: Vec<PathBuf>,
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
                    .expect("RUNFILES_DIR should be relative to ROOT")
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

fn profraw_version(path: &Path) -> u32 {
    let mut header = [0u8; 16];
    let read = fs::File::open(path).and_then(|mut f| f.read_exact(&mut header));
    if let Err(e) = read {
        fail(&format!("cannot read {}: {e}", path.display()));
    }
    collect::raw_profile_version(&header)
        .unwrap_or_else(|| fail(&format!("{} is not an LLVM raw profile", path.display())))
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

fn export(ctx: &Context, pair: &Pair, profile: &Path) -> String {
    let mut cmd = Command::new(&pair.cov);
    cmd.args(collect::export_args(profile, &ctx.execroot, &pair.objects))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    debug_log!("Spawning {:#?}", cmd);
    let out = cmd
        .output()
        .unwrap_or_else(|e| fail(&format!("cannot run {}: {e}", pair.cov.display())));
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("no coverage data found") {
            debug_log!("No {} coverage data found", pair.name);
            return String::new();
        }
        fail(&format!("{} llvm-cov export failed:\n{stderr}", pair.name));
    }
    let report = String::from_utf8(out.stdout)
        .unwrap_or_else(|_| fail(&format!("{} llvm-cov export wrote non-UTF-8", pair.name)));
    collect::rewrite_paths(&report, &ctx.execroot)
}

fn main() {
    let coverage_dir = PathBuf::from(required("COVERAGE_DIR"));
    let execroot = PathBuf::from(required("ROOT"));
    let runfiles_dir = env::var("RUNFILES_DIR").ok().map(|d| {
        let p = PathBuf::from(d);
        if p.is_absolute() {
            p
        } else {
            execroot.join(p)
        }
    });
    let ctx = Context {
        coverage_dir,
        execroot,
        runfiles_dir,
    };
    let output = ctx.coverage_dir.join("coverage.dat");

    let mut profraw: Vec<PathBuf> = fs::read_dir(&ctx.coverage_dir)
        .unwrap_or_else(|e| fail(&format!("cannot list COVERAGE_DIR: {e}")))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "profraw"))
        .collect();
    profraw.sort();
    if profraw.is_empty() {
        debug_log!("No .profraw files in COVERAGE_DIR; writing empty report");
        fs::write(&output, "").unwrap_or_else(|e| fail(&format!("cannot write report: {e}")));
        return;
    }

    let groups = collect::group_by_version(
        profraw
            .into_iter()
            .map(|path| {
                let version = profraw_version(&path);
                (path, version)
            })
            .collect(),
    );

    let mut pairs: Vec<Pair> = PAIRS
        .iter()
        .filter_map(|spec| {
            let profdata = env::var(spec.profdata_env).ok()?;
            let cov = env::var(spec.cov_env).ok()?;
            let objects = match spec.objects {
                Objects::TestBinary => vec![ctx.test_binary()],
                Objects::Manifest => {
                    let objects = ctx.manifest_objects();
                    if objects.is_empty()
                        && env::var("DX_COVERAGE_CLANG_OBJECTS").as_deref() == Ok("test_binary")
                    {
                        vec![ctx.test_binary()]
                    } else {
                        objects
                    }
                }
            };
            Some(Pair {
                name: spec.name,
                profdata: ctx.tool(&profdata),
                cov: ctx.tool(&cov),
                objects,
                accepted: Vec::new(),
            })
        })
        .collect();
    if pairs.is_empty() {
        fail("no llvm-profdata/llvm-cov pair is set");
    }

    let mut intermediates: Vec<PathBuf> = Vec::new();
    for (version, files) in &groups {
        let mut errors: Vec<String> = Vec::new();
        for pair in pairs.iter_mut() {
            let out = ctx
                .coverage_dir
                .join(format!("_dx_{}_v{version}.profdata", pair.name));
            match merge(&pair.profdata, files, &out) {
                Ok(()) => {
                    intermediates.push(out.clone());
                    pair.accepted.push(out);
                }
                Err(stderr) => errors.push(format!(
                    "{} ({}):\n{stderr}",
                    pair.name,
                    pair.profdata.display()
                )),
            }
        }
        if errors.len() == pairs.len() {
            let names: Vec<String> = files.iter().map(|f| f.display().to_string()).collect();
            fail(&format!(
                "no llvm-profdata accepts raw profile version {version} in {}\n{}",
                names.join(", "),
                errors.join("\n")
            ));
        }
    }

    let mut report = String::new();
    for pair in &pairs {
        if pair.accepted.is_empty() {
            continue;
        }
        if pair.objects.is_empty() {
            debug_log!("No {} objects to export", pair.name);
            continue;
        }
        let profile = if pair.accepted.len() == 1 {
            pair.accepted[0].clone()
        } else {
            let combined = ctx.coverage_dir.join(format!("_dx_{}.profdata", pair.name));
            merge(&pair.profdata, &pair.accepted, &combined)
                .unwrap_or_else(|stderr| fail(&format!("{} merge failed:\n{stderr}", pair.name)));
            intermediates.push(combined.clone());
            combined
        };
        report.push_str(&export(&ctx, pair, &profile));
    }

    debug_log!("Writing output to {}", output.display());
    fs::write(&output, report).unwrap_or_else(|e| fail(&format!("cannot write report: {e}")));
    for file in intermediates {
        fs::remove_file(&file).ok();
    }
}
