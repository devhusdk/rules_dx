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
        let env = |name: &str| (name == "HOME").then(|| "/home/u".to_owned());
        let read = |path: &Path| self.files.get(path).cloned();
        let flags: Vec<String> = flags.iter().map(|flag| (*flag).to_owned()).collect();
        remote_execution(&env, Path::new(WS), &read, self.read_user_rc, &flags)
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

#[test]
fn a_bazelrc_env_file_is_not_read_yet() {
    let files: HashMap<PathBuf, String> = [(
        PathBuf::from("/rc/extra"),
        "build --remote_executor=grpcs://x\n".to_owned(),
    )]
    .into();
    let env = |name: &str| match name {
        "HOME" => Some("/home/u".to_owned()),
        "BAZELRC" => Some("/rc/extra".to_owned()),
        _ => None,
    };
    let read = |path: &Path| files.get(path).cloned();
    assert!(!remote_execution(&env, Path::new(WS), &read, true, &[]));
}
