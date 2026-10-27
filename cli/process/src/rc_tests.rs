use super::*;

use std::collections::HashMap;

use crate::{build_workflow_argv_with, startup_argv_with, ArgvPolicy};

struct Machine {
    env: HashMap<&'static str, String>,
    files: HashMap<PathBuf, String>,
}

impl Machine {
    fn local() -> Machine {
        let mut env = HashMap::new();
        env.insert("HOME", "/home/u".to_owned());
        env.insert("XDG_CACHE_HOME", "/xdg".to_owned());
        env.insert("ProgramData", "/programdata".to_owned());
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

fn gc_pair() -> [String; 2] {
    [
        format!("--experimental_disk_cache_gc_max_size={DISK_CACHE_GC_MAX_SIZE}"),
        format!("--experimental_disk_cache_gc_max_age={DISK_CACHE_GC_MAX_AGE}"),
    ]
}

fn cache_and_gc() -> Vec<String> {
    let mut options = vec![format!(
        "--disk_cache={}",
        local_cache().unwrap_or_default()
    )];
    options.extend(gc_pair());
    options
}

#[test]
fn ci_keeps_bazel_hermetic() {
    let policy = Machine::local().env("CI", "true").policy();
    assert_eq!(policy, RcPolicy::hermetic());
    assert_eq!(policy.startup_opts(), &["--nohome_rc", "--nosystem_rc"]);
    assert!(policy.verb_options("build", None, &[]).is_empty());
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
    assert!(policy.disk_cache_gc);
    assert_eq!(policy.gc_idle_delay_secs, None);
    assert_eq!(policy.verb_options("test", None, &[]), cache_and_gc());
}

#[test]
fn disk_cache_only_follows_build_verbs() {
    let policy = Machine::local().policy();
    for verb in ["build", "test", "coverage", "run"] {
        assert_eq!(
            policy.verb_options(verb, None, &[]),
            cache_and_gc(),
            "{verb}"
        );
    }
    for verb in ["query", "cquery", "aquery", "info", "shutdown"] {
        assert!(policy.verb_options(verb, None, &[]).is_empty(), "{verb}");
    }
}

#[test]
fn any_rc_disk_cache_setting_skips_the_default() {
    let system = if cfg!(windows) {
        "/programdata/bazel.bazelrc"
    } else {
        "/etc/bazel.bazelrc"
    };
    let cases = [
        ("/ws/.bazelrc", "build --disk_cache=/d\n", Some("/d")),
        ("/ws/.bazelrc", "build:remote --disk_cache=\n", None),
        ("/ws/.bazelrc", "common --nodisk_cache\n", None),
        (
            "/ws/.bazelrc",
            "build \"--disk_cache=/d d\"\n",
            Some("/d d"),
        ),
        (
            "/home/u/.bazelrc",
            "common --disk_cache=~/c\n",
            Some("/home/u/c"),
        ),
        (system, "build --disk_cache=/c\n", Some("/c")),
        ("/ws/.bazelrc", "test --disk_cache=/t\n", None),
    ];
    for (path, text, resolved) in cases {
        let policy = Machine::local().file(path, text).policy();
        assert!(policy.read_user_rc, "{path}: {text}");
        assert_eq!(policy.disk_cache, None, "{path}: {text}");
        assert_eq!(
            policy.bazel_disk_cache,
            resolved.map(PathBuf::from),
            "{path}: {text}"
        );
    }
}

fn resolved(machine: Machine) -> Option<PathBuf> {
    machine.policy().bazel_disk_cache
}

#[test]
fn disk_cache_precedence_follows_bazel_read_order() {
    let all = Machine::local()
        .file("/etc/bazel.bazelrc", "build --disk_cache=/sys\n")
        .file("/ws/.bazelrc", "build --disk_cache=/ws-cache\n")
        .file("/home/u/.bazelrc", "build --disk_cache=/home\n");
    assert_eq!(resolved(all), Some(PathBuf::from("/home")));
    let ws = Machine::local()
        .file("/etc/bazel.bazelrc", "build --disk_cache=/sys\n")
        .file("/ws/.bazelrc", "build --disk_cache=/ws-cache\n");
    assert_eq!(resolved(ws), Some(PathBuf::from("/ws-cache")));
    let build_over_common = Machine::local()
        .file("/etc/bazel.bazelrc", "build --disk_cache=/sys\n")
        .file("/home/u/.bazelrc", "common --disk_cache=/home\n");
    assert_eq!(resolved(build_over_common), Some(PathBuf::from("/sys")));
    let last_line = Machine::local().file(
        "/ws/.bazelrc",
        "build --disk_cache=/one\nbuild --disk_cache=/two\n",
    );
    assert_eq!(resolved(last_line), Some(PathBuf::from("/two")));
}

#[test]
fn disk_cache_forms_and_config_groups_resolve() {
    let split = Machine::local().file("/ws/.bazelrc", "build --disk_cache /x\n");
    assert_eq!(resolved(split), Some(PathBuf::from("/x")));
    for value in ["c", "%workspace%/c"] {
        let machine =
            Machine::local().file("/ws/.bazelrc", &format!("build --disk_cache={value}\n"));
        assert_eq!(resolved(machine), Some(PathBuf::from("/ws/c")), "{value}");
    }
    let config = Machine::local().file(
        "/ws/.bazelrc",
        "build:ci --disk_cache=/x\ncommon --disk_cache=/y\n",
    );
    assert_eq!(resolved(config), Some(PathBuf::from("/y")));
}

#[test]
fn no_rc_setting_reports_the_dx_default() {
    let policy = Machine::local().policy();
    assert_eq!(policy.bazel_disk_cache, local_cache().map(PathBuf::from));
    assert_eq!(RcPolicy::hermetic().bazel_disk_cache, None);
}

#[test]
fn imported_rc_disk_cache_skips_the_default() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "try-import %workspace%/user.bazelrc\n")
        .file("/ws/user.bazelrc", "import /home/u/shared.bazelrc\n")
        .file("/home/u/shared.bazelrc", "build --disk_cache=/shared\n")
        .policy();
    assert_eq!(policy.disk_cache, None);
    assert_eq!(policy.bazel_disk_cache, Some(PathBuf::from("/shared")));
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
    assert!(build_verb_options("build", &[]).is_empty());
}

