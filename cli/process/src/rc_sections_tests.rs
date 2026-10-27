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
