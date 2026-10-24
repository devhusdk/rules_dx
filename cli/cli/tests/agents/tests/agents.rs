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
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("HOME", &env.home)
        .env("XDG_CACHE_HOME", env.home.join(".cache"))
        .env("CLAUDE_CODE_SESSION_ID", env.session)
        .env("CLAUDE_PROJECT_DIR", &env.project)
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

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
        .lines()
        .filter(|line| !line.starts_with("VERSION=") && !line.starts_with("set \"VERSION="))
        .collect::<Vec<_>>()
        .join("\n")
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
        normalize(&read(&root.join("tools/dx"))),
        normalize(&read(&repo_root().join("tools/dx")))
    );
    assert_eq!(
        normalize(&read(&root.join("tools/dx.cmd"))),
        normalize(&read(&repo_root().join("tools/dx.cmd")))
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
    assert_eq!(commands.len(), 3);

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
        bogus.stderr.contains("want one of: create, remove, guard"),
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
    write(&root.join(".dx/version"), "0.0.0\n");
    write(&root.join("tools/dx"), &read(&repo_root().join("tools/dx")));
    executable(&root.join("tools/dx"));
    let fakes = base.join("fakes");
    std::fs::create_dir_all(&fakes).expect("fakes");
    (root, fakes)
}

#[test]
fn the_launcher_in_a_rules_dx_checkout_builds_once_and_reruns_the_built_dx() {
    let (root, fakes) = launcher_root("dx-agents-launcher-");
    script(
        &fakes.join("bazel"),
        &format!(
            "echo \"$*\" >> '{log}'\nmkdir -p bazel-bin/cli/cli\nprintf '#!/bin/sh\\necho built \"$@\"\\n' > bazel-bin/cli/cli/dx\nchmod +x bazel-bin/cli/cli/dx",
            log = root.join("bazel.log").display()
        ),
    );
    let first = launcher_run(&root, &fakes, &["worktree", "hook", "guard"]);
    assert_eq!(first.code, 0, "{}{}", first.stdout, first.stderr);
    assert_eq!(first.stdout.trim(), "built worktree hook guard");
    let builds = |root: &Path| read(&root.join("bazel.log")).lines().count();
    assert_eq!(builds(&root), 1);
    assert!(read(&root.join("bazel.log")).contains("build"));
    assert!(read(&root.join("bazel.log")).contains("//:dx"));

    let second = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(second.stdout.trim(), "built version", "{}", second.stderr);
    assert_eq!(builds(&root), 1, "an up-to-date dx is not rebuilt");

    write(&root.join("cli/cli/src/main.rs"), "fn main() {}\n");
    let touched = Command::new("touch")
        .args(["-d", "+1 hour"])
        .arg(root.join("cli/cli/src/main.rs"))
        .status()
        .expect("touch");
    assert!(touched.success());
    let third = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(third.stdout.trim(), "built version", "{}", third.stderr);
    assert_eq!(builds(&root), 2, "a newer source rebuilds dx");
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
    script(&fakes.join("bazel"), "exit 3");
    let failed = launcher_run(&root, &fakes, &["version"]);
    assert_eq!(failed.code, 1);
    assert!(
        failed.stderr.contains("bazel build //:dx failed"),
        "{}",
        failed.stderr
    );
}

#[test]
fn the_launcher_hands_a_windows_shell_to_the_cmd_launcher() {
    let (root, fakes) = launcher_root("dx-agents-launcher-windows-");
    script(&fakes.join("uname"), "echo MINGW64_NT-10.0");
    script(&fakes.join("cygpath"), "echo \"$2\"");
    script(&fakes.join("cmd.exe"), "echo \"cmd $*\"");
    let out = launcher_run(&root, &fakes, &["worktree", "hook", "guard"]);
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(
        out.stdout.trim(),
        format!("cmd /c {}/tools/dx.cmd worktree hook guard", root.display())
    );
}