fn local_policy() -> RcPolicy {
    RcPolicy {
        read_user_rc: true,
        disk_cache: Some("/c/dx/disk-cache".to_owned()),
        bazel_disk_cache: Some(std::path::PathBuf::from("/c/dx/disk-cache")),
        local_cpu: true,
        local_memory: true,
        disk_cache_gc: true,
        gc_idle_delay_secs: None,
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
        ArgvPolicy {
            rc: &local_policy(),
            share: None,
        },
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
        ArgvPolicy {
            rc: &local_policy(),
            share: Some(&SHARE_4),
        },
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

const SHARE_4: LocalShare = LocalShare {
    cpus: 4,
    memory_mb: 8192,
};

fn workflow(
    policy: &RcPolicy,
    share: Option<&LocalShare>,
    verb: &str,
    user: &[&str],
) -> Vec<String> {
    let user: Vec<String> = user.iter().map(|arg| (*arg).to_owned()).collect();
    build_workflow_argv_with(
        ArgvPolicy { rc: policy, share },
        verb,
        &user,
        &["--config=dx".to_owned()],
        &[],
        &["//...".to_owned()],
        &[],
    )
    .expect("argv")
}

#[test]
fn a_share_adds_local_resources_after_the_disk_cache() {
    for verb in ["build", "test", "coverage", "run"] {
        assert_eq!(
            workflow(&local_policy(), Some(&SHARE_4), verb, &[]),
            vec![
                "bazel",
                verb,
                "--disk_cache=/c/dx/disk-cache",
                "--experimental_disk_cache_gc_max_size=20G",
                "--experimental_disk_cache_gc_max_age=14d",
                "--local_resources=cpu=4",
                "--local_resources=memory=8192",
                "--config=dx",
                "//...",
            ],
            "{verb}"
        );
    }
    for verb in ["query", "info"] {
        assert_eq!(
            workflow(&local_policy(), Some(&SHARE_4), verb, &[]),
            vec!["bazel", verb, "--config=dx", "//..."],
            "{verb}"
        );
    }
}

#[test]
fn no_share_adds_no_local_resources() {
    assert_eq!(
        workflow(&local_policy(), None, "build", &[]),
        vec![
            "bazel",
            "build",
            "--disk_cache=/c/dx/disk-cache",
            "--experimental_disk_cache_gc_max_size=20G",
            "--experimental_disk_cache_gc_max_age=14d",
            "--config=dx",
            "//..."
        ]
    );
}

#[test]
fn hermetic_adds_no_local_resources() {
    let hermetic = RcPolicy::hermetic();
    assert_eq!(
        hermetic.verb_options("build", Some(&SHARE_4), &[]),
        Vec::<String>::new()
    );
    assert!(!hermetic.local_cpu && !hermetic.local_memory);
    assert!(!hermetic.disk_cache_gc);
    assert_eq!(hermetic.gc_idle_delay_secs, None);
    let ci = Machine::local().env("CI", "true").policy();
    assert!(!ci.local_cpu && !ci.local_memory);
}

fn resources(policy: &RcPolicy) -> (bool, bool) {
    (policy.local_cpu, policy.local_memory)
}

#[test]
fn an_rc_resource_suppresses_only_that_resource() {
    let cases = [
        ("common --local_resources=cpu=4\n", (false, true)),
        ("build --local_resources=memory=4096\n", (true, false)),
        (
            "build --local_resources memory=HOST_RAM*.3\n",
            (true, false),
        ),
        (
            "build \"--local_resources=cpu=HOST_CPUS-2\"\n",
            (false, true),
        ),
        ("build --local_cpu_resources=6\n", (false, true)),
        ("build --local_ram_resources=HOST_RAM*.4\n", (true, false)),
    ];
    for (text, want) in cases {
        let policy = Machine::local().file("/home/u/.bazelrc", text).policy();
        assert_eq!(resources(&policy), want, "{text}");
        assert_eq!(policy.disk_cache, local_cache(), "{text}");
    }
    let policy = Machine::local()
        .file("/ws/.bazelrc", "common --local_resources=cpu=4\n")
        .policy();
    assert_eq!(
        policy.verb_options("build", Some(&SHARE_4), &[]),
        [
            cache_and_gc(),
            vec!["--local_resources=memory=8192".to_owned()]
        ]
        .concat()
    );
}

#[test]
fn preset_values_and_custom_resources_suppress_nothing() {
    let policy = Machine::local()
        .file(
            "/ws/.bazelrc",
            "common --local_resources=memory=HOST_RAM*.5\ncommon --local_resources=cpu=HOST_CPUS*.75\ncommon --local_resources=nested_bazel=2\n# common --local_resources=cpu=1\n",
        )
        .policy();
    assert_eq!(resources(&policy), (true, true));
}

#[test]
fn imported_rc_resources_are_honored() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "try-import %workspace%/user.bazelrc\n")
        .file("/ws/user.bazelrc", "import /home/u/shared.bazelrc\n")
        .file(
            "/home/u/shared.bazelrc",
            "build --local_resources=memory=2048\n",
        )
        .policy();
    assert_eq!(resources(&policy), (true, false));
}

