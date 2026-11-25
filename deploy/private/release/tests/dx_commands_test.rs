use crate::quickstart_consumer_test::{init_consumer, nested_env};
use dx_nested::{self as nested, Case};
use dx_testing::{Run, read_text, resolve_runfiles, rules_dx_source, write_text};
use std::path::{Path, PathBuf};
use std::time::Duration;

const TEST: &str = "dx_commands_test";

const AUDIT_TODAY: &str = "2026-01-01";

type CaseFn = fn(&Consumer) -> Result<(), String>;

const CASES: [(&str, CaseFn); 26] = [
    ("init", init),
    ("status", status),
    ("config", config),
    ("completion", completion),
    ("upgrade", upgrade),
    ("worktree", worktree),
    ("license", license),
    ("update", update),
    ("security", security),
    ("build", build),
    ("run", run),
    ("test", test),
    ("fetch", fetch),
    ("coverage", coverage),
    ("bench", bench),
    ("fuzz", fuzz),
    ("deploy", deploy),
    ("docs", docs),
    ("format", format),
    ("lint", lint),
    ("typecheck", typecheck),
    ("generate", generate),
    ("check", check),
    ("watch", watch),
    ("sync", sync),
    ("clean", clean),
];

struct Consumer {
    case: Case,
    dx: PathBuf,
    env: Vec<(&'static str, String)>,
    workspace: String,
    scratch: PathBuf,
}

impl Consumer {
    fn envs(&self) -> Vec<(&str, &str)> {
        self.env
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect()
    }

    fn dx(&self, args: &[&str]) -> Run {
        crate::stage(TEST, &format!("dx {}", args.join(" ")));
        let mut argv: Vec<&str> = args.iter().copied().take(1).collect();
        argv.push(self.workspace.as_str());
        argv.extend(args.iter().skip(1));
        self.case.run(&self.dx, &argv, &self.envs())
    }

    fn write(&self, rel: &str, text: &str) {
        write_text(&self.case.root().join(rel), text);
    }

    fn remove(&self, rel: &str) -> Result<(), String> {
        let path = self.case.root().join(rel);
        std::fs::remove_dir_all(&path)
            .map_err(|error| format!("remove {}: {error}", path.display()))
    }
}

fn tail(run: &Run) -> String {
    let combined = run.combined();
    let lines: Vec<&str> = combined.lines().collect();
    lines
        .iter()
        .skip(lines.len().saturating_sub(40))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

fn expect(consumer: &Consumer, args: &[&str], code: i32, needles: &[&str]) -> Result<Run, String> {
    let run = consumer.dx(args);
    let what = format!("dx {}", args.join(" "));
    if run.status.code() != Some(code) {
        return Err(format!(
            "{what}: exit {:?}, want {code}\n{}",
            run.status.code(),
            tail(&run)
        ));
    }
    let combined = run.combined();
    if let Some(missing) = needles.iter().find(|needle| !combined.contains(**needle)) {
        return Err(format!("{what}: no `{missing}` in\n{}", tail(&run)));
    }
    Ok(run)
}

fn init(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["init"], 0, &["init current"]).map(drop)
}

fn status(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["status"], 0, &["module: ok"]).map(drop)
}

fn config(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["config"], 0, &["buildifier"])?;
    expect(consumer, &["config", "diff"], 2, &["<tool>"]).map(drop)
}

fn completion(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["completion", "bash"], 0, &["_clap_complete_dx"]).map(drop)
}

fn upgrade(consumer: &Consumer) -> Result<(), String> {
    expect(
        consumer,
        &["upgrade", "--to", "0.0.0"],
        0,
        &["upgrade current"],
    )
    .map(drop)
}

