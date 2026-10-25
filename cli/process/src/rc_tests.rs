use super::*;

use std::collections::HashMap;

use crate::{build_workflow_argv_with, startup_argv_with};

struct Machine {
    env: HashMap<&'static str, String>,
    files: HashMap<PathBuf, String>,
}

impl Machine {
    fn local() -> Machine {
        let mut env = HashMap::new();
        env.insert("HOME", "/home/u".to_owned());
        env.insert("XDG_CACHE_HOME", "/xdg".to_owned());
        Machine {
            env,
            files: HashMap::new(),
        }
    }

    fn env(mut self, name: &'static str, value: &str) -> Machine {
        self.env.insert(name, value.to_owned());
        self
    }

    fn file(mut self, path: &str, text: &str) -> Machine {
        self.files.insert(PathBuf::from(path), text.to_owned());
        self
    }

    fn policy(&self) -> RcPolicy {
        RcPolicy::from_env(
            &|name| self.env.get(name).cloned(),
            Path::new("/ws"),
            &|path| self.files.get(path).cloned(),
        )
    }
}

fn local_cache() -> Option<String> {
    Some(
        default_disk_cache(Path::new("/xdg"))
            .to_string_lossy()
            .into_owned(),
    )
}

#[test]
fn ci_keeps_bazel_hermetic() {
    let policy = Machine::local().env("CI", "true").policy();
    assert_eq!(policy, RcPolicy::hermetic());
    assert_eq!(policy.startup_opts(), &["--nohome_rc", "--nosystem_rc"]);
    assert!(policy.cache_options("build").is_empty());
}

#[test]
fn bazel_test_keeps_bazel_hermetic() {
    let policy = Machine::local().env("BAZEL_TEST", "1").policy();
    assert_eq!(policy, RcPolicy::hermetic());
}

#[test]
fn ci_other_than_true_and_empty_bazel_test_stay_local() {
    let policy = Machine::local()
        .env("CI", "1")
        .env("BAZEL_TEST", "")
        .policy();
    assert!(policy.read_user_rc);
}

#[test]
fn local_reads_user_rc_and_shares_the_disk_cache() {
    let policy = Machine::local().policy();
    assert!(policy.read_user_rc);
    assert!(policy.startup_opts().is_empty());
    assert_eq!(policy.disk_cache, local_cache());
    assert_eq!(
        policy.cache_options("test"),
        vec![format!(
            "--disk_cache={}",
            local_cache().unwrap_or_default()
        )]
    );
}

#[test]
fn disk_cache_only_follows_build_verbs() {
    let policy = Machine::local().policy();
    for verb in ["build", "test", "coverage", "run"] {
        assert_eq!(policy.cache_options(verb).len(), 1, "{verb}");
    }
    for verb in ["query", "cquery", "aquery", "info", "shutdown"] {
        assert!(policy.cache_options(verb).is_empty(), "{verb}");
    }
}

#[test]
fn any_rc_disk_cache_setting_skips_the_default() {
    let cases = [
        ("/ws/.bazelrc", "build --disk_cache=/d\n"),
        ("/ws/.bazelrc", "build:remote --disk_cache=\n"),
        ("/ws/.bazelrc", "common --nodisk_cache\n"),
        ("/ws/.bazelrc", "build \"--disk_cache=/d d\"\n"),
        ("/home/u/.bazelrc", "common --disk_cache=~/c\n"),
        ("/etc/bazel.bazelrc", "build --disk_cache=/c\n"),
    ];
    for (path, text) in cases {
        let policy = Machine::local().file(path, text).policy();
        assert!(policy.read_user_rc, "{path}: {text}");
        assert_eq!(policy.disk_cache, None, "{path}: {text}");
    }
}

#[test]
fn imported_rc_disk_cache_skips_the_default() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "try-import %workspace%/user.bazelrc\n")
        .file("/ws/user.bazelrc", "import /home/u/shared.bazelrc\n")
        .file("/home/u/shared.bazelrc", "build --disk_cache=/shared\n")
        .policy();
    assert_eq!(policy.disk_cache, None);
}

#[test]
fn relative_import_resolves_under_the_workspace() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "import \"tools/ci.bazelrc\"\n")
        .file("/ws/tools/ci.bazelrc", "build --disk_cache=/c\n")
        .policy();
    assert_eq!(policy.disk_cache, None);
}