#[test]
fn a_user_resource_after_the_separator_suppresses_only_that_resource() {
    assert_eq!(
        workflow(
            &local_policy(),
            Some(&SHARE_4),
            "test",
            &["--local_resources=memory=1000"]
        ),
        vec![
            "bazel",
            "test",
            "--disk_cache=/c/dx/disk-cache",
            "--experimental_disk_cache_gc_max_size=20G",
            "--experimental_disk_cache_gc_max_age=14d",
            "--local_resources=cpu=4",
            "--local_resources=memory=1000",
            "--config=dx",
            "//...",
        ]
    );
}

#[test]
fn the_vendored_preset_suppresses_nothing() {
    let path = std::env::var("DX_PRESET").expect("DX_PRESET");
    let preset = std::fs::read_to_string(&path).expect("preset");
    for value in PRESET_LOCAL_RESOURCES {
        assert!(
            preset.contains(&format!("common --local_resources={value}\n")),
            "{value}"
        );
    }
    for dir in ["tools/bazelrc", ".dx/bazelrc"] {
        let policy = Machine::local()
            .file(
                "/ws/.bazelrc",
                &format!("import %workspace%/{dir}/resources.bazelrc\n"),
            )
            .file(&format!("/ws/{dir}/resources.bazelrc"), &preset)
            .policy();
        assert_eq!(resources(&policy), (true, true), "{dir}");
    }
}