fn worktree(consumer: &Consumer) -> Result<(), String> {
    let guard = |command: &str| -> Result<i32, String> {
        use std::io::Write;
        let input = serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": command},
            "cwd": consumer.case.root(),
        });
        crate::stage(TEST, &format!("dx worktree hook guard: {command}"));
        let mut child = consumer
            .case
            .command(&consumer.dx, &consumer.envs())
            .args(["worktree", consumer.workspace.as_str(), "hook", "guard"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|error| format!("spawn dx worktree hook guard: {error}"))?;
        child
            .stdin
            .take()
            .ok_or("no stdin")?
            .write_all(input.to_string().as_bytes())
            .map_err(|error| format!("write the hook input: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("wait for dx worktree hook guard: {error}"))?;
        output.status.code().ok_or_else(|| "killed".to_owned())
    };
    let allowed = guard("ls")?;
    let refused = guard(&["git", "worktree", "add", "elsewhere"].join(" "))?;
    if (allowed, refused) == (0, 2) {
        Ok(())
    } else {
        Err(format!(
            "dx worktree hook guard: exits {allowed} and {refused}, want 0 for ls and 2 for a new worktree"
        ))
    }
}

fn license(consumer: &Consumer) -> Result<(), String> {
    expect(
        consumer,
        &["license", "--offline"],
        0,
        &["audit license: clean"],
    )
    .map(drop)
}

fn update(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["update"], 0, &[]).map(drop)
}

fn security(consumer: &Consumer) -> Result<(), String> {
    let mut pins = read_text(&consumer.case.root().join(".dx/pins"));
    let snapshot = "[]";
    let sha256 = dx_digest::sha256_hex(snapshot.as_bytes());
    for family in ["cargo", "cargo-yanked"] {
        consumer.write(
            &format!(".dx/local/advisory/{sha256}/{family}.json"),
            snapshot,
        );
        pins.push_str(&format!(
            "advisory {family} {AUDIT_TODAY} {sha256} https://example.test/{family}.json\n"
        ));
    }
    consumer.write(".dx/pins", &pins);
    expect(consumer, &["security"], 0, &["audit security: clean"])?;
    consumer.write("leak/BUILD.bazel", "exports_files([\"settings.txt\"])\n");
    consumer.write(
        "leak/settings.txt",
        &["aws_access_key_id = ", "AKIA", "QYLPMN5HHHFPZAM2"].concat(),
    );
    let found = expect(consumer, &["security"], 1, &["settings.txt"]);
    consumer.remove("leak")?;
    found.map(drop)
}

fn build(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["build", "//app/..."], 0, &[]).map(drop)
}

fn run(consumer: &Consumer) -> Result<(), String> {
    expect(
        consumer,
        &["run", "//hello:hello"],
        0,
        &["hello from dx run"],
    )
    .map(drop)
}

fn test(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["test", "//app/..."], 0, &[]).map(drop)
}

fn fetch(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["fetch", "//app/..."], 0, &[]).map(drop)
}

fn coverage(consumer: &Consumer) -> Result<(), String> {
    let lcov = consumer.scratch.join("lcov.info");
    let report = format!("lcov={}", lcov.display());
    expect(
        consumer,
        &[
            "coverage",
            "--report",
            &report,
            "--min-coverage",
            "10",
            "//app/...",
        ],
        0,
        &["meets minimum 10%"],
    )?;
    let text = read_text(&lcov);
    if !text.contains("calc.rs") {
        return Err(format!("{} names no calc.rs:\n{text}", lcov.display()));
    }
    expect(
        consumer,
        &["coverage", "--min-coverage", "100", "//app/..."],
        1,
        &["coverage_below_minimum"],
    )?;
    expect(
        consumer,
        &[
            "coverage",
            "--min-changed-line-coverage",
            "80",
            "--output=json",
            "//app/...",
        ],
        1,
        &["changed_line_coverage", "unavailable"],
    )?;
    expect(
        consumer,
        &["coverage", "--fail-on", "never", "//app/..."],
        2,
        &[],
    )?;
    consumer.write(
        "gate/BUILD.bazel",
        "load(\"@rules_dx//rust:defs.bzl\", \"rust_test\")\n\nrust_test(\n    name = \"fails_test\",\n    size = \"small\",\n    srcs = [\"fails_test.rs\"],\n    crate_root = \"fails_test.rs\",\n)\n\nrust_test(\n    name = \"uncollected_test\",\n    size = \"small\",\n    srcs = [\"uncollected_test.rs\"],\n    crate_root = \"uncollected_test.rs\",\n    tags = [\"dx-no-coverage-collector\"],\n)\n",
    );
    consumer.write(
        "gate/fails_test.rs",
        "//! A test that fails.\n\n#[test]\nfn fails() {\n    assert_eq!(1 + 1, 3);\n}\n",
    );
    consumer.write(
        "gate/uncollected_test.rs",
        "//! A test with no coverage collector.\n\n#[test]\nfn passes() {\n    assert_eq!(1 + 1, 2);\n}\n",
    );
    let gates = expect(consumer, &["coverage", "//gate:fails_test"], 3, &[])
        .and_then(|_| {
            expect(
                consumer,
                &["coverage", "//gate:uncollected_test"],
                1,
                &["no_coverage_collector"],
            )
        })
        .and_then(|_| {
            expect(
                consumer,
                &["coverage", "--fail-on", "error", "//gate:uncollected_test"],
                0,
                &[],
            )
        });
    consumer.remove("gate")?;
    gates.map(drop)
}