#[test]
fn comments_and_other_flags_keep_the_default() {
    let policy = Machine::local()
        .file(
            "/ws/.bazelrc",
            "# build --disk_cache=/old\nbuild --remote_cache=grpc://c\ntry-import %workspace%/missing.bazelrc\n",
        )
        .policy();
    assert_eq!(policy.disk_cache, local_cache());
}

#[test]
fn import_cycle_terminates() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "import %workspace%/a.bazelrc\n")
        .file("/ws/a.bazelrc", "import %workspace%/b.bazelrc\n")
        .file(
            "/ws/b.bazelrc",
            "import %workspace%/a.bazelrc\nimport /ws/.bazelrc\n",
        )
        .policy();
    assert_eq!(policy.disk_cache, local_cache());
}

#[test]
fn deep_import_chain_stops_at_the_depth_cap() {
    let mut machine = Machine::local().file("/ws/.bazelrc", "import /ws/0.bazelrc\n");
    for index in 0..=MAX_IMPORT_DEPTH {
        let path = format!("/ws/{index}.bazelrc");
        let next = format!("import /ws/{}.bazelrc\n", index + 1);
        machine = machine.file(&path, &next);
    }
    let last = format!("/ws/{}.bazelrc", MAX_IMPORT_DEPTH + 1);
    machine = machine.file(&last, "build --disk_cache=/deep\n");
    assert_eq!(machine.policy().disk_cache, local_cache());
}

#[test]
fn user_cache_dir_prefers_xdg_then_home_then_localappdata() {
    let env = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    };
    assert_eq!(
        user_cache_dir(&env(&[("XDG_CACHE_HOME", "/x"), ("HOME", "/h")])),
        Some(PathBuf::from("/x"))
    );
    assert_eq!(
        user_cache_dir(&env(&[("XDG_CACHE_HOME", ""), ("HOME", "/h")])),
        Some(PathBuf::from("/h").join(".cache"))
    );
    assert_eq!(
        user_cache_dir(&env(&[("LOCALAPPDATA", "C:/L")])),
        Some(PathBuf::from("C:/L"))
    );
    assert_eq!(user_cache_dir(&env(&[])), None);
    assert_eq!(
        default_disk_cache(Path::new("/c")),
        PathBuf::from("/c").join("dx").join("disk-cache")
    );
}

#[test]
fn no_cache_dir_means_no_disk_cache() {
    let policy = RcPolicy::from_env(&|_| None, Path::new("/ws"), &|_| None);
    assert!(policy.read_user_rc);
    assert_eq!(policy.disk_cache, None);
}

#[test]
fn configure_real_under_bazel_test_stays_hermetic() {
    configure_real(Path::new("/nonexistent-dx-workspace"));
    assert_eq!(current(), &RcPolicy::hermetic());
    assert!(build_cache_options("build").is_empty());
}

fn local_policy() -> RcPolicy {
    RcPolicy {
        read_user_rc: true,
        disk_cache: Some("/c/dx/disk-cache".to_owned()),
    }
}

#[test]
fn local_startup_argv_drops_the_no_rc_flags() {
    assert_eq!(
        startup_argv_with(&local_policy(), &["--output_base=/o".to_owned()]),
        vec!["bazel".to_owned(), "--output_base=/o".to_owned()]
    );
}

#[test]
fn local_workflow_argv_adds_the_disk_cache_after_the_verb() {
    let argv = build_workflow_argv_with(
        &local_policy(),
        "build",
        &["--disk_cache=".to_owned()],
        &["--config=dx".to_owned()],
        &[],
        &["//...".to_owned()],
        &[],
    )
    .expect("argv");
    assert_eq!(
        argv,
        vec![
            "bazel",
            "build",
            "--disk_cache=/c/dx/disk-cache",
            "--disk_cache=",
            "--config=dx",
            "//...",
        ]
    );
}

#[test]
fn local_query_argv_gets_no_disk_cache() {
    let argv = build_workflow_argv_with(
        &local_policy(),
        "cquery",
        &[],
        &[],
        &[],
        &["//...".to_owned()],
        &[],
    )
    .expect("argv");
    assert_eq!(argv, vec!["bazel", "cquery", "//..."]);
}

#[test]
fn shutdown_startup_opts_follow_the_policy() {
    assert_eq!(current().startup_opts(), WORKFLOW_STARTUP_OPTS);
    assert!(local_policy().startup_opts().is_empty());
}