#[test]
fn a_share_round_trips_through_its_env_value() {
    assert_eq!(SHARE_4.to_env(), "cpu=4,memory=8192");
    assert_eq!(LocalShare::parse("cpu=4,memory=8192"), Some(SHARE_4));
    for bad in [
        "",
        "cpu=4",
        "cpu=x,memory=1",
        "memory=1,cpu=4",
        "cpu=0,memory=1",
        "cpu=1,memory=-1",
    ] {
        assert_eq!(LocalShare::parse(bad), None, "{bad}");
    }
}

#[test]
fn the_process_share_clears_only_its_own_value() {
    set_share(Some(SHARE_4));
    assert_eq!(share(), Some(SHARE_4));
    clear_share(&LocalShare {
        cpus: 2,
        memory_mb: 1,
    });
    assert_eq!(share(), Some(SHARE_4));
    clear_share(&SHARE_4);
    assert_eq!(share(), None);
}

fn idle_delay(secs: u64) -> String {
    format!("--experimental_disk_cache_gc_idle_delay={secs}s")
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

#[test]
fn the_gc_pair_follows_only_build_verbs_and_never_hermetic_runs() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "startup --max_idle_secs=90\n")
        .policy();
    for verb in ["query", "cquery", "info", "shutdown"] {
        assert!(policy.verb_options(verb, None, &[]).is_empty(), "{verb}");
    }
    for env in [("CI", "true"), ("BAZEL_TEST", "1")] {
        let policy = Machine::local()
            .env(env.0, env.1)
            .file("/ws/.bazelrc", "startup --max_idle_secs=90\n")
            .policy();
        assert_eq!(policy, RcPolicy::hermetic(), "{env:?}");
        assert!(policy.verb_options("build", None, &[]).is_empty());
    }
}

#[test]
fn any_rc_gc_bound_drops_the_pair_and_keeps_the_cache() {
    let cases = [
        (
            "/ws/.bazelrc",
            "common --experimental_disk_cache_gc_max_size=40G\n",
        ),
        (
            "/ws/.bazelrc",
            "build:x --experimental_disk_cache_gc_max_age=3d\n",
        ),
        (
            "/ws/.bazelrc",
            "build \"--experimental_disk_cache_gc_max_age=3d\"\n",
        ),
        (
            "/ws/.bazelrc",
            "build --experimental_disk_cache_gc_max_size 40G\n",
        ),
        (
            "/home/u/.bazelrc",
            "common --experimental_disk_cache_gc_max_size=1G\n",
        ),
        (
            "/etc/bazel.bazelrc",
            "common --experimental_disk_cache_gc_max_age=1d\n",
        ),
    ];
    for (path, text) in cases {
        let policy = Machine::local().file(path, text).policy();
        assert_eq!(policy.disk_cache, local_cache(), "{path}: {text}");
        assert!(!policy.disk_cache_gc, "{path}: {text}");
        assert_eq!(
            policy.verb_options("build", None, &[]),
            vec![format!(
                "--disk_cache={}",
                local_cache().unwrap_or_default()
            )],
            "{path}: {text}"
        );
    }
    let imported = Machine::local()
        .file("/ws/.bazelrc", "try-import %workspace%/user.bazelrc\n")
        .file(
            "/ws/user.bazelrc",
            "common --experimental_disk_cache_gc_max_size=5G\n",
        )
        .policy();
    assert!(!imported.disk_cache_gc);
}