fn bench(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["bench", "//bench/..."], 0, &["parse"]).map(drop)
}

fn fuzz(consumer: &Consumer) -> Result<(), String> {
    expect(
        consumer,
        &["fuzz", "//fuzz:parse"],
        0,
        &["Fuzzing //fuzz:parse"],
    )?;
    expect(
        consumer,
        &["fuzz", "//fuzz:crashy"],
        1,
        &["//fuzz:crashy crashed; reproduce with"],
    )?;
    let crash = consumer
        .case
        .root()
        .join(".dx/local/fuzz/fuzz/crashy/crash-0001");
    if !crash.is_file() {
        return Err(format!("no crash file at {}", crash.display()));
    }
    expect(
        consumer,
        &["fuzz", "//app/..."],
        0,
        &["no dx-fuzz targets in scope"],
    )
    .map(drop)
}

fn deploy(consumer: &Consumer) -> Result<(), String> {
    expect(
        consumer,
        &["deploy", "//hello:hello_deploy"],
        0,
        &["Validated //hello:hello_deploy (built, not published"],
    )?;
    let out = consumer.scratch.join("released");
    std::fs::create_dir_all(&out).map_err(|error| format!("{}: {error}", out.display()))?;
    let dir = out.to_string_lossy().into_owned();
    expect(
        consumer,
        &["deploy", "--apply", "//hello:hello_deploy", "--", &dir],
        0,
        &[],
    )?;
    for name in ["hello_deploy.tar.gz", "hello_deploy.tar.gz.sha256"] {
        if !out.join(name).is_file() {
            return Err(format!("dx deploy --apply wrote no {name} in {dir}"));
        }
    }
    expect(consumer, &["deploy", "//hello/..."], 2, &[]).map(drop)
}

fn docs(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["docs", "//app:calc"], 0, &[]).map(drop)
}

fn format(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["format", "//app/..."], 0, &[]).map(drop)
}

fn lint(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["lint", "//app/..."], 0, &[]).map(drop)
}

fn typecheck(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["typecheck", "//app/..."], 0, &[]).map(drop)
}

fn generate(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["generate", "//app/..."], 0, &[]).map(drop)
}

fn check(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["check", "//app/..."], 0, &[]).map(drop)
}

fn watch(consumer: &Consumer) -> Result<(), String> {
    use std::io::BufRead;
    crate::stage(TEST, "dx watch --poll build //app:calc");
    let mut child = consumer
        .case
        .command(&consumer.dx, &consumer.envs())
        .args([
            "watch",
            consumer.workspace.as_str(),
            "--poll",
            "build",
            "//app:calc",
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("spawn dx watch: {error}"))?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    let mut seen = Vec::new();
    let mut result = Err(String::new());
    while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
        match lines.recv_timeout(left) {
            Ok(line) if line.starts_with("iteration 1: ok") => {
                result = Ok(());
                break;
            }
            Ok(line) => seen.push(line),
            Err(_) => break,
        }
    }
    let exited = child
        .try_wait()
        .map_err(|error| format!("dx watch: {error}"))?;
    drop(child.kill());
    drop(child.wait());
    match (result, exited) {
        (Ok(()), None) => Ok(()),
        (Ok(()), Some(status)) => Err(format!("dx watch exited {status} while watching")),
        (Err(_), _) => Err(format!(
            "dx watch printed no `iteration 1: ok`:\n{}",
            seen.join("\n")
        )),
    }
}

