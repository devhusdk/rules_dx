//! The one-line notice before the first Bazel run in a workspace.

use std::path::{Path, PathBuf};

use dx_plan::args::{Command, Invocation};

/// What dx prints to stderr before the first Bazel call in a workspace.
pub const FIRST_RUN_NOTICE: &str = "dx: first run in this workspace; Bazel will fetch toolchains and dependencies and this can take several minutes";

/// Startup flags that move the output base; an rc that sets one makes the answer unknown.
const ROOT_FLAGS: [&str; 2] = ["output_user_root", "output_base"];

/// The rc files Bazel reads that can move the output root: the workspace `.bazelrc` and what
/// it imports, the user's `~/.bazelrc` and the system rc.
fn rc_files(workspace: &Path, env_get: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let mut files = vec![workspace.join(".bazelrc")];
    if let Some(home) = env_get("HOME")
        .or_else(|| env_get("USERPROFILE"))
        .filter(|home| !home.is_empty())
    {
        files.push(PathBuf::from(home).join(".bazelrc"));
    }
    files.push(PathBuf::from("/etc/bazel.bazelrc"));
    files
}

/// Paths named by `import` and `try-import` lines, with `%workspace%` expanded.
fn rc_imports(text: &str, workspace: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for line in text.lines() {
        let mut tokens = line.split_whitespace();
        if !matches!(tokens.next(), Some("import" | "try-import")) {
            continue;
        }
        let Some(path) = tokens.next() else { continue };
        let path = path.trim_matches('"');
        let path = match path.strip_prefix("%workspace%") {
            Some(rest) => workspace.join(rest.trim_start_matches(['/', '\\'])),
            None => PathBuf::from(path),
        };
        paths.push(path);
    }
    paths
}

/// Whether any rc file Bazel reads for `workspace` names an output root flag.
fn rc_moves_output_root(workspace: &Path, env_get: &dyn Fn(&str) -> Option<String>) -> bool {
    let mut pending = rc_files(workspace, env_get);
    let mut seen: Vec<PathBuf> = Vec::new();
    while let Some(file) = pending.pop() {
        if seen.contains(&file) || seen.len() > 64 {
            continue;
        }
        seen.push(file.clone());
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        if ROOT_FLAGS.iter().any(|flag| text.contains(flag)) {
            return true;
        }
        pending.extend(rc_imports(&text, workspace));
    }
    false
}

