use std::path::Path;

use dx_adopt::scaffold::ScaffoldFile;
use dx_worktree::setup;

use super::{agents, claude_md};

const ESSENTIAL: [&str; 7] = [
    ".dx/version",
    ".gitignore",
    ".bazelignore",
    "MODULE.bazel",
    "MODULE.bazel.snippet",
    "tools/dx",
    "tools/dx.cmd",
];

/// One file init manages.
pub(super) struct InitFile {
    pub(super) path: String,
    pub(super) essential: bool,
}

/// Every file init manages, once each, starting with the planned scaffold files.
pub(super) fn owned(planned: &[ScaffoldFile]) -> Vec<InitFile> {
    let mut paths: Vec<String> = planned.iter().map(|file| file.path.clone()).collect();
    paths.push(dx_adopt::LAUNCHER_SH_REL.to_owned());
    paths.push(dx_adopt::LAUNCHER_CMD_REL.to_owned());
    paths.extend(
        dx_adopt::render_overrides()
            .into_iter()
            .map(|(path, _)| path),
    );
    paths.extend(dx_adopt::preset_paths());
    paths.extend(setup::HOOK_FILES.iter().map(|file| file.path.to_owned()));
    paths.push(agents::FILE.to_owned());
    paths.push(claude_md::FILE.to_owned());
    let mut files: Vec<InitFile> = Vec::new();
    for path in paths {
        if files.iter().all(|file| file.path != path) {
            let essential = ESSENTIAL.contains(&path.as_str());
            files.push(InitFile { path, essential });
        }
    }
    files
}

/// What a workspace lacks against what init writes.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Drift {
    pub(super) missing: Vec<String>,
    pub(super) stale: Vec<String>,
    pub(super) other: Vec<String>,
}

/// Every owned file that is missing, stale or unreadable under `root`, without writing.
pub(super) fn drift(root: &Path) -> Drift {
    let mut drift = Drift::default();
    for rel in [dx_adopt::LAUNCHER_SH_REL, dx_adopt::LAUNCHER_CMD_REL] {
        if !root.join(rel).exists() {
            drift.missing.push(rel.to_owned());
        }
    }
    drift.stale.extend(dx_adopt::stale_launchers(root));
    let (stale, other) = dx_adopt::stale_owned_files(root);
    drift.stale.extend(stale);
    drift.other.extend(other);
    let preset = dx_adopt::preset_drift(root);
    drift.missing.extend(preset.missing);
    drift.stale.extend(preset.stale);
    drift.other.extend(preset.other);
    for (path, content) in dx_adopt::render_overrides() {
        match std::fs::read(root.join(&path)) {
            Ok(bytes) if bytes == content.as_bytes() => {}
            Ok(_) => drift.stale.push(path),
            Err(_) => drift.missing.push(path),
        }
    }
    if std::fs::read_to_string(root.join(".bazelignore"))
        .is_ok_and(|text| !dx_adopt::bazelignore_covers_dx(&text))
    {
        drift.stale.push(".bazelignore".to_owned());
    }
    for (path, missing) in setup::workspace_missing(root) {
        if root.join(path).exists() {
            drift
                .stale
                .push(format!("{path} (lacks hooks: {})", missing.join("; ")));
        } else {
            drift.missing.push(path.to_owned());
        }
    }
    match agents::planned(root, true) {
        Ok(None) => {}
        Ok(Some(_)) if root.join(agents::FILE).exists() => {
            drift.stale.push(agents::FILE.to_owned());
        }
        Ok(Some(_)) => drift.missing.push(agents::FILE.to_owned()),
        Err(error) => drift.other.push(format!("{}: {error}", agents::FILE)),
    }
    match claude_md::state(root) {
        Ok(claude_md::State::Imported | claude_md::State::Linked) => {}
        Ok(claude_md::State::Missing) => {
            drift
                .stale
                .push(format!("{} (lacks @{})", claude_md::FILE, agents::FILE));
        }
        Ok(claude_md::State::Absent) => drift.missing.push(claude_md::FILE.to_owned()),
        Err(error) => drift.other.push(format!("{}: {error}", claude_md::FILE)),
    }
    drift
}