fn sync(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["sync"], 0, &[]).map(drop)
}

fn clean(consumer: &Consumer) -> Result<(), String> {
    expect(consumer, &["clean"], 0, &["dx clean: removed"]).map(drop)
}

fn write_packages(root: &Path) {
    let files = [
        (
            "app/BUILD.bazel",
            "load(\"@rules_dx//rust:defs.bzl\", \"rust_library\", \"rust_test\")\n\nrust_library(\n    name = \"calc\",\n    srcs = [\"calc.rs\"],\n    crate_root = \"calc.rs\",\n)\n\nrust_test(\n    name = \"calc_test\",\n    size = \"small\",\n    crate = \":calc\",\n)\n",
        ),
        (
            "app/calc.rs",
            "//! Arithmetic for the dx command cases.\n\n/// Returns the sum of `a` and `b`.\n#[must_use]\npub fn add(a: u32, b: u32) -> u32 {\n    a.saturating_add(b)\n}\n\n/// Returns the product of `a` and `b`.\n#[must_use]\npub fn mul(a: u32, b: u32) -> u32 {\n    a.saturating_mul(b)\n}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn adds() {\n        assert_eq!(super::add(2, 3), 5);\n    }\n}\n",
        ),
        (
            "hello/BUILD.bazel",
            "load(\"@rules_dx//deploy:defs.bzl\", \"archive_deploy\")\nload(\"@rules_dx//rust:defs.bzl\", \"rust_binary\")\n\nrust_binary(\n    name = \"hello\",\n    srcs = [\"hello.rs\"],\n    crate_root = \"hello.rs\",\n)\n\narchive_deploy(\n    name = \"hello_deploy\",\n    app = \":hello\",\n    build_mode = \"dev\",\n)\n",
        ),
        (
            "hello/hello.rs",
            "//! Prints a greeting.\n\nuse std::io::Write;\n\nfn main() -> std::io::Result<()> {\n    writeln!(std::io::stdout(), \"hello from dx run\")\n}\n",
        ),
        (
            "bench/BUILD.bazel",
            "load(\"@rules_dx//rust:defs.bzl\", \"rust_bench\")\n\nrust_bench(\n    name = \"parse_bench\",\n    srcs = [\"parse_bench.rs\"],\n    crate_root = \"parse_bench.rs\",\n)\n",
        ),
        (
            "bench/parse_bench.rs",
            "//! Reports one fixed result in libtest's bencher format.\n\nuse std::io::Write;\n\nfn main() -> std::io::Result<()> {\n    writeln!(std::io::stdout(), \"test parse ... bench: 1,200 ns/iter (+/- 100)\")\n}\n",
        ),
        (
            "fuzz/BUILD.bazel",
            "load(\"@rules_dx//rust:defs.bzl\", \"rust_binary\")\n\nfilegroup(\n    name = \"parse\",\n    srcs = [\"runner.rs\"],\n    tags = [\"dx-fuzz\"],\n)\n\nrust_binary(\n    name = \"parse_run\",\n    srcs = [\"runner.rs\"],\n    crate_root = \"runner.rs\",\n)\n\nfilegroup(\n    name = \"crashy\",\n    srcs = [\"runner.rs\"],\n    tags = [\"dx-fuzz\"],\n)\n\nrust_binary(\n    name = \"crashy_run\",\n    srcs = [\"runner.rs\"],\n    args = [\"--crash\"],\n    crate_root = \"runner.rs\",\n)\n",
        ),
        (
            "fuzz/runner.rs",
            "//! A dx fuzz runner: with `--crash` it writes `crash-0001` under `--out` and exits 77.\n\nuse std::path::PathBuf;\nuse std::process::ExitCode;\n\nfn main() -> ExitCode {\n    let args: Vec<String> = std::env::args().skip(1).collect();\n    let crash = args.iter().any(|arg| arg == \"--crash\");\n    let out = args.iter().skip_while(|arg| *arg != \"--out\").nth(1);\n    let (true, Some(out)) = (crash, out) else {\n        return ExitCode::SUCCESS;\n    };\n    let root = std::env::var_os(\"BUILD_WORKSPACE_DIRECTORY\").map_or_else(PathBuf::new, PathBuf::from);\n    match std::fs::write(root.join(out).join(\"crash-0001\"), b\"boom\") {\n        Ok(()) => ExitCode::from(77),\n        Err(_) => ExitCode::from(3),\n    }\n}\n",
        ),
        (
            "deps/BUILD.bazel",
            "exports_files([\n    \"Cargo.lock\",\n    \"Cargo.toml\",\n])\n",
        ),
        (
            "deps/Cargo.toml",
            "[package]\nname = \"deps\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"lib.rs\"\n",
        ),
        (
            "deps/Cargo.lock",
            "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"deps\"\nversion = \"0.1.0\"\n",
        ),
        ("deps/lib.rs", "//! A crate with no dependencies.\n"),
    ];
    for (rel, text) in files {
        write_text(&root.join(rel), text);
    }
}

