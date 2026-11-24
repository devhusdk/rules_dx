use std::process::ExitCode;

use dx_nested as nested;

/// A repository rule whose output Bazel may take from the repo contents cache.
const PROBE_BZL: &str = "def _impl(rctx):\n    rctx.file(\"BUILD.bazel\", \"filegroup(name = \\\"all\\\")\\n\")\n    return rctx.repo_metadata(reproducible = True)\n\nprobe = repository_rule(implementation = _impl)\n";

const MODULE: &str = "module(name = \"probe\")\n\nprobe = use_repo_rule(\"//:probe.bzl\", \"probe\")\n\nprobe(name = \"dx_probe\")\n";

/// The line the Bazel client prints when it extracts its install base.
const EXTRACTING: &str = "Extracting Bazel installation";

fn server_pid(case: &nested::Case, version: &str) -> Result<(String, bool), String> {
    for (file, text) in [
        (".bazelversion", version),
        ("MODULE.bazel", MODULE),
        ("BUILD.bazel", ""),
        ("probe.bzl", PROBE_BZL),
    ] {
        std::fs::write(case.join(file), text).map_err(|error| format!("write {file}: {error}"))?;
    }
    let info = case.bazel(&["info", "server_pid"]);
    if !info.status.success() {
        return Err(info.combined());
    }
    if case.servers().len() != 1 {
        return Err("one server per test binary".to_owned());
    }
    let extracted = info.combined().contains(EXTRACTING);
    Ok((info.stdout.trim().to_owned(), extracted))
}

/// Depends on every module the lock selected, so the module graph only needs files the lock holds hashes for.
fn locked_deps(lock: &str) -> String {
    lock.lines()
        .filter_map(|line| {
            let url = line.trim().strip_prefix('"')?.split('"').next()?;
            let path = url.strip_suffix("/source.json")?;
            let (rest, version) = path.rsplit_once('/')?;
            let (_, name) = rest.rsplit_once("/modules/")?;
            Some(format!(
                "bazel_dep(name = \"{name}\", version = \"{version}\")\n"
            ))
        })
        .collect()
}

/// The repository rules a query of the probe repository ran.
fn fetched(case: &nested::Case) -> Result<Vec<String>, String> {
    case.use_lock("harness");
    let lock = std::fs::read_to_string(case.join("MODULE.bazel.lock"))
        .map_err(|error| format!("read MODULE.bazel.lock: {error}"))?;
    std::fs::write(
        case.join("MODULE.bazel"),
        format!("{MODULE}{}", locked_deps(&lock)),
    )
    .map_err(|error| format!("write MODULE.bazel: {error}"))?;
    let log = case.join("rules.log");
    let flag = format!("--experimental_workspace_rules_log_file={}", log.display());
    let query = case.bazel(&["query", &flag, "@dx_probe//:all"]);
    if !query.status.success() {
        return Err(query.combined());
    }
    match std::fs::read(&log) {
        Ok(bytes) => Ok(nested::record::contexts(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("read {}: {error}", log.display())),
    }
}

fn run() -> Result<String, String> {
    let version = std::env::var("DX_PROBE_BAZEL_VERSION")
        .map_err(|error| format!("DX_PROBE_BAZEL_VERSION must name the Bazel version: {error}"))?;
    let case = nested::case("probe");
    let (first, extracted) = server_pid(&case, &version)?;
    let fetched = fetched(&case)?;
    let (second, _) = server_pid(&nested::case("probe"), &version)?;
    if first != second {
        return Err(format!("cases must share one server: {first} != {second}"));
    }
    Ok(format!(
        "server_pid={second}\nextracted={extracted}\nfetched={}\n",
        fetched.join(" ")
    ))
}

fn main() -> ExitCode {
    match run() {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("nested_bazel_probe: {error}");
            ExitCode::FAILURE
        }
    }
}
