#![cfg(unix)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use dx_testing::{mkscratch, resolve_runfiles};

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

fn env_path(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"));
    resolve_runfiles(&rel)
}

fn repo_root() -> PathBuf {
    env_path("DX_MODULE")
        .canonicalize()
        .expect("MODULE.bazel resolves to the source tree")
        .parent()
        .expect("repo root")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("read {path:?}: {error}"))
}

fn write(path: &Path, text: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| panic!("mkdir: {error}"));
    }
    std::fs::write(path, text).unwrap_or_else(|error| panic!("write {path:?}: {error}"));
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=dx",
            "-c",
            "user.email=dx@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .status()
        .unwrap_or_else(|error| panic!("git: {error}"));
    assert!(status.success(), "git {args:?}");
}

struct Env {
    home: PathBuf,
    session: &'static str,
    project: PathBuf,
}

fn run_with(program: &str, args: &[&str], cwd: &Path, env: &Env, stdin: &str) -> Out {
    run_env(program, args, cwd, env, stdin, &[])
}

fn run_env(
    program: &str,
    args: &[&str],
    cwd: &Path,
    env: &Env,
    stdin: &str,
    extra: &[(&str, &str)],
) -> Out {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("HOME", &env.home)
        .env("XDG_CACHE_HOME", env.home.join(".cache"))
        .env("CLAUDE_CODE_SESSION_ID", env.session)
        .env("CLAUDE_PROJECT_DIR", &env.project)
        .envs(extra.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("spawn {program}: {error}"));
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("stdin write");
    let output = child.wait_with_output().expect("wait");
    Out {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn block(text: &str) -> String {
    let start = text.find("<!-- dx:begin -->").expect("begin marker");
    let end = text.find("<!-- dx:end -->").expect("end marker") + "<!-- dx:end -->".len();
    text[start..end].replace("\r\n", "\n")
}

fn init(root: &Path, env: &Env) {
    let dx = env_path("DX_BIN");
    let out = run_with(
        dx.to_str().expect("utf8"),
        &[
            "init",
            "--apply",
            "--with=agents",
            "--workspace",
            root.to_str().expect("utf8"),
        ],
        root,
        env,
        "",
    );
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
}

fn scratch_env(name: &str) -> (PathBuf, Env) {
    let base = mkscratch(name).unwrap_or_else(|error| panic!("scratch: {error}"));
    let home = base.join("home");
    std::fs::create_dir_all(&home).expect("home");
    let root = base.join("app");
    std::fs::create_dir_all(&root).expect("root");
    let env = Env {
        home,
        session: "session-a",
        project: root.clone(),
    };
    (root, env)
}

#[test]
fn rules_dx_commits_what_dx_init_writes() {
    let (root, env) = scratch_env("dx-agents-drift-");
    init(&root, &env);
    assert_eq!(
        read(&root.join(".claude/settings.json")),
        read(&repo_root().join(".claude/settings.json"))
    );
    assert_eq!(
        block(&read(&root.join("AGENTS.md"))),
        block(&read(&repo_root().join("AGENTS.md")))
    );
}

fn hook_commands(settings: &str) -> Vec<(String, String)> {
    let parsed: dx_testing::serde_json::Value =
        dx_testing::serde_json::from_str(settings).expect("settings json");
    let hooks = parsed["hooks"].as_object().expect("hooks");
    hooks
        .iter()
        .map(|(event, groups)| {
            let command = groups[0]["hooks"][0]["command"]
                .as_str()
                .expect("command")
                .to_owned();
            (event.clone(), command)
        })
        .collect()
}

fn hook(commands: &[(String, String)], event: &str) -> String {
    commands
        .iter()
        .find(|(name, _)| name == event)
        .map(|(_, command)| command.clone())
        .unwrap_or_else(|| panic!("no {event} hook"))
}

fn cache_dx(env: &Env) {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    };
    let dir = env.home.join(".cache/dx/bin/0.0.0");
    std::fs::create_dir_all(&dir).expect("cache dir");
    std::fs::copy(env_path("DX_BIN"), dir.join(format!("dx-{os}-{arch}"))).expect("cache dx");
}

#[test]
fn an_agent_session_in_a_fresh_consumer_uses_the_launcher_and_hooks() {
    let (root, env) = scratch_env("dx-agents-consumer-");
    let origin = root.parent().expect("base").join("origin.git");
    git(
        root.parent().expect("base"),
        &[
            "init",
            "--quiet",
            "--bare",
            "--initial-branch=main",
            origin.to_str().expect("utf8"),
        ],
    );
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    write(&root.join("MODULE.bazel"), "module(name = \"consumer\")\n");
    init(&root, &env);
    cache_dx(&env);
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    git(
        &root,
        &["remote", "add", "origin", origin.to_str().expect("utf8")],
    );
    git(&root, &["push", "--quiet", "-u", "origin", "main"]);

    let launcher = root.join("tools/dx");
    let launcher = launcher.to_str().expect("utf8");
    let version = run_with(launcher, &["version"], &root, &env, "");
    assert_eq!(version.code, 0, "{}{}", version.stdout, version.stderr);

    let commands = hook_commands(&read(&root.join(".claude/settings.json")));
    assert_eq!(commands.len(), 4);

    let guard = hook(&commands, "PreToolUse");
    let blocked = run_with(
        "sh",
        &["-c", &guard],
        &root,
        &env,
        "{\"tool_input\":{\"command\":\"git worktree add ../x\"}}",
    );
    assert_eq!(blocked.code, 2, "{}{}", blocked.stdout, blocked.stderr);
    assert!(
        blocked.stderr.contains("dx worktree add"),
        "{}",
        blocked.stderr
    );
    let allowed = run_with(
        "sh",
        &["-c", &guard],
        &root,
        &env,
        "{\"tool_input\":{\"command\":\"ls\"}}",
    );
    assert_eq!(allowed.code, 0, "{}{}", allowed.stdout, allowed.stderr);
    let named = run_with(
        "sh",
        &["-c", &guard],
        &root,
        &env,
        "{\"agent_id\":\"a1\",\"tool_input\":{\"command\":\"tools/dx worktree release\"}}",
    );
    assert_eq!(named.code, 0, "{}{}", named.stdout, named.stderr);
    assert!(
        named
            .stdout
            .contains("export DX_WORKTREE_HOLDER='a1'\\ntools/dx worktree release"),
        "{}",
        named.stdout
    );

    let test = run_with(
        "sh",
        &["-c", &guard],
        &root,
        &env,
        "{\"agent_id\":\"a1\",\"tool_input\":{\"command\":\"tools/dx test //x\"}}",
    );
    assert_eq!(test.code, 0, "{}{}", test.stdout, test.stderr);
    assert!(
        test.stdout
            .contains("export DX_WORKTREE_HOLDER='a1'\\ntools/dx test //x"),
        "{}",
        test.stdout
    );

    let added = run_with(
        launcher,
        &[
            "worktree",
            "add",
            "issue-1",
            "--create=origin/main",
            "--output=path",
        ],
        &root,
        &env,
        "",
    );
    assert_eq!(added.code, 0, "{}{}", added.stdout, added.stderr);
    let slot = PathBuf::from(added.stdout.trim());
    assert!(slot.join(".git").exists(), "{slot:?}");
    assert!(slot.join("tools/dx").exists());

    let other = Env {
        home: env.home.clone(),
        session: "session-b",
        project: root.clone(),
    };
    let taken = run_with(
        launcher,
        &["worktree", "add", "issue-1", "--output=path"],
        &root,
        &other,
        "",
    );
    assert_eq!(taken.code, 1, "{}{}", taken.stdout, taken.stderr);
    let busy = run_with(launcher, &["status"], &slot, &other, "");
    assert_eq!(busy.code, 2, "{}{}", busy.stdout, busy.stderr);

    let create = hook(&commands, "WorktreeCreate");
    let sub = run_with(
        "sh",
        &["-c", &create],
        &root,
        &env,
        &format!(
            "{{\"name\":\"sub one\",\"cwd\":{:?}}}",
            root.to_str().expect("utf8")
        ),
    );
    assert_eq!(sub.code, 0, "{}{}", sub.stdout, sub.stderr);
    let sub_path = PathBuf::from(sub.stdout.trim());
    assert!(sub_path.join(".git").exists(), "{sub_path:?}");
    assert_ne!(sub_path, slot);

    let remove = hook(&commands, "WorktreeRemove");
    let remove_input = format!(
        "{{\"worktree_path\":{:?}}}",
        sub_path.to_str().expect("utf8")
    );
    assert!(remove.contains("hook remove"), "{remove}");
    let before = run_with(launcher, &["worktree", "list"], &root, &env, "");
    assert_eq!(before.code, 0, "{}{}", before.stdout, before.stderr);
    let bogus = run_with(
        "sh",
        &["-c", &remove.replace("hook remove", "hook bogus")],
        &root,
        &env,
        &remove_input,
    );
    assert_eq!(bogus.code, 2, "{}{}", bogus.stdout, bogus.stderr);
    assert!(
        bogus
            .stderr
            .contains("want one of: create, remove, stop, guard"),
        "{}",
        bogus.stderr
    );
    let after = run_with(launcher, &["worktree", "list"], &root, &env, "");
    let slots = |text: &str| {
        text.lines()
            .filter(|line| line.contains('\t'))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(
        slots(&before.stdout),
        slots(&after.stdout),
        "an unknown hook changed a lease"
    );
    assert!(
        slots(&after.stdout).contains("worktree-sub-one"),
        "{}",
        after.stdout
    );

    let removed = run_with("sh", &["-c", &remove], &root, &env, &remove_input);
    assert_eq!(removed.code, 0, "{}{}", removed.stdout, removed.stderr);

    let released = run_with(launcher, &["worktree", "release"], &slot, &env, "");
    assert_eq!(released.code, 0, "{}{}", released.stdout, released.stderr);

    let again = run_with(
        launcher,
        &[
            "worktree",
            "add",
            "issue-2",
            "--create=origin/main",
            "--output=path",
        ],
        &root,
        &other,
        "",
    );
    assert_eq!(again.code, 0, "{}{}", again.stdout, again.stderr);
    let warm = PathBuf::from(again.stdout.trim());
    assert!(warm == slot || warm == sub_path, "{warm:?} is a new slot");

    let status = run_with(launcher, &["status"], &root, &env, "");
    assert!(
        status.stdout.contains("agents: ok"),
        "{}{}",
        status.stdout,
        status.stderr
    );
}

fn hook_repo(name: &str) -> (PathBuf, Env) {
    let (root, env) = scratch_env(name);
    let origin = root.parent().expect("base").join("origin.git");
    git(
        root.parent().expect("base"),
        &[
            "init",
            "--quiet",
            "--bare",
            "--initial-branch=main",
            origin.to_str().expect("utf8"),
        ],
    );
    git(&root, &["init", "--quiet", "--initial-branch=main"]);
    write(&root.join("MODULE.bazel"), "module(name = \"consumer\")\n");
    git(&root, &["add", "."]);
    git(&root, &["commit", "--quiet", "-m", "base"]);
    git(
        &root,
        &["remote", "add", "origin", origin.to_str().expect("utf8")],
    );
    git(&root, &["push", "--quiet", "-u", "origin", "main"]);
    (root, env)
}

#[test]
fn hooks_ignore_bad_defaults_and_bad_input() {
    let (root, env) = hook_repo("dx-agents-hooks-");
    write(&root.join("dx.toml"), "[dx]\nfail_on = \"error\"\n");

    let dx = env_path("DX_BIN");
    let dx = dx.to_str().expect("utf8");
    let bad = [("DX_QUIET", "1"), ("DX_DRY_RUN", "1")];
    let run = |args: &[&str], stdin: &str| run_env(dx, args, &root, &env, stdin, &bad);

    let allowed = run(
        &["worktree", "hook", "guard"],
        "{\"tool_input\":{\"command\":\"ls\"}}",
    );
    assert_eq!(allowed.code, 0, "{}{}", allowed.stdout, allowed.stderr);
    assert_eq!(allowed.stdout, "");
    let empty = run(&["worktree", "hook", "guard"], "");
    assert_eq!(empty.code, 0, "{}{}", empty.stdout, empty.stderr);
    assert!(
        empty.stderr.contains("command not checked"),
        "{}",
        empty.stderr
    );
    let blocked = run(
        &["worktree", "hook", "guard"],
        "{\"tool_input\":{\"command\":\"git worktree add ../x\"}}",
    );
    assert_eq!(blocked.code, 2, "{}{}", blocked.stdout, blocked.stderr);

    let bad_create = run(&["worktree", "hook", "create"], "");
    assert_eq!(
        bad_create.code, 1,
        "{}{}",
        bad_create.stdout, bad_create.stderr
    );

    let created = run(
        &["worktree", "hook", "create"],
        &format!(
            "{{\"name\":\"one\",\"cwd\":{:?}}}",
            root.to_str().expect("utf8")
        ),
    );
    assert_eq!(created.code, 0, "{}{}", created.stdout, created.stderr);
    assert_eq!(created.stdout.lines().count(), 1, "{}", created.stdout);
    let path = PathBuf::from(created.stdout.trim());
    assert!(path.join(".git").exists(), "{path:?}");

    let stopped = run(&["worktree", "hook", "stop"], "{\"agent_id\":\"one\"}");
    assert_eq!(stopped.code, 0, "{}{}", stopped.stdout, stopped.stderr);
    let removed = run(
        &["worktree", "hook", "remove"],
        &format!("{{\"worktree_path\":{:?}}}", path.to_str().expect("utf8")),
    );
    assert_eq!(removed.code, 0, "{}{}", removed.stdout, removed.stderr);

    let version = run(&["version"], "");
    assert_eq!(version.code, 2, "{}{}", version.stdout, version.stderr);
}

#[test]
fn hooks_fall_back_to_the_bazel_run_working_directory() {
    let (root, env) = hook_repo("dx-agents-hooks-bazel-run-");
    let base = root.parent().expect("base");
    let runfiles = base.join("runfiles");
    std::fs::create_dir_all(&runfiles).expect("runfiles");
    let dx = env_path("DX_BIN");
    let dx = dx.to_str().expect("utf8");
    let user_dir = [
        ("BUILD_WORKING_DIRECTORY", root.to_str().expect("utf8")),
        ("GIT_CEILING_DIRECTORIES", base.to_str().expect("utf8")),
    ];
    let run = |args: &[&str], stdin: &str| run_env(dx, args, &runfiles, &env, stdin, &user_dir);

    let created = run(&["worktree", "hook", "create"], "{\"name\":\"two\"}");
    assert_eq!(created.code, 0, "{}{}", created.stdout, created.stderr);
    assert_eq!(created.stdout.lines().count(), 1, "{}", created.stdout);
    let path = PathBuf::from(created.stdout.trim());
    assert!(path.join(".git").exists(), "{path:?}");

    let removed = run(
        &["worktree", "hook", "remove"],
        &format!("{{\"worktree_path\":{:?}}}", path.to_str().expect("utf8")),
    );
    assert_eq!(removed.code, 0, "{}{}", removed.stdout, removed.stderr);
}

fn executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn script(path: &Path, body: &str) {
    write(path, &format!("#!/bin/sh\n{body}\n"));
    executable(path);
}

fn launcher_run(root: &Path, path: &Path, args: &[&str]) -> Out {
    let output = Command::new(root.join("tools/dx"))
        .args(args)
        .current_dir(root)
        .env("PATH", format!("{}:/usr/bin:/bin", path.display()))
        .output()
        .unwrap_or_else(|error| panic!("launcher: {error}"));
    Out {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn launcher_root(name: &str) -> (PathBuf, PathBuf) {
    let base = mkscratch(name).unwrap_or_else(|error| panic!("scratch: {error}"));
    let root = base.join("rules dx");
    std::fs::create_dir_all(&root).expect("root");
    let root = root.canonicalize().expect("root path");
    write(&root.join("MODULE.bazel"), &read(&env_path("DX_MODULE")));
    write(&root.join("cli/cli/BUILD.bazel"), "");
    write(&root.join("tools/dx"), &read(&repo_root().join("tools/dx")));
    executable(&root.join("tools/dx"));
    let fakes = base.join("fakes");
    std::fs::create_dir_all(&fakes).expect("fakes");
    (root, fakes)
}

fn fake_bazel(root: &Path, fakes: &Path, dx: &str, rewrite: bool) {
    let guard = if rewrite { "" } else { "[ -x \"$out\" ] || " };
    script(
        &fakes.join("bazel"),
        &format!(
            "echo \"$*\" >> '{log}'\ncase \"$3\" in\n  build)\n    out=bazel-bin/cli/cli/{dx}\n    mkdir -p bazel-bin/cli/cli\n    {guard}{{ printf '#!/bin/sh\\nprintf \"%%s\\\\n\" \"$MSYS2_ARG_CONV_EXCL\" \"$@\"\\n' > \"$out\"; chmod +x \"$out\"; }}\n    ;;\n  query) printf '%s\\n' //cli/cli:BUILD.bazel //cli/cli:src/main.rs //tools/bazelrc:src/lib.rs @rules_rust//rust:defs.bzl ;;\nesac",
            log = root.join("bazel.log").display()
        ),
    );
}

fn bazel_calls(root: &Path, verb: &str) -> usize {
    std::fs::read_to_string(root.join("bazel.log")).map_or(0, |log| {
        log.lines()
            .filter(|line| line.split_whitespace().nth(2) == Some(verb))
            .count()
    })
}

fn touch_later(path: &Path) {
    if !path.exists() {
        write(path, "\n");
    }
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open");
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(3600))
        .expect("touch");
}

fn backdate(path: &Path, ago: std::time::Duration) {
    let file = std::fs::File::options()
        .write(true)
        .open(path)
        .expect("open");
    let when = std::time::SystemTime::now()
        .checked_sub(ago)
        .expect("clock past epoch");
    file.set_modified(when).expect("backdate");
}

fn backdate_tree(dir: &Path, ago: std::time::Duration) {
    for entry in std::fs::read_dir(dir).expect("read_dir") {
        let path = entry.expect("entry").path();
        let kind = std::fs::symlink_metadata(&path).expect("metadata").file_type();
        if kind.is_dir() {
            backdate_tree(&path, ago);
        } else if kind.is_file() {
            backdate(&path, ago);
        }
    }
}

fn built_launcher_root(name: &str, rewrite: bool) -> (PathBuf, PathBuf) {
    let (root, fakes) = launcher_root(name);
    fake_bazel(&root, &fakes, "dx", rewrite);
    let first = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
    assert_eq!(bazel_calls(&root, "build"), 1);
    assert_eq!(bazel_calls(&root, "query"), 1);
    (root, fakes)
}

#[test]
fn the_launcher_in_a_rules_dx_checkout_builds_once_and_reruns_the_built_dx() {
    let (root, fakes) = launcher_root("dx-agents-launcher-");
    fake_bazel(&root, &fakes, "dx", true);
    let first = launcher_run(&root, &fakes, &["worktree", "hook", "guard"]);
    assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
    assert_eq!(first.stdout, "\nworktree\nhook\nguard\n");
    assert!(
        first.stderr.contains("dx: building //:dx"),
        "{}",
        first.stderr
    );
    assert!(root.join(".dx/launcher/dx").exists());
    assert_eq!(bazel_calls(&root, "build"), 1);
    let log = read(&root.join("bazel.log"));
    assert!(log.starts_with("--nohome_rc --nosystem_rc build "), "{log}");
    assert!(log.contains("\n--nohome_rc --nosystem_rc query "), "{log}");
    assert!(log.contains("//:dx"));
    assert_eq!(bazel_calls(&root, "query"), 1);
    assert_eq!(
        read(&root.join(".dx/launcher/inputs")),
        "//cli/cli:BUILD.bazel\n//cli/cli:src/main.rs\n//tools/bazelrc:src/lib.rs\n"
    );

    let second = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(second.stdout, "\nversion\n", "{}", second.stderr);
    assert_eq!(
        read(&root.join("bazel.log")).lines().count(),
        2,
        "an unchanged tree runs dx without Bazel"
    );

    touch_later(&root.join("cli/cli/src/main.rs"));
    let third = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(third.stdout, "\nversion\n", "{}", third.stderr);
    assert_eq!(bazel_calls(&root, "build"), 2, "a newer source rebuilds dx");
}

#[test]
fn the_launcher_runs_hooks_with_the_built_dx_without_bazel() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-hook-", true);
    touch_later(&root.join("cli/cli/src/main.rs"));
    let hook = launcher_run(&root, &fakes, &["worktree", "hook", "guard"]);
    assert_eq!(hook.code, 0, "{}{}", hook.stdout, hook.stderr);
    assert_eq!(hook.stdout, "\nworktree\nhook\nguard\n");
    assert_eq!(bazel_calls(&root, "build"), 1);
    assert_eq!(bazel_calls(&root, "query"), 1);
    let next = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(next.stdout, "\nversion\n", "{}", next.stderr);
    assert_eq!(
        bazel_calls(&root, "build"),
        2,
        "the next other command rebuilds dx"
    );
}

#[test]
fn the_launcher_rebuilds_after_a_preset_layer_edit() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-preset-", true);
    touch_later(&root.join("tools/bazelrc/resources.bazelrc"));
    let out = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(bazel_calls(&root, "build"), 2);
}

#[test]
fn the_launcher_rebuilds_after_a_non_cli_input_edit() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-input-", true);
    touch_later(&root.join("tools/bazelrc/src/lib.rs"));
    let out = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(bazel_calls(&root, "build"), 2);
}

