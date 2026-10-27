#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use dx_nested::{self as nested, Case};
use dx_testing::{mkscratch, resolve_runfiles, serde_json, Run};

/// Writes `$XML_OUTPUT_FILE` in one mode; the mode only reaches the action under `--config=consumer`.
pub const DEFS: &str = r##"_XML = """<?xml version="1.0" encoding="UTF-8"?>
<testsuites><testsuite name="cases.{name}" tests="{tests}" failures="{failures}" errors="0" skipped="{skipped}">{cases}</testsuite></testsuites>
"""

_PASS = '<testcase classname="cases.{name}" name="works" time="0.01"/><testcase classname="cases.{name}" name="later" time="0"><skipped message="not yet"/></testcase>'

_FAIL = '<testcase classname="cases.{name}" name="breaks" time="0.01"><failure message="expected 1, got 2">assertion failed</failure></testcase>'

def _xml(name, cases, tests, failures = 0, skipped = 0):
    return _XML.format(name = name, cases = cases.format(name = name), tests = tests, failures = failures, skipped = skipped)

def _write(name, cases, tests, failures = 0, skipped = 0):
    return "cat > \"$XML_OUTPUT_FILE\" <<'XML'\n" + _xml(name, cases, tests, failures, skipped) + "XML\n"

def _script(name, mode, state):
    if mode == "pass":
        return _write(name, _PASS, 2, skipped = 1) + "exit 0\n"
    if mode == "fail":
        return _write(name, _FAIL, 1, failures = 1) + "echo 'expected 1, got 2' >&2\nexit 1\n"
    if mode == "none":
        return _write(name, "", 0) + "exit 0\n"
    if mode == "truncated":
        return "printf '<testsuites><testsuite' > \"$XML_OUTPUT_FILE\"\nexit 0\n"
    marker = "'{}/{}".format(state, name)
    if mode == "retry":
        return "".join([
            "if [ ! -e {}.failed' ]; then\n".format(marker),
            ": > {}.failed'\n".format(marker),
            _write(name, _FAIL, 1, failures = 1),
            "exit 1\n",
            "fi\n",
            _write(name, _PASS, 2, skipped = 1),
            "exit 0\n",
        ])
    if mode == "sleep":
        return "echo $$ > {m}.pid.tmp'\nmv {m}.pid.tmp' {m}.pid'\nexec sleep 600\n".format(m = marker)
    return "echo 'the consumer config did not reach this test' >&2\nexit 1\n"

def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".sh")
    header = "#!/bin/sh\n# nonce {}\n".format(ctx.attr.nonce)
    ctx.actions.write(out, header + _script(ctx.label.name, ctx.attr.mode, ctx.attr.state), is_executable = True)
    return [DefaultInfo(executable = out)]

_case_test = rule(
    implementation = _impl,
    test = True,
    attrs = {
        "mode": attr.string(mandatory = True),
        "nonce": attr.string(),
        "state": attr.string(),
    },
)

def case_test(name, mode, nonce, state = "", **kwargs):
    _case_test(
        name = name,
        mode = select({
            "//cases:consumer": mode,
            "//conditions:default": "unconfigured",
        }),
        nonce = nonce,
        state = state,
        **kwargs
    )
"##;

/// Runs the real `bazel` from `DX_REAL_PATH`, then drops or corrupts one named artifact.
///
/// It touches nothing unless the command wrote a JSON event file and a `DX_TAMPER_*` variable is
/// set, and it always exits with Bazel's code. Each tampered path is appended to `DX_TAMPER_LOG`.
pub const TAMPER_BAZEL: &str = r##"#!/bin/sh
bep=
for arg in "$@"; do
  case "$arg" in
    --build_event_json_file=*) bep=${arg#--build_event_json_file=} ;;
  esac
done
PATH="$DX_REAL_PATH" bazel "$@"
code=$?
[ -n "$bep" ] && [ -f "$bep" ] || exit "$code"
xml_of() {
  grep -F "{\"id\":{\"testResult\":{\"label\":\"$1\"," "$bep" |
    grep -o '"name":"test.xml","uri":"file://[^"]*"' |
    sed -e 's|^"name":"test.xml","uri":"file://||' -e 's|"$||'
}
files_like() {
  grep -F '"namedSetOfFiles":{' "$bep" |
    grep -o '"uri":"file://[^"]*"' |
    sed -e 's|^"uri":"file://||' -e 's|"$||' |
    grep -E -- "$1"
}
log() {
  echo "$1" >> "${DX_TAMPER_LOG:-/dev/null}"
}
if [ -n "${DX_TAMPER_REMOVE:-}" ]; then
  for path in $(xml_of "$DX_TAMPER_REMOVE"); do
    rm -f "$path" && log "removed $path"
  done
fi
if [ -n "${DX_TAMPER_CORRUPT:-}" ]; then
  for path in $(xml_of "$DX_TAMPER_CORRUPT"); do
    rm -f "$path" && printf '<testsuites><testsuite' > "$path" && log "corrupted $path"
  done
fi
if [ -n "${DX_TAMPER_FILE:-}" ]; then
  for path in $(files_like "$DX_TAMPER_FILE"); do
    rm -f "$path" && log "removed $path"
  done
fi
exit "$code"
"##;

pub fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

pub fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

pub fn rules_dx_root() -> PathBuf {
    let marker = resolve_runfiles("cli/cli/tests/results_consumer/tests/common/mod.rs");
    let real = std::fs::canonicalize(&marker).unwrap_or_else(|error| panic!("{marker:?}: {error}"));
    real.ancestors()
        .nth(7)
        .unwrap_or_else(|| panic!("no workspace above {real:?}"))
        .to_path_buf()
}

/// The directory the `retry` and `sleep` cases keep their markers in.
pub fn state(case: &Case) -> PathBuf {
    case.root().join("state")
}

/// A consumer that depends on rules_dx as `my_dx` and declares one `case_test` per `(name, mode)`.
pub fn consumer(name: &str, cases: &[(&str, &str)]) -> Case {
    let rules = rules_dx_root();
    let case = nested::case(name);
    let root = case.root().to_path_buf();
    write(
        &root.join(".bazelversion"),
        &read(&rules.join(".bazelversion")),
    );
    write(
        &root.join("MODULE.bazel"),
        &format!(
            "module(name = \"consumer\")\n\nbazel_dep(name = \"rules_dx\", version = \"0.0.0\", repo_name = \"my_dx\")\nlocal_path_override(module_name = \"rules_dx\", path = \"{}\")\n",
            rules.display()
        ),
    );
    write(
        &root.join(".bazelrc"),
        &format!(
            "{}common:consumer --define=lane=consumer\n",
            dx_testing::preset_rc(&rules)
        ),
    );
    write(&root.join("BUILD.bazel"), "");
    write(&root.join("cases/defs.bzl"), DEFS);
    let state = state(&case);
    std::fs::create_dir_all(&state).unwrap_or_else(|error| panic!("mkdir {state:?}: {error}"));
    let nonce = nonce();
    let mut build = String::from(
        "load(\":defs.bzl\", \"case_test\")\n\nconfig_setting(\n    name = \"consumer\",\n    define_values = {\"lane\": \"consumer\"},\n)\n",
    );
    for (target, mode) in cases {
        let local = matches!(*mode, "retry" | "sleep");
        build.push_str(&format!(
            "\ncase_test(\n    name = \"{target}\",\n    mode = \"{mode}\",\n    nonce = \"{nonce}\",\n    state = \"{}\",\n    tags = [{}],\n)\n",
            state.display(),
            if local { "\"local\"" } else { "" }
        ));
    }
    write(&root.join("cases/BUILD.bazel"), &build);
    case
}

/// Changes every test action of a new consumer, so no earlier run's cached result answers for it.
fn nonce() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{}-{nanos}", std::process::id())
}

pub fn dx_bin() -> PathBuf {
    let rel = std::env::var("DX_BIN").unwrap_or_else(|_| panic!("DX_BIN must name dx"));
    resolve_runfiles(&rel)
}

/// Where the tamper wrapper logs what it changed.
pub struct Tamper {
    pub dir: PathBuf,
    pub log: PathBuf,
}

impl Default for Tamper {
    fn default() -> Tamper {
        Tamper::new()
    }
}

impl Tamper {
    pub fn new() -> Tamper {
        let dir = mkscratch("results-tamper").unwrap_or_else(|error| panic!("scratch: {error}"));
        let bin = dir.join("bin");
        write(&bin.join("bazel"), TAMPER_BAZEL);
        make_executable(&bin.join("bazel"));
        Tamper {
            log: dir.join("tamper.log"),
            dir: bin,
        }
    }

    /// The lines the wrapper logged, one per tampered file.
    pub fn changed(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("chmod {path:?}: {error}"));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}

/// Builds `dx <command> --workspace <case> <rest>`; with `tamper` set, Bazel runs through the wrapper.
pub fn dx_command(
    case: &Case,
    args: &[&str],
    tamper: &[(&str, &str)],
    wrapper: &Tamper,
) -> Command {
    let mut command = case.command(&dx_bin(), &[]);
    if !tamper.is_empty() {
        let harness = command
            .get_envs()
            .find(|(key, _)| *key == "PATH")
            .and_then(|(_, value)| value.map(ToOwned::to_owned))
            .unwrap_or_else(|| panic!("the harness sets PATH"));
        let mut dirs = vec![wrapper.dir.clone()];
        dirs.extend(std::env::split_paths(&harness));
        let joined = std::env::join_paths(dirs).unwrap_or_else(|error| panic!("PATH: {error}"));
        command.env("PATH", joined);
        command.env("DX_REAL_PATH", &harness);
        command.env("DX_TAMPER_LOG", &wrapper.log);
        for (key, value) in tamper {
            command.env(key, value);
        }
    }
    let workspace = case.root().to_string_lossy().into_owned();
    let (first, rest) = args
        .split_first()
        .unwrap_or_else(|| panic!("no dx command"));
    command
        .arg(first)
        .arg("--workspace")
        .arg(workspace)
        .args(rest);
    command
}

pub fn dx(case: &Case, args: &[&str], tamper: &[(&str, &str)], wrapper: &Tamper) -> Run {
    let output = dx_command(case, args, tamper, wrapper)
        .output()
        .unwrap_or_else(|error| panic!("run dx: {error}"));
    Run {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

pub fn events(run: &Run) -> Vec<serde_json::Value> {
    run.stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line).unwrap_or_else(|error| panic!("json {line:?}: {error}"))
        })
        .collect()
}

pub fn of_kind(events: &[serde_json::Value], kind: &str) -> Vec<serde_json::Value> {
    events
        .iter()
        .filter(|event| event["event"] == kind)
        .cloned()
        .collect()
}

pub fn finished(events: &[serde_json::Value]) -> serde_json::Value {
    let last = events
        .last()
        .cloned()
        .unwrap_or_else(|| panic!("no events"));
    assert_eq!(last["event"], "command_finished", "{last}");
    last
}

pub fn error_codes(events: &[serde_json::Value]) -> Vec<String> {
    of_kind(events, "error")
        .iter()
        .map(|event| event["code"].as_str().unwrap_or("?").to_owned())
        .collect()
}

pub fn results_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&read(path)).unwrap_or_else(|error| panic!("json {path:?}: {error}"))
}
