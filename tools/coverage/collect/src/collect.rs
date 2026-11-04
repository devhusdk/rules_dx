use std::path::{Path, PathBuf};

const RUNTIME_OBJECTS_SUFFIX: &str = "runtime_objects_list.txt";

pub fn runtime_object_lists(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(RUNTIME_OBJECTS_SUFFIX))
        .map(str::to_string)
        .collect()
}

pub fn object_lines(list: &str) -> Vec<String> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn resolve(root: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

/// Where a report the test itself wrote to COVERAGE_OUTPUT_FILE is kept so the merger still sees it.
pub fn preserved_report(output: &Path, coverage_dir: &Path) -> Option<PathBuf> {
    if output.starts_with(coverage_dir) {
        return None;
    }
    Some(coverage_dir.join("test_report.dat"))
}

pub fn export_args(profdata: &Path, root: &Path, objects: &[PathBuf]) -> Vec<String> {
    let mut args = vec![
        "export".to_string(),
        "-format=lcov".to_string(),
        "-instr-profile".to_string(),
        profdata.display().to_string(),
        "-ignore-filename-regex=.*external/.+".to_string(),
        format!("-path-equivalence=.,{}", root.display()),
    ];
    for (i, object) in objects.iter().enumerate() {
        if i > 0 {
            args.push("-object".to_string());
        }
        args.push(object.display().to_string());
    }
    args
}

pub fn rewrite_paths(report: &str, root: &Path) -> String {
    let root = root.display().to_string();
    report
        .replace("/proc/self/cwd/", "")
        .replace(&format!("{root}/"), "")
        .replace(&root, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_runtime_object_lists_from_manifest() {
        let manifest = "cc/tests/fixtures/c-app/cli/cli.c\nbazel-out/k8/bin/cc/tests/fixtures/c-app/cli/cli_upstreamruntime_objects_list.txt\n\nbazel-out/k8/bin/x.gcno\n";
        assert_eq!(
            runtime_object_lists(manifest),
            vec![
                "bazel-out/k8/bin/cc/tests/fixtures/c-app/cli/cli_upstreamruntime_objects_list.txt"
            ]
        );
    }

    #[test]
    fn reads_object_lines() {
        assert_eq!(
            object_lines("bazel-out/k8/bin/a\n\n  bazel-out/k8/bin/b \n"),
            vec!["bazel-out/k8/bin/a", "bazel-out/k8/bin/b"]
        );
    }

    #[test]
    fn keeps_a_report_written_outside_the_coverage_dir() {
        let dir = Path::new("/exec/cov/test");
        assert_eq!(
            preserved_report(Path::new("/exec/testlogs/x/coverage.dat"), dir),
            Some(PathBuf::from("/exec/cov/test/test_report.dat"))
        );
        assert_eq!(
            preserved_report(Path::new("/exec/cov/test/coverage.dat"), dir),
            None
        );
    }

    #[test]
    fn resolves_relative_paths_against_root() {
        let root = Path::new("/exec/root");
        assert_eq!(
            resolve(root, "bazel-out/a"),
            PathBuf::from("/exec/root/bazel-out/a")
        );
        assert_eq!(resolve(root, "/abs/b"), PathBuf::from("/abs/b"));
    }

    #[test]
    fn passes_first_object_positionally() {
        let args = export_args(
            Path::new("p.profdata"),
            Path::new("/r"),
            &[PathBuf::from("a"), PathBuf::from("b"), PathBuf::from("c")],
        );
        assert_eq!(
            args[args.len() - 5..],
            ["a", "-object", "b", "-object", "c"].map(String::from)
        );
        assert!(args.contains(&"-path-equivalence=.,/r".to_string()));
        assert_eq!(
            args[..4],
            ["export", "-format=lcov", "-instr-profile", "p.profdata"].map(String::from)
        );
    }

    #[test]
    fn keeps_sources_under_a_tmp_workspace_root() {
        let args = export_args(
            Path::new("p.profdata"),
            Path::new("/tmp/ws"),
            &[PathBuf::from("a")],
        );
        let ignores: Vec<&String> = args
            .iter()
            .filter(|arg| arg.starts_with("-ignore-filename-regex="))
            .collect();
        assert_eq!(ignores, vec!["-ignore-filename-regex=.*external/.+"]);
        assert!(args.contains(&"-path-equivalence=.,/tmp/ws".to_string()));
    }

    #[test]
    fn strips_cwd_and_execroot_prefixes() {
        let report = "SF:/proc/self/cwd/a.c\nSF:/exec/root/b.rs\nSF:c.cc\n";
        assert_eq!(
            rewrite_paths(report, Path::new("/exec/root")),
            "SF:a.c\nSF:b.rs\nSF:c.cc\n"
        );
    }
}