#[test]
fn the_launcher_detects_a_newer_input_with_a_posix_only_find() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-posix-find-", true);
    script(
        &fakes.join("find"),
        r#"for arg in "$@"; do
  case "$arg" in
    -quit|-maxdepth|-mindepth|-print0|-printf|-newer?*) echo "find: unsupported $arg" >&2; exit 1 ;;
  esac
done
if [ -x /usr/bin/find ]; then exec /usr/bin/find "$@"; fi
exec /bin/find "$@""#,
    );
    let quiet = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(quiet.code, 0, "{}{}", quiet.stdout, quiet.stderr);
    assert_eq!(bazel_calls(&root, "build"), 1);
    touch_later(&root.join("cli/cli/src/main.rs"));
    let out = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(bazel_calls(&root, "build"), 2);
}

#[test]
fn the_launcher_ignores_files_dx_does_not_build_from() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-ignore-", true);
    touch_later(&root.join("cli/cli/tests/new_test.rs"));
    let out = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(read(&root.join("bazel.log")).lines().count(), 2);
}

#[test]
fn the_launcher_does_not_rebuild_when_bazel_leaves_dx_untouched() {
    let hour = std::time::Duration::from_secs(3600);
    let (root, fakes) = built_launcher_root("dx-agents-launcher-untouched-", false);
    backdate_tree(&root, hour * 2);
    let log = || read(&root.join("bazel.log")).lines().count();
    let before = log();
    let idle = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(idle.code, 0, "{}{}", idle.stdout, idle.stderr);
    assert_eq!(bazel_calls(&root, "build"), 1);
    assert_eq!(log(), before, "a backdated, unedited tree is not rebuilt");
    let lib = root.join("tools/bazelrc/src/lib.rs");
    write(&lib, "pub fn edited() {}\n");
    backdate(&lib, hour);
    let rebuilt = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(rebuilt.code, 0, "{}{}", rebuilt.stdout, rebuilt.stderr);
    assert_eq!(bazel_calls(&root, "build"), 2);
    let calls = log();
    let again = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(again.code, 0, "{}{}", again.stdout, again.stderr);
    assert_eq!(log(), calls, "an unchanged dx is not rebuilt again");
}

