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
    base: PathBuf,
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
        // Git must not find the enclosing checkout: a bazel test scratch dir sits
        // under the execroot, whose `.git` links to the source repository.
        .env("GIT_CEILING_DIRECTORIES", &env.base)
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
        base,
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
        base: env.base.clone(),
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
    // `bazel run` sets both variables. Without BUILD_WORKSPACE_DIRECTORY, dx
    // walks up from the runfiles dir and finds the execroot's MODULE.bazel.
    let user_dir = [
        ("BUILD_WORKSPACE_DIRECTORY", root.to_str().expect("utf8")),
        ("BUILD_WORKING_DIRECTORY", root.to_str().expect("utf8")),
    ];
    let run = |args: &[&str], stdin: &str| run_env(dx, args, &runfiles, &env, stdin, &user_dir);

    let created = run(&["worktree", "hook", "create"], "{\"name\":\"two\"}");
    assert_eq!(created.code, 0, "{}{}", created.stdout, created.stderr);
    assert_eq!(created.stdout.lines().count(), 1, "{}", created.stdout);
    let path = PathBuf::from(created.stdout.trim());
    assert!(path.join(".git").exists(), "{path:?}");
    // The slot must belong to the scratch repo, not the rules_dx checkout that
    // the execroot's `.git` and `MODULE.bazel` expose to this test.
    assert!(
        path.starts_with(env.home.join(".cache/dx/worktrees/app")),
        "{path:?}"
    );

    let removed = run(
        &["worktree", "hook", "remove"],
        &format!("{{\"worktree_path\":{:?}}}", path.to_str().expect("utf8")),
    );
    assert_eq!(removed.code, 0, "{}{}", removed.stdout, removed.stderr);
}
