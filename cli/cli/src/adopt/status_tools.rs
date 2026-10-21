use std::collections::BTreeMap;
use std::path::Path;

use dx_adopt::StatusCheck;
use dx_env::DX_DIR_NAME;
use dx_setup::{CommitError, CURRENT_LINK_NAME, ENVIRONMENTS_DIR_NAME, SETUPS_DIR_NAME};

use crate::exec::{ENV_ARTIFACTS_DIR, ENV_VALUES_FILE};

const REFRESH: &str = "dx env --apply";

fn tools(status: &str, detail: String, hint: &str) -> StatusCheck {
    StatusCheck {
        name: "tools".to_owned(),
        status: status.to_owned(),
        detail,
        hint: hint.to_owned(),
    }
}

fn relative(text: &str, workspace: &Path) -> String {
    let prefix = format!("{}{}", workspace.display(), std::path::MAIN_SEPARATOR);
    text.replace(&prefix, "")
}

fn selection_error(error: &CommitError, workspace: &Path) -> StatusCheck {
    tools(
        "error",
        format!("unavailable: {}", relative(&error.to_string(), workspace)),
        &format!("remove {DX_DIR_NAME}/{SETUPS_DIR_NAME}/{CURRENT_LINK_NAME}, then run {REFRESH}"),
    )
}

fn leaf_problem(leaf: &Path, key: &str) -> Option<String> {
    match std::fs::symlink_metadata(leaf) {
        Err(_) => Some(format!("{key} is missing")),
        Ok(meta) if !meta.file_type().is_symlink() => Some(format!("{key} is not a managed link")),
        Ok(_) if std::fs::metadata(leaf).is_err() => {
            Some(format!("{key} points at a missing Bazel artifact"))
        }
        Ok(_) => None,
    }
}