#[test]
fn the_launcher_reports_a_failed_build_and_a_missing_bazel() {
    let (root, fakes) = launcher_root("dx-agents-launcher-fail-");
    let missing = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(missing.code, 1);
    assert!(
        missing.stderr.contains("bazel is not on PATH"),
        "{}",
        missing.stderr
    );
    assert!(
        !missing.stderr.contains("bazel run @rules_dx"),
        "{}",
        missing.stderr
    );
    script(&fakes.join("bazel"), "exit 3");
    let failed = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(failed.code, 1);
    assert!(
        failed.stderr.contains("bazel build //:dx failed"),
        "{}",
        failed.stderr
    );
    assert!(
        !failed.stderr.contains("bazel run @rules_dx"),
        "{}",
        failed.stderr
    );
}

#[test]
fn the_launcher_runs_the_previous_dx_when_dx_does_not_build() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-fallback-", true);
    script(
        &fakes.join("bazel"),
        &format!(
            "echo \"$*\" >> '{log}'\ncase \"$3\" in\n  build) rm -f bazel-bin/cli/cli/dx; exit 1 ;;\nesac",
            log = root.join("bazel.log").display()
        ),
    );
    touch_later(&root.join("cli/cli/src/main.rs"));
    let out = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(out.stdout, "\nversion\n");
    assert!(out.stderr.contains("dx: building //:dx"), "{}", out.stderr);
    assert!(
        out.stderr
            .contains("dx: //:dx does not build; running the previous dx"),
        "{}",
        out.stderr
    );
    assert!(
        !out.stderr.contains("bazel run @rules_dx"),
        "{}",
        out.stderr
    );
    assert!(!root.join("bazel-bin/cli/cli/dx").exists());
    assert_eq!(bazel_calls(&root, "build"), 2);
    let mut state: Vec<String> = std::fs::read_dir(root.join(".dx/launcher"))
        .expect("state dir")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    state.sort();
    assert_eq!(state, ["built", "dx", "inputs"]);

    let again = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(again.code, 0, "{}{}", again.stdout, again.stderr);
    assert_eq!(again.stdout, "\nversion\n");
    assert_eq!(
        bazel_calls(&root, "build"),
        3,
        "a failed build is tried again"
    );
}

