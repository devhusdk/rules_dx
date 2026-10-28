use super::*;

use std::collections::HashMap;

const WS: &str = "/ws";

struct Host {
    files: HashMap<PathBuf, String>,
    read_user_rc: bool,
}

impl Host {
    fn new() -> Host {
        Host {
            files: HashMap::new(),
            read_user_rc: true,
        }
    }

    fn file(mut self, path: &str, text: &str) -> Host {
        self.files.insert(PathBuf::from(path), text.to_owned());
        self
    }

    fn hermetic(mut self) -> Host {
        self.read_user_rc = false;
        self
    }

    fn remote(&self, flags: &[&str]) -> bool {
        self.remote_with(&[], &[("HOME", "/home/u")], flags)
    }

    fn remote_with(&self, startup: &[&str], env: &[(&str, &str)], flags: &[&str]) -> bool {
        let env = |name: &str| {
            env.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        };
        let read = |path: &Path| self.files.get(path).cloned();
        let startup: Vec<String> = startup.iter().map(|opt| (*opt).to_owned()).collect();
        let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
        remote_execution(
            &env,
            Path::new(WS),
            &read,
            self.read_user_rc,
            &startup,
            &flags,
        )
    }
}

const HOME_RC: &str = "/home/u/.bazelrc";
const WS_RC: &str = "/ws/.bazelrc";

#[test]
fn an_enabled_config_sets_remote_execution() {
    let host = Host::new().file(
        HOME_RC,
        "build:rbe --remote_executor=grpcs://x\nbuild --config=rbe\n",
    );
    assert!(host.remote(&[]));
}

#[test]
fn a_config_that_is_never_enabled_does_nothing() {
    let host = Host::new().file(HOME_RC, "build:rbe --remote_executor=grpcs://x\n");
    assert!(!host.remote(&[]));
}

#[test]
fn common_and_nested_configs_expand() {
    let common = Host::new().file(
        WS_RC,
        "common:x --remote_executor=grpc://a\ncommon --config=x\n",
    );
    assert!(common.remote(&[]));
    let nested = Host::new().file(
        WS_RC,
        "build:inner --remote_executor=grpc://a\nbuild:outer --config=inner\nbuild --config outer\n",
    );
    assert!(nested.remote(&[]));
}

#[test]
fn a_config_cycle_ends() {
    let host = Host::new().file(
        WS_RC,
        "build:a --config=b\nbuild:b --config=a\nbuild:b --remote_executor=grpc://a\nbuild --config=a\n",
    );
    assert!(host.remote(&[]));
}

#[test]
fn passed_flags_come_last() {
    let host = Host::new().file(WS_RC, "build:rbe --remote_executor=grpcs://x\n");
    assert!(host.remote(&["--config=rbe"]));
    assert!(!host.remote(&["--config=rbe", "--remote_executor="]));
    assert!(host.remote(&["--remote_executor", "grpcs://y"]));
}

#[test]
fn home_rc_overrides_workspace_rc() {
    let host = Host::new()
        .file(WS_RC, "build --remote_executor=grpcs://x\n")
        .file(HOME_RC, "build --remote_executor=\n");
    assert!(!host.remote(&[]));
}

#[test]
fn build_lines_beat_common_lines() {
    let host = Host::new()
        .file(WS_RC, "build --remote_executor=grpcs://x\n")
        .file(HOME_RC, "common --remote_executor=\n");
    assert!(host.remote(&[]));
}

#[test]
fn a_test_only_executor_does_not_count() {
    let host = Host::new().file(WS_RC, "test --remote_executor=grpcs://x\n");
    assert!(!host.remote(&[]));
}

#[test]
fn platform_configs_expand_when_enabled() {
    let os = std::env::consts::OS;
    let text = format!(
        "common --enable_platform_specific_config\nbuild:{os} --remote_executor=grpc://a\n"
    );
    assert!(Host::new().file(WS_RC, &text).remote(&[]));
    let off = format!("build:{os} --remote_executor=grpc://a\n");
    assert!(!Host::new().file(WS_RC, &off).remote(&[]));
}