/// Whether Bazel has never run in `workspace`: `Some(true)` when no output base records it,
/// `Some(false)` when one does, and `None` when the output root is not where dx can see it.
pub fn first_run(
    workspace: &Path,
    startup_options: &[String],
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<bool> {
    let set = |name: &str| env_get(name).is_some_and(|value| !value.is_empty());
    if cfg!(windows) || set("TEST_TMPDIR") || set("BAZEL_TEST") {
        return None;
    }
    let mut roots = None;
    for option in startup_options {
        if let Some(base) = option.strip_prefix("--output_base=") {
            return Some(!Path::new(base).join("DO_NOT_BUILD_HERE").is_file());
        }
        if let Some(root) = option.strip_prefix("--output_user_root=") {
            roots = Some(vec![PathBuf::from(root)]);
        } else if option.starts_with("--bazelrc") || option.starts_with("--output_") {
            return None;
        }
    }
    let roots = match roots {
        Some(roots) => roots,
        None if rc_moves_output_root(workspace, env_get) => return None,
        None => dx_worktree::host::default_output_roots(env_get),
    };
    Some(!dx_worktree::host::has_output_base(&roots, workspace))
}

/// The notice to print before running `invocation`, if it is a chatty Bazel-backed command
/// in a workspace where Bazel has never run.
pub fn first_run_notice(
    invocation: &Invocation,
    workspace: &Path,
    env_get: &dyn Fn(&str) -> Option<String>,
) -> Option<&'static str> {
    let bazel_backed = matches!(
        invocation.command,
        Command::Build | Command::Test | Command::Coverage | Command::Run | Command::Deploy
    );
    if !bazel_backed || !invocation.chatty() || !workspace.join("MODULE.bazel").is_file() {
        return None;
    }
    match first_run(workspace, &invocation.bazel_startup_options, env_get) {
        Some(true) => Some(FIRST_RUN_NOTICE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dx_output::OutputMode;
    use std::collections::BTreeMap;

    struct Host {
        _scratch: dx_test_scratch::TempDir,
        home: PathBuf,
        workspace: PathBuf,
        env: BTreeMap<String, String>,
    }

    impl Host {
        fn new() -> Host {
            let scratch = dx_test_scratch::scratch("dx-first-run-");
            let home = scratch.path().join("home");
            let workspace = scratch.path().join("ws");
            std::fs::create_dir_all(&home).expect("home");
            std::fs::create_dir_all(&workspace).expect("ws");
            std::fs::write(workspace.join("MODULE.bazel"), "").expect("module");
            let env = BTreeMap::from([
                ("HOME".to_owned(), home.display().to_string()),
                ("USER".to_owned(), "me".to_owned()),
                (
                    "XDG_CACHE_HOME".to_owned(),
                    home.join(".cache").display().to_string(),
                ),
            ]);
            Host {
                _scratch: scratch,
                home,
                workspace,
                env,
            }
        }

        fn get(&self) -> impl Fn(&str) -> Option<String> + '_ {
            |name| self.env.get(name).cloned()
        }

        fn first_run(&self, startup: &[&str]) -> Option<bool> {
            let startup: Vec<String> = startup.iter().map(|s| (*s).to_owned()).collect();
            first_run(&self.workspace, &startup, &self.get())
        }

        fn base(&self, root: &Path, name: &str, workspace: &Path) -> PathBuf {
            let base = root.join(name);
            std::fs::create_dir_all(&base).expect("base");
            std::fs::write(
                base.join("DO_NOT_BUILD_HERE"),
                format!("{}\n", workspace.display()),
            )
            .expect("marker");
            base
        }

        fn default_root(&self) -> PathBuf {
            self.home.join(".cache/bazel/_bazel_me")
        }
    }

    #[test]
    fn no_output_root_is_a_first_run() {
        assert_eq!(Host::new().first_run(&[]), Some(true));
    }

    #[test]
    fn an_output_base_for_this_workspace_is_not_a_first_run() {
        let host = Host::new();
        host.base(&host.default_root(), "abc", &host.workspace);
        assert_eq!(host.first_run(&[]), Some(false));
    }

    #[test]
    fn another_workspace_base_is_still_a_first_run() {
        let host = Host::new();
        host.base(&host.default_root(), "abc", Path::new("/elsewhere/ws"));
        assert_eq!(host.first_run(&[]), Some(true));
    }

    #[test]
    fn explicit_output_base_is_checked_directly() {
        let host = Host::new();
        let base = host.home.join("ob");
        let flag = format!("--output_base={}", base.display());
        assert_eq!(host.first_run(&[&flag]), Some(true));
        host.base(&host.home, "ob", &host.workspace);
        assert_eq!(host.first_run(&[&flag]), Some(false));
    }

    #[test]
    fn explicit_output_user_root_replaces_the_default_roots() {
        let host = Host::new();
        host.base(&host.default_root(), "abc", &host.workspace);
        let root = host.home.join("our");
        let flag = format!("--output_user_root={}", root.display());
        assert_eq!(host.first_run(&[&flag]), Some(true));
        host.base(&root, "abc", &host.workspace);
        assert_eq!(host.first_run(&[&flag]), Some(false));
    }

    #[test]
    fn an_rc_that_moves_the_output_root_makes_it_unknown() {
        let host = Host::new();
        std::fs::write(
            host.home.join(".bazelrc"),
            "startup --output_user_root=/big/disk\n",
        )
        .expect("home rc");
        assert_eq!(host.first_run(&[]), None);
    }

    #[test]
    fn a_workspace_rc_import_that_moves_the_output_root_makes_it_unknown() {
        let host = Host::new();
        std::fs::write(
            host.workspace.join(".bazelrc"),
            "try-import %workspace%/user.bazelrc\n",
        )
        .expect("rc");
        assert_eq!(host.first_run(&[]), Some(true));
        std::fs::write(
            host.workspace.join("user.bazelrc"),
            "startup --output_base=/x\n",
        )
        .expect("user rc");
        assert_eq!(host.first_run(&[]), None);
    }

    #[test]
    fn under_bazel_test_the_output_root_is_unknown() {
        let mut host = Host::new();
        host.env
            .insert("TEST_TMPDIR".to_owned(), "/tmp/t".to_owned());
        assert_eq!(host.first_run(&[]), None);
        host.env.remove("TEST_TMPDIR");
        host.env.insert("BAZEL_TEST".to_owned(), "1".to_owned());
        assert_eq!(host.first_run(&[]), None);
    }

    fn invocation(args: &[&str]) -> Invocation {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        dx_plan::args::parse(&args).expect("parse")
    }

    #[test]
    fn notice_prints_only_for_chatty_bazel_commands() {
        let host = Host::new();
        let get = host.get();
        for args in [&["test"][..], &["build", "//..."], &["run", "//app:bin"]] {
            assert_eq!(
                first_run_notice(&invocation(args), &host.workspace, &get),
                Some(FIRST_RUN_NOTICE),
                "{args:?}"
            );
        }
        for args in [
            &["test", "--quiet"][..],
            &["test", "--output=json"],
            &["lint"],
        ] {
            assert_eq!(
                first_run_notice(&invocation(args), &host.workspace, &get),
                None,
                "{args:?}"
            );
        }
        let json = invocation(&["test", "--output=json"]);
        assert_eq!(json.output, OutputMode::Json);
    }

    #[test]
    fn notice_needs_a_bazel_module() {
        let host = Host::new();
        std::fs::remove_file(host.workspace.join("MODULE.bazel")).expect("rm");
        let get = host.get();
        assert_eq!(
            first_run_notice(&invocation(&["test"]), &host.workspace, &get),
            None
        );
    }
}