#[cfg(test)]
mod tests {
    use super::{drift, owned};
    use crate::adopt::test_support::{invocation, run};

    #[test]
    fn everything_init_writes_is_owned_and_present() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-");
        let root = scratch.path();
        let mut planned = dx_adopt::plan_init_files_at(root, "my_project").expect("plan");
        planned.extend(dx_adopt::plan_fresh_files("my_project"));
        let files = owned(&planned);
        let (code, out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        for path in out.lines().filter_map(|line| line.strip_prefix("wrote ")) {
            assert!(files.iter().any(|file| file.path == path), "{path}");
        }
        for file in &files {
            let accounted = root.join(&file.path).exists()
                || out.contains(&file.path)
                || err.contains(&file.path);
            assert!(accounted, "{}", file.path);
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn each_path_is_listed_once() {
        let files = owned(&[]);
        let mut paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
        paths.sort_unstable();
        let count = paths.len();
        paths.dedup();
        assert_eq!(paths.len(), count);
        assert!(
            files
                .iter()
                .filter(|file| file.path == ".gitignore")
                .count()
                <= 1
        );
    }

    #[test]
    fn check_and_apply_write_the_same_snippet_in_a_cuda_tree() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-cuda-");
        let root = scratch.path();
        std::fs::write(root.join("a.cu"), "").expect("write");
        let planned = dx_adopt::plan_init_files_at(root, "my_project").expect("plan");
        let planned_snippet = planned
            .iter()
            .find(|file| file.path == "MODULE.bazel.snippet")
            .expect("snippet")
            .content
            .clone();
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        let written = std::fs::read_to_string(root.join("MODULE.bazel.snippet")).expect("read");
        assert_eq!(written, planned_snippet);
        scratch.close().expect("cleanup");
    }

    #[test]
    fn drift_in_an_empty_dir_is_owned() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-drift-");
        let files = owned(&[]);
        for path in drift(scratch.path()).missing {
            assert!(files.iter().any(|file| file.path == path), "{path}");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn drift_names_each_owned_gap() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-gaps-");
        let root = scratch.path();
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(drift(root), super::Drift::default());
        std::fs::write(root.join(".bazelignore"), "node_modules\n").expect("ignore");
        std::fs::write(
            root.join("tools/dx"),
            "#!/bin/sh\nset -eu\nVERSION=\"1.2.3\"\n",
        )
        .expect("dx");
        std::fs::write(root.join(".dx/version"), "4.5.6\n").expect("pin");
        std::fs::write(root.join("AGENTS.md"), "# Mine\n").expect("agents");
        std::fs::write(root.join("CLAUDE.md"), "# Mine\n").expect("claude");
        let (path, _) = dx_adopt::render_overrides().remove(0);
        std::fs::write(root.join(&path), "changed\n").expect("override");
        let stale = drift(root).stale.join("\n");
        for want in [
            ".bazelignore",
            "tools/dx",
            "AGENTS.md",
            "lacks @AGENTS.md",
            path.as_str(),
        ] {
            assert!(stale.contains(want), "{want}: {stale}");
        }
        scratch.close().expect("cleanup");
    }

    #[test]
    fn old_dx_keys_in_vscode_settings_are_stale() {
        let scratch = dx_test_scratch::scratch("dx-init-owned-vscode-");
        let root = scratch.path();
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        let path = root.join(".vscode/settings.json");
        let text = std::fs::read_to_string(&path).expect("read");
        let (key, _) = text.split_once(':').expect("key");
        std::fs::write(&path, format!("{key}: 1,\n\"x\": 1}}\n")).expect("write");
        assert!(drift(root)
            .stale
            .contains(&".vscode/settings.json".to_owned()));
        let (code, _out, err) = run(&invocation(&["init", "--apply"]), root);
        assert_eq!(code, 0, "{err}");
        assert_eq!(drift(root), super::Drift::default());
        scratch.close().expect("cleanup");
    }
}
