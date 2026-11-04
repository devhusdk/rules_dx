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

#[cfg(test)]
mod tests {
    use super::owned;
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
        for path in crate::adopt::wiring::drift(scratch.path()).missing {
            assert!(files.iter().any(|file| file.path == path), "{path}");
        }
        scratch.close().expect("cleanup");
    }
}
