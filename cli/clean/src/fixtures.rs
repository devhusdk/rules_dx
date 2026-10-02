//! Test fixtures the clean tests share.

use std::fs;
use std::path::{Path, PathBuf};

use dx_env::DX_DIR_NAME;
use dx_setup::{GenerationId, SetupPair};

use crate::planning::GenerationView;
use crate::records::{validate_record, GenerationKind, SetupRecordView};

pub(crate) fn digest(tag: char) -> String {
    tag.to_string().repeat(64)
}

pub(crate) fn pair_env_gen(env: char, gen: char) -> (String, String, String) {
    let environment = GenerationId::new(&digest(env)).expect("env digest");
    let generated = GenerationId::new(&digest(gen)).expect("gen digest");
    let pair = SetupPair {
        environment,
        generated,
    };
    (dx_setup::setup_hex(&pair), digest(env), digest(gen))
}

pub(crate) fn record(env: char, gen: char) -> SetupRecordView {
    let (hex, environment_hex, generated_hex) = pair_env_gen(env, gen);
    validate_record(&hex, &environment_hex, &generated_hex).expect("valid record")
}

pub(crate) fn generation(kind: GenerationKind, tag: char) -> GenerationView {
    GenerationView {
        kind,
        hex: digest(tag),
    }
}

pub(crate) fn setup_pair(env: char, gen: char) -> SetupPair {
    SetupPair {
        environment: GenerationId::new(&digest(env)).expect("env digest"),
        generated: GenerationId::new(&digest(gen)).expect("gen digest"),
    }
}

pub(crate) fn workspace_of(root: &Path) -> PathBuf {
    root.join("ws")
}

pub(crate) fn two_record_workspace(root: &Path) -> (PathBuf, String, String) {
    let workspace = workspace_of(root);
    let stale = setup_pair('3', '4');
    let current = setup_pair('1', '2');
    dx_setup::commit_pair(&workspace, &stale).expect("commit stale");
    dx_setup::commit_pair(&workspace, &current).expect("commit current");
    let dx_dir = workspace.join(DX_DIR_NAME);
    for (kind, tag) in [
        (GenerationKind::Environment, '1'),
        (GenerationKind::Generated, '2'),
        (GenerationKind::Environment, '3'),
        (GenerationKind::Generated, '4'),
    ] {
        fs::create_dir_all(dx_dir.join(kind.dir_name()).join(digest(tag)))
            .expect("create generation dir");
    }
    let stale_hex = dx_setup::setup_hex(&stale);
    let current_hex = dx_setup::setup_hex(&current);
    (workspace, stale_hex, current_hex)
}

#[cfg(windows)]
pub(crate) fn stage_symlink(target: &Path, link: &Path) {
    std::os::windows::fs::symlink_file(target, link).expect("stage test link");
}

#[cfg(not(windows))]
pub(crate) fn stage_symlink(target: &Path, link: &Path) {
    std::os::unix::fs::symlink(target, link).expect("stage test link");
}