#[test]
fn imports_are_read_in_place() {
    let host = Host::new()
        .file(WS_RC, "import %workspace%/tools/remote.bazelrc\n")
        .file(
            "/ws/tools/remote.bazelrc",
            "build --remote_executor=grpc://a\n",
        );
    assert!(host.remote(&[]));
    let later = Host::new()
        .file(
            WS_RC,
            "try-import %workspace%/tools/remote.bazelrc\nbuild --remote_executor=\n",
        )
        .file(
            "/ws/tools/remote.bazelrc",
            "build --remote_executor=grpc://a\n",
        );
    assert!(!later.remote(&[]));
}

#[test]
fn hermetic_runs_skip_the_home_rc() {
    let host = Host::new()
        .file(HOME_RC, "build --remote_executor=grpcs://x\n")
        .hermetic();
    assert!(!host.remote(&[]));
}

#[test]
fn split_and_quoted_tokens_parse() {
    let split = Host::new().file(WS_RC, "build --remote_executor grpcs://x\n");
    assert!(split.remote(&[]));
    let quoted = Host::new().file(WS_RC, "build \"--remote_executor=grpcs://x\"\n");
    assert!(quoted.remote(&[]));
    let comment = Host::new().file(WS_RC, "# build --remote_executor=grpcs://x\n");
    assert!(!comment.remote(&[]));
}

const EXTRA_RC: &str = "/rc/extra";
const REMOTE_RC: &str = "build --remote_executor=grpcs://x\n";

#[test]
fn a_bazelrc_env_file_sets_remote_execution() {
    let host = Host::new().file(EXTRA_RC, REMOTE_RC);
    assert!(host.remote_with(&[], &[("HOME", "/home/u"), ("BAZELRC", EXTRA_RC)], &[]));
}

#[test]
fn a_comma_separated_bazelrc_env_is_read_in_order() {
    let host = Host::new().file("/rc/b", REMOTE_RC);
    let env = [("BAZELRC", "/rc/a,/rc/b")];
    assert!(host.remote_with(&[], &env, &[]));
}

#[test]
fn a_bazelrc_startup_option_sets_remote_execution() {
    let host = Host::new().file(EXTRA_RC, REMOTE_RC);
    assert!(host.remote_with(&["--bazelrc=/rc/extra"], &[], &[]));
    assert!(host.remote_with(&["--bazelrc", "/rc/extra"], &[], &[]));
}

#[test]
fn a_dev_null_bazelrc_ends_the_list() {
    let host = Host::new().file(EXTRA_RC, REMOTE_RC);
    assert!(!host.remote_with(&["--bazelrc=/dev/null", "--bazelrc=/rc/extra"], &[], &[]));
}

#[test]
fn bazelrc_files_are_not_read_when_hermetic() {
    let host = Host::new().file(EXTRA_RC, REMOTE_RC).hermetic();
    assert!(!host.remote_with(&["--bazelrc=/rc/extra"], &[("BAZELRC", EXTRA_RC)], &[]));
}

fn options(host: &Host, command: &str, flags: &[&str]) -> Vec<String> {
    let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
    let lines = load_rc(
        &|_| None,
        Path::new(WS),
        &|path| host.files.get(path).cloned(),
        host.read_user_rc,
        &[],
    );
    effective_options(&lines, command, &flags)
}

#[test]
fn command_levels_follow_bazel_inheritance() {
    let host = Host::new().file(
        "/ws/.bazelrc",
        "common --a=1\nbuild --a=2\ntest --a=3\nrun --a=4\ncoverage --a=5\n",
    );
    let last = |command: &str| {
        let options = options(&host, command, &[]);
        last_value(&options, "--a").map(str::to_owned)
    };
    assert_eq!(last("build").as_deref(), Some("2"));
    assert_eq!(last("test").as_deref(), Some("3"));
    assert_eq!(last("coverage").as_deref(), Some("5"));
    assert_eq!(last("run").as_deref(), Some("4"));
}

#[test]
fn configs_respect_command_levels() {
    let host = Host::new().file("/ws/.bazelrc", "test:x --a=t\nbuild:x --a=b\n");
    let flags = ["--config=x"];
    assert_eq!(options(&host, "build", &flags), ["--a=b"]);
    assert_eq!(options(&host, "run", &flags), ["--a=b"]);
    assert_eq!(options(&host, "test", &flags), ["--a=b", "--a=t"]);
}

#[test]
fn a_quoted_value_with_a_space_is_one_token() {
    let host = Host::new().file("/ws/.bazelrc", "build \"--disk_cache=/d d\" --x\n");
    assert_eq!(options(&host, "build", &[]), ["--disk_cache=/d d", "--x"]);
}