#[test]
fn the_launcher_seeds_the_previous_dx_without_rebuilding() {
    let (root, fakes) = built_launcher_root("dx-agents-launcher-seed-", true);
    std::fs::remove_file(root.join(".dx/launcher/dx")).expect("remove the copy");
    let seeded = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(seeded.code, 0, "{}{}", seeded.stdout, seeded.stderr);
    assert_eq!(seeded.stdout, "\nversion\n");
    assert_eq!(bazel_calls(&root, "build"), 1);
    assert!(root.join(".dx/launcher/dx").exists());

    script(
        &fakes.join("bazel"),
        &format!(
            "echo \"$*\" >> '{log}'\ncase \"$3\" in\n  build) rm -f bazel-bin/cli/cli/dx; exit 1 ;;\nesac",
            log = root.join("bazel.log").display()
        ),
    );
    touch_later(&root.join("cli/cli/src/main.rs"));
    let out = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(out.stdout, "\nversion\n");
    assert!(
        out.stderr
            .contains("dx: //:dx does not build; running the previous dx"),
        "{}",
        out.stderr
    );
    assert_eq!(bazel_calls(&root, "build"), 2);
}

#[test]
fn the_launcher_runs_dx_exe_natively_under_msys() {
    let (root, fakes) = launcher_root("dx-agents-launcher-windows-");
    let log = root.join("calls.log");
    script(&fakes.join("uname"), "echo MINGW64_NT-10.0");
    script(
        &fakes.join("cygpath"),
        &format!("echo cygpath >> '{}'\necho \"$2\"", log.display()),
    );
    script(
        &fakes.join("cmd.exe"),
        &format!("echo cmd.exe >> '{}'", log.display()),
    );
    fake_bazel(&root, &fakes, "dx.exe", true);
    let out = launcher_run(&root, &fakes, &["a&b", "c|d", "^e", "%PATH%"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(out.stdout, "*\na&b\nc|d\n^e\n%PATH%\n");
    assert!(root.join("bazel-bin/cli/cli/dx.exe").exists());
    assert!(!log.exists(), "{}", read(&log));
}