#[test]
fn a_longer_flag_with_the_same_prefix_keeps_the_pair() {
    let policy = Machine::local()
        .file(
            "/ws/.bazelrc",
            "common --experimental_disk_cache_gc_max_size_extra=1\n# common --experimental_disk_cache_gc_max_age=1d\n",
        )
        .policy();
    assert!(policy.disk_cache_gc);
}

#[test]
fn an_rc_disk_cache_drops_the_cache_and_the_pair() {
    let policy = Machine::local()
        .file("/ws/.bazelrc", "build --disk_cache=/d\n")
        .policy();
    assert_eq!(policy.disk_cache, None);
    assert!(!policy.disk_cache_gc);
    assert!(policy.verb_options("build", None, &[]).is_empty());
}

#[test]
fn user_gc_or_cache_args_drop_the_pair() {
    let policy = Machine::local().policy();
    let cache = format!("--disk_cache={}", local_cache().unwrap_or_default());
    for user in [
        &["--experimental_disk_cache_gc_max_size=40G"][..],
        &["--experimental_disk_cache_gc_max_age", "1d"],
        &["--disk_cache=/x"],
        &["--disk_cache", "/x"],
    ] {
        assert_eq!(
            policy.verb_options("build", None, &strings(user)),
            vec![cache.clone()],
            "{user:?}"
        );
    }
    assert_eq!(
        policy.verb_options("build", None, &strings(&["--config=dx"])),
        cache_and_gc()
    );
}

#[test]
fn a_short_max_idle_secs_adds_an_idle_delay_under_it() {
    let policy = Machine::local()
        .file("/home/u/.bazelrc", "startup --max_idle_secs=90\n")
        .policy();
    assert_eq!(policy.gc_idle_delay_secs, Some(30));
    assert_eq!(
        policy.verb_options("build", None, &[]),
        [cache_and_gc(), vec![idle_delay(30)]].concat()
    );
    let own = Machine::local()
        .file(
            "/home/u/.bazelrc",
            "startup --max_idle_secs 90\ncommon --disk_cache=~/c\ncommon --experimental_disk_cache_gc_max_size=20G\n",
        )
        .policy();
    assert_eq!(own.disk_cache, None);
    assert_eq!(own.verb_options("test", None, &[]), vec![idle_delay(30)]);
    for (text, want) in [
        ("startup --max_idle_secs=900\n", None),
        ("startup --max_idle_secs=899\n", Some(299)),
        ("startup --max_idle_secs=0\n", None),
        ("startup --max_idle_secs=2\n", Some(1)),
        ("startup --max_idle_secs=x\n", None),
        ("# startup --max_idle_secs=90\n", None),
        ("build --max_idle_secs=90\n", None),
    ] {
        let policy = Machine::local().file("/ws/.bazelrc", text).policy();
        assert_eq!(policy.gc_idle_delay_secs, want, "{text}");
    }
}

#[test]
fn the_smallest_max_idle_secs_across_rc_files_wins() {
    let policy = Machine::local()
        .file(
            "/ws/.bazelrc",
            "startup --max_idle_secs=300\nimport /ws/a.bazelrc\n",
        )
        .file("/ws/a.bazelrc", "startup --max_idle_secs=90\n")
        .file("/home/u/.bazelrc", "startup --max_idle_secs=600\n")
        .policy();
    assert_eq!(policy.gc_idle_delay_secs, Some(30));
}

#[test]
fn a_set_idle_delay_drops_the_dx_delay() {
    let policy = Machine::local()
        .file(
            "/home/u/.bazelrc",
            "startup --max_idle_secs=90\ncommon --experimental_disk_cache_gc_idle_delay=10s\n",
        )
        .policy();
    assert_eq!(policy.gc_idle_delay_secs, None);
    let policy = Machine::local()
        .file("/home/u/.bazelrc", "startup --max_idle_secs=90\n")
        .policy();
    for user in [
        &["--experimental_disk_cache_gc_idle_delay=1m"][..],
        &["--experimental_disk_cache_gc_idle_delay", "1m"],
    ] {
        assert_eq!(
            policy.verb_options("build", None, &strings(user)),
            cache_and_gc(),
            "{user:?}"
        );
    }
}