fn nested_selected(text: &str) -> bool {
    let read = |path: &Path| (path == Path::new(WS_RC)).then(|| text.to_owned());
    nested_bazel_selected(&|_| None, Path::new(WS), &read, false, &[], &[])
}

#[test]
fn test_tag_filters_decide_whether_nested_bazel_tests_run() {
    assert!(nested_selected(""));
    assert!(nested_selected("test --test_tag_filters=-exclusive\n"));
    assert!(!nested_selected(
        "test --test_tag_filters=-resources:nested_bazel:1\n"
    ));
    assert!(!nested_selected("common --test_tag_filters=smoke\n"));
    assert!(nested_selected(
        "test --test_tag_filters=resources:nested_bazel:1\n"
    ));
    assert!(nested_selected(
        "test --test_tag_filters=-resources:nested_bazel:1\ntest --test_tag_filters=\n"
    ));
    assert!(!nested_selected(
        "build --test_tag_filters=-resources:nested_bazel\n"
    ));
}

fn paths(host: &Host, os: &str, startup: &[&str], env: &[(&str, &str)]) -> StartupPaths {
    let env = |name: &str| {
        env.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    };
    let read = |path: &Path| host.files.get(path).cloned();
    let startup: Vec<String> = startup.iter().map(|opt| (*opt).to_owned()).collect();
    startup_output_paths_for_os(&env, Path::new(WS), &read, host.read_user_rc, &startup, os)
}

fn root(host: &Host) -> Option<PathBuf> {
    paths(host, "linux", &[], &[("HOME", "/home/u")]).output_user_root
}

#[test]
fn startup_root_reads_both_forms_and_last_wins() {
    let eq = Host::new().file(WS_RC, "startup --output_user_root=/a\n");
    assert_eq!(root(&eq), Some(PathBuf::from("/a")));
    let split = Host::new().file(WS_RC, "startup --output_user_root /b\n");
    assert_eq!(root(&split), Some(PathBuf::from("/b")));
    let home = Host::new()
        .file(WS_RC, "startup --output_user_root=/a\n")
        .file(HOME_RC, "startup --output_user_root=/h\n");
    assert_eq!(root(&home), Some(PathBuf::from("/h")));
}

#[test]
fn startup_argv_beats_rc() {
    let host = Host::new().file(WS_RC, "startup --output_base=/a\n");
    let got = paths(&host, "linux", &["--output_base=/c"], &[]);
    assert_eq!(got.output_base, Some(PathBuf::from("/c")));
}

#[test]
fn startup_os_lines_apply_only_on_the_host_os() {
    let host = Host::new().file(
        WS_RC,
        "startup:linux --output_user_root=/l\nstartup:foo --output_user_root=/f\n",
    );
    assert_eq!(root(&host), Some(PathBuf::from("/l")));
    assert_eq!(paths(&host, "windows", &[], &[]).output_user_root, None);
}

#[test]
fn startup_follows_try_import_and_bazelrc_env() {
    let host = Host::new()
        .file(WS_RC, "try-import /x/extra.rc\n")
        .file("/x/extra.rc", "startup --output_user_root=/imp\n")
        .file("/y/env.rc", "startup --output_base=/envb\n");
    assert_eq!(root(&host), Some(PathBuf::from("/imp")));
    let got = paths(&host, "linux", &[], &[("BAZELRC", "/y/env.rc")]);
    assert_eq!(got.output_base, Some(PathBuf::from("/envb")));
}

#[test]
fn startup_hermetic_skips_home_and_build_lines_are_ignored() {
    let host = Host::new()
        .file(HOME_RC, "startup --output_user_root=/h\n")
        .file(WS_RC, "build --output_base=/nope\n")
        .hermetic();
    let got = paths(&host, "linux", &[], &[("HOME", "/home/u")]);
    assert_eq!(got, StartupPaths::default());
}

#[test]
fn startup_relative_joins_workspace_and_empty_clears() {
    let rel = Host::new().file(WS_RC, "startup --output_user_root=out\n");
    assert_eq!(root(&rel), Some(PathBuf::from("/ws/out")));
    let empty = Host::new().file(
        WS_RC,
        "startup --output_user_root=/a\nstartup --output_user_root=\n",
    );
    assert_eq!(root(&empty), None);
}