pub(crate) fn managed_tools_check(workspace: &Path) -> StatusCheck {
    let pair = match dx_setup::read_current_pair(workspace) {
        Ok(Some(pair)) => pair,
        Ok(None) => {
            return tools(
                "warn",
                format!(
                    "unknown: no managed environment is selected in {DX_DIR_NAME}/{SETUPS_DIR_NAME}/{CURRENT_LINK_NAME}"
                ),
                REFRESH,
            );
        }
        Err(error) => return selection_error(&error, workspace),
    };
    let id = pair.environment.as_str();
    let short = id.get(..12).unwrap_or(id);
    let dir = workspace
        .join(DX_DIR_NAME)
        .join(ENVIRONMENTS_DIR_NAME)
        .join(id);
    if !dir.is_dir() {
        return tools(
            "error",
            format!("unavailable: selected environment {short} is missing from {DX_DIR_NAME}/{ENVIRONMENTS_DIR_NAME}"),
            REFRESH,
        );
    }
    let values = match std::fs::read_to_string(dir.join(ENV_VALUES_FILE)) {
        Ok(text) => text,
        Err(_) => {
            return tools(
                "error",
                format!("unavailable: environment {short} has no readable {ENV_VALUES_FILE}"),
                REFRESH,
            );
        }
    };
    let keys: BTreeMap<String, String> = match serde_json::from_str(&values) {
        Ok(keys) => keys,
        Err(error) => {
            return tools(
                "error",
                format!(
                    "unavailable: environment {short} has a corrupt {ENV_VALUES_FILE}: {error}"
                ),
                REFRESH,
            );
        }
    };
    if keys.is_empty() {
        return tools(
            "ok",
            format!("not applicable: environment {short} declares no managed tools"),
            "dx env --check",
        );
    }
    let artifacts = dir.join(ENV_ARTIFACTS_DIR);
    let problems: Vec<String> = keys
        .keys()
        .filter_map(|key| leaf_problem(&artifacts.join(key), key))
        .collect();
    if !problems.is_empty() {
        return tools(
            "error",
            format!("unavailable: environment {short}: {}", problems.join("; ")),
            REFRESH,
        );
    }
    let noun = if keys.len() == 1 { "tool" } else { "tools" };
    tools(
        "ok",
        format!(
            "available: {} managed {noun} in environment {short}",
            keys.len()
        ),
        "dx env --check",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENV: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const GEN: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const SETUP: &str = "3b047baa3764c0439825877a47aabce13030571807c14b34ba597ca402021770";

    #[cfg(unix)]
    fn link_dir(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("symlink");
    }

    #[cfg(unix)]
    fn link_file(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("symlink");
    }

    #[cfg(windows)]
    fn link_dir(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_dir(target, link).expect("symlink");
    }

    #[cfg(windows)]
    fn link_file(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_file(target, link).expect("symlink");
    }

    fn select(root: &Path) -> std::path::PathBuf {
        let env = root.join(".dx/environments").join(ENV);
        std::fs::create_dir_all(env.join("artifacts")).expect("env");
        std::fs::create_dir_all(root.join(".dx/generated").join(GEN)).expect("gen");
        let record = root.join(".dx/setups").join(SETUP);
        std::fs::create_dir_all(&record).expect("record");
        link_dir(
            Path::new(&format!("../../environments/{ENV}/")),
            &record.join("environment"),
        );
        link_dir(
            Path::new(&format!("../../generated/{GEN}/")),
            &record.join("generated"),
        );
        link_dir(Path::new(SETUP), &root.join(".dx/setups/current"));
        env
    }

    fn install(env: &Path, root: &Path, key: &str) -> std::path::PathBuf {
        let artifact = root.join(format!("bazel-out-{key}"));
        std::fs::write(&artifact, "#!/bin/sh\n").expect("artifact");
        link_file(&artifact, &env.join("artifacts").join(key));
        artifact
    }

    #[test]
    fn unselected_environment_is_unknown() {
        let scratch = dx_test_scratch::scratch("dx-status-tools-none-");
        let check = managed_tools_check(scratch.path());
        assert_eq!(check.status, "warn");
        assert!(check.detail.starts_with("unknown:"), "{}", check.detail);
        assert_eq!(check.hint, "dx env --apply");
    }

    #[test]
    fn selected_tools_are_available() {
        let scratch = dx_test_scratch::scratch("dx-status-tools-ok-");
        let root = scratch.path();
        let env = select(root);
        install(&env, root, "tool");
        std::fs::write(env.join("values.json"), r#"{"tool":"v"}"#).expect("values");
        let check = managed_tools_check(root);
        assert_eq!(check.status, "ok", "{}", check.detail);
        assert_eq!(
            check.detail,
            "available: 1 managed tool in environment aaaaaaaaaaaa"
        );
        std::fs::write(env.join("values.json"), "{}").expect("values");
        let empty = managed_tools_check(root);
        assert_eq!(empty.status, "ok");
        assert!(
            empty.detail.starts_with("not applicable:"),
            "{}",
            empty.detail
        );
    }

    #[test]
    fn missing_and_corrupt_tools_fail() {
        let scratch = dx_test_scratch::scratch("dx-status-tools-bad-");
        let root = scratch.path();
        let env = select(root);
        let artifact = install(&env, root, "gone");
        std::fs::remove_file(&artifact).expect("rm artifact");
        std::fs::write(env.join("artifacts/plain"), "copied").expect("plain");
        std::fs::write(
            env.join("values.json"),
            r#"{"absent":"v","gone":"v","plain":"v"}"#,
        )
        .expect("values");
        let check = managed_tools_check(root);
        assert_eq!(check.status, "error");
        assert_eq!(
            check.detail,
            "unavailable: environment aaaaaaaaaaaa: absent is missing; gone points at a missing Bazel artifact; plain is not a managed link"
        );
        assert_eq!(check.hint, "dx env --apply");
        std::fs::write(env.join("values.json"), "not json").expect("values");
        let corrupt = managed_tools_check(root);
        assert_eq!(corrupt.status, "error");
        assert!(
            corrupt.detail.contains("corrupt values.json"),
            "{}",
            corrupt.detail
        );
        std::fs::remove_file(env.join("values.json")).expect("rm values");
        assert!(managed_tools_check(root)
            .detail
            .contains("no readable values.json"));
        std::fs::remove_dir_all(&env).expect("rm env");
        let pruned = managed_tools_check(root);
        assert_eq!(pruned.status, "error");
        assert!(
            pruned.detail.contains("is missing from .dx/environments"),
            "{}",
            pruned.detail
        );
    }

    #[test]
    fn foreign_selection_fails_without_absolute_paths() {
        let scratch = dx_test_scratch::scratch("dx-status-tools-foreign-");
        let root = scratch.path();
        std::fs::create_dir_all(root.join(".dx/setups/current")).expect("foreign dir");
        let check = managed_tools_check(root);
        assert_eq!(check.status, "error");
        assert!(check.detail.starts_with("unavailable:"), "{}", check.detail);
        assert!(
            !check.detail.contains(&root.display().to_string()),
            "{}",
            check.detail
        );
        assert_eq!(
            check.hint,
            "remove .dx/setups/current, then run dx env --apply"
        );
    }
}