#[test]
fn the_idle_scan_survives_cycles_and_the_depth_cap() {
    let cycle = Machine::local()
        .file(
            "/ws/.bazelrc",
            "import %workspace%/a.bazelrc\nstartup --max_idle_secs=60\n",
        )
        .file("/ws/a.bazelrc", "import /ws/.bazelrc\n")
        .policy();
    assert_eq!(cycle.gc_idle_delay_secs, Some(20));
    let mut machine = Machine::local().file("/ws/.bazelrc", "import /ws/0.bazelrc\n");
    for index in 0..=MAX_IMPORT_DEPTH {
        let path = format!("/ws/{index}.bazelrc");
        let next = format!("import /ws/{}.bazelrc\n", index + 1);
        machine = machine.file(&path, &next);
    }
    let last = format!("/ws/{}.bazelrc", MAX_IMPORT_DEPTH + 1);
    machine = machine.file(&last, "startup --max_idle_secs=90\n");
    assert_eq!(machine.policy().gc_idle_delay_secs, None);
}

fn repo_caches(machine: &Machine, startup: &[&str]) -> Vec<PathBuf> {
    repository_caches(
        &|name| machine.env.get(name).cloned(),
        Path::new("/ws"),
        &|path| machine.files.get(path).cloned(),
        &strings(startup),
    )
}

fn user_default(base: &str) -> PathBuf {
    PathBuf::from(base).join("_bazel_me/cache/repos/v1")
}

#[test]
fn repository_cache_flags_come_first_in_read_order() {
    let machine = Machine::local()
        .file(
            "/ws/.bazelrc",
            "build --repository_cache=/a\ncommon:ci --repository_cache /b\nimport %workspace%/more.rc",
        )
        .file("/ws/more.rc", "fetch --repository_cache=%workspace%/c")
        .file("/home/u/.bazelrc", "common --repository_cache=~/d");
    assert_eq!(
        repo_caches(&machine, &[]),
        vec![
            PathBuf::from("/a"),
            PathBuf::from("/b"),
            PathBuf::from("/ws/c"),
            PathBuf::from("/home/u/d"),
        ]
    );
}

#[test]
fn empty_and_negated_repository_caches_are_skipped_and_duplicates_removed() {
    let machine = Machine::local().file(
        "/ws/.bazelrc",
        "build --repository_cache=\nbuild --norepository_cache\nbuild --repository_cache=/a\ntest --repository_cache=/a\nbuild --repository_cache_urls_as_default_canonical_id",
    );
    assert_eq!(repo_caches(&machine, &[]), vec![PathBuf::from("/a")]);
}

#[test]
fn output_user_roots_follow_dx_startup_options_then_rc() {
    let machine = Machine::local().file(
        "/ws/.bazelrc",
        "startup --output_user_root=/rc\nstartup:linux --output_user_root /rc2\nbuild --output_user_root=/ignored",
    );
    let caches = repo_caches(&machine, &["--output_user_root=/dx"]);
    assert_eq!(
        caches[..3],
        [
            PathBuf::from("/dx/cache/repos/v1"),
            PathBuf::from("/rc/cache/repos/v1"),
            PathBuf::from("/rc2/cache/repos/v1"),
        ]
    );
    assert!(!caches.contains(&PathBuf::from("/ignored/cache/repos/v1")));
}

#[test]
fn default_user_roots_follow_test_tmpdir_then_xdg_then_home() {
    let machine = Machine::local().env("USER", "me").env("TEST_TMPDIR", "/tt");
    let caches = repo_caches(&machine, &[]);
    assert_eq!(
        caches[..5],
        [
            user_default("/tt"),
            user_default("/xdg/bazel"),
            user_default("/home/u/.cache/bazel"),
            user_default("/home/u/Library/Caches/bazel"),
            user_default("/private/var/tmp"),
        ]
    );
}

#[test]
fn no_user_name_means_no_default_user_roots() {
    let machine = Machine::local();
    assert!(repo_caches(&machine, &[]).is_empty());
    let logname = Machine::local().env("LOGNAME", "me");
    assert_eq!(
        repo_caches(&logname, &[]).first(),
        Some(&user_default("/xdg/bazel"))
    );
}