fn add_cargo_set(root: &Path) {
    let module_path = root.join("MODULE.bazel");
    let mut module = read_text(&module_path);
    module.push_str("\ncargo = use_extension(\"@rules_dx//rust:extensions.bzl\", \"cargo\")\ncargo.from_cargo(\n    name = \"crates\",\n    cargo_lockfile = \"//deps:Cargo.lock\",\n    manifests = [\"//deps:Cargo.toml\"],\n)\nuse_repo(cargo, \"crates\")\n");
    write_text(&module_path, &module);
}

#[test]
fn every_dx_command_has_a_case() {
    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN must name dx"));
    let help = dx_testing::run(&dx, &["--help"], &[]).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(
        help.status.code(),
        Some(0),
        "dx --help: {}",
        help.combined()
    );
    let mut listed: Vec<&str> = help
        .stdout
        .lines()
        .skip_while(|line| *line != "Commands:")
        .skip(1)
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    listed.sort_unstable();
    let mut covered: Vec<&str> = CASES.iter().map(|(name, _)| *name).collect();
    covered.sort_unstable();
    assert_eq!(
        listed, covered,
        "each command in `dx --help` needs one case"
    );
}

#[test]
fn every_dx_command_runs_in_a_consumer() {
    let source = rules_dx_source().unwrap_or_else(|error| panic!("{error}"));
    let case = nested::case("release_commands");
    let dx = resolve_runfiles(&std::env::var("DX_BIN").expect("DX_BIN must name dx"));
    let mut init_args = vec![
        "init".to_owned(),
        "--apply".to_owned(),
        "--no-generate".to_owned(),
    ];
    if cfg!(windows) {
        init_args.extend([
            "--accept-license".to_owned(),
            "visual-studio-build-tools".to_owned(),
        ]);
    }
    write_packages(case.root());
    init_consumer(&case, &source, &dx, &init_args, TEST);
    add_cargo_set(case.root());
    case.use_lock("release_commands");
    let scratch = dx_testing::scratch_dir("dx_commands");
    let mut env = nested_env();
    env.push(("DX_AUDIT_TODAY", AUDIT_TODAY.to_owned()));
    let consumer = Consumer {
        workspace: format!("--workspace={}", case.root().display()),
        case,
        dx,
        env,
        scratch,
    };
    let failures: Vec<String> = CASES
        .iter()
        .filter_map(|(name, case)| {
            case(&consumer)
                .err()
                .map(|error| format!("{name}: {error}"))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
