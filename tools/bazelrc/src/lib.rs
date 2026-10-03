#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const PRESET_BAZEL_VERSION: &str = "9.2.0";

pub const PRESET_DX_VERSION: &str = "0.0.0";

const UPSTREAM_FLAGS: [&str; 3] = [
    "common --enable_bzlmod",
    "build --verbose_failures",
    "test --test_output=errors",
];

const COVERAGE_FLAGS: [&str; 8] = [
    "common --enable_platform_specific_config",
    "coverage --test_env=GENERATE_LLVM_LCOV=1",
    "coverage --combined_report=lcov",
    "coverage --test_tag_filters=-no-coverage",
    "coverage --enable_runfiles",
    "coverage:linux --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
    "coverage:macos --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov",
    "coverage --instrumentation_filter=^//",
];

const BUILD_PROFILES: [&str; 5] = [
    "build:dx_debug --compilation_mode=dbg",
    "build:dx_dev --compilation_mode=fastbuild",
    "build:dx_release --compilation_mode=opt",
    "build:dx_dev_remote --compilation_mode=fastbuild",
    "build:dx_toolchain --compilation_mode=fastbuild",
];

const RUNFILES_FLAGS: [&str; 2] = [
    "common --nolegacy_external_runfiles",
    "build:windows --nobuild_runfile_links",
];

const WINDOWS_FLAGS: [&str; 1] = ["build:windows --enable_runfiles"];

pub fn render_fragment() -> String {
    let mut lines = vec![
        "# Vendored Bazel execution preset -- GENERATED, do not edit.".to_owned(),
        "# Regenerate: `bazel run //tools/bazelrc:preset_update`.".to_owned(),
    ];
    lines.extend(UPSTREAM_FLAGS.iter().map(|flag| (*flag).to_owned()));
    lines.extend(COVERAGE_FLAGS.iter().map(|flag| (*flag).to_owned()));
    lines.extend(BUILD_PROFILES.iter().map(|flag| (*flag).to_owned()));
    lines.extend(RUNFILES_FLAGS.iter().map(|flag| (*flag).to_owned()));
    lines.extend(WINDOWS_FLAGS.iter().map(|flag| (*flag).to_owned()));
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

pub fn rendered_flag_lines(rendered: &str) -> BTreeSet<String> {
    rendered
        .lines()
        .filter_map(|line| {
            if line.is_empty() || line.starts_with('#') {
                None
            } else {
                Some(line.to_owned())
            }
        })
        .collect()
}

pub fn owned_collisions_in_content(root_content: &str, rendered: &str) -> Vec<String> {
    let owned = rendered_flag_lines(rendered);
    let mut collisions = Vec::new();
    for raw in root_content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with("import ") || line.starts_with("try-import ") {
            continue;
        }
        if owned.contains(line) {
            collisions.push(line.to_owned());
        }
    }
    collisions.sort();
    collisions.dedup();
    collisions
}

pub fn preset_paths(workspace: &Path) -> (PathBuf, PathBuf) {
    (
        workspace.join(".bazelrc"),
        workspace.join("tools/bazelrc/preset.bazelrc"),
    )
}

pub fn source_dir(workspace: &Path) -> PathBuf {
    workspace.join("tools/bazelrc")
}

pub fn bin_usage(err: &mut dyn std::io::Write) -> i32 {
    let _ = writeln!(err, "usage: preset.update [--verify-only]");
    1
}

pub fn bin_cannot(err: &mut dyn std::io::Write, error: impl std::fmt::Display) -> i32 {
    let _ = writeln!(err, "{error}");
    1
}

pub fn resolve_workspace() -> std::io::Result<PathBuf> {
    if let Ok(workspace) = std::env::var("BUILD_WORKSPACE_DIRECTORY") {
        if !workspace.is_empty() {
            return Ok(PathBuf::from(workspace));
        }
    }
    std::env::current_dir()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresetError {
    OwnedCollision { lines: String },
    Stale { detail: String },
    Unwritable { path: String, detail: String },
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OwnedCollision { lines } => write!(
                formatter,
                "preset.update: root .bazelrc duplicates preset lines; reconcile (remove owned duplicates, keep project overrides explicit): {lines}"
            ),
            Self::Stale { detail } => write!(formatter, "{detail}"),
            Self::Unwritable { path, detail } => {
                write!(formatter, "preset.update: cannot write {path}: {detail}")
            }
        }
    }
}

impl std::error::Error for PresetError {}

pub fn unified_diff(checked_in: &str, regenerated: &str) -> String {
    let old: Vec<&str> = checked_in.lines().collect();
    let new: Vec<&str> = regenerated.lines().collect();
    let mut out = vec!["--- checked-in".to_owned(), "+++ regenerated".to_owned()];
    let max = old.len().max(new.len());
    for i in 0..max {
        let old_line = old.get(i).copied().unwrap_or("");
        let new_line = new.get(i).copied().unwrap_or("");
        if old_line != new_line {
            if i < old.len() {
                out.push(format!("-{old_line}"));
            }
            if i < new.len() {
                out.push(format!("+{new_line}"));
            }
        }
    }
    out.join("\n")
}

fn read_collisions(root_path: &Path, rendered: &str) -> std::io::Result<Vec<String>> {
    match std::fs::read_to_string(root_path) {
        Ok(content) => Ok(owned_collisions_in_content(&content, rendered)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn collision_error(collisions: &[String]) -> PresetError {
    PresetError::OwnedCollision {
        lines: collisions.join(", "),
    }
}

pub fn check_preset(workspace: &Path) -> Result<(), PresetError> {
    let source = source_dir(workspace);
    if !source.is_dir() {
        return Err(PresetError::Unwritable {
            path: source.display().to_string(),
            detail: "source directory not found".to_owned(),
        });
    }
    let rendered = render_fragment();
    let (root_path, fragment_path) = preset_paths(workspace);
    let collisions =
        read_collisions(&root_path, &rendered).map_err(|error| PresetError::Unwritable {
            path: ".bazelrc".to_owned(),
            detail: error.to_string(),
        })?;
    if !collisions.is_empty() {
        return Err(collision_error(&collisions));
    }
    let checked_in = std::fs::read_to_string(&fragment_path).map_err(|_| {
        let diff = unified_diff("", &rendered);
        PresetError::Stale {
            detail: format!(
                "preset.update: preset.bazelrc is stale; run the regen command\n{diff}"
            ),
        }
    })?;
    if checked_in == rendered {
        Ok(())
    } else {
        let diff = unified_diff(&checked_in, &rendered);
        Err(PresetError::Stale {
            detail: format!(
                "preset.update: preset.bazelrc is stale; run the regen command\n{diff}"
            ),
        })
    }
}

pub fn update_preset(workspace: &Path) -> Result<(), PresetError> {
    let source = source_dir(workspace);
    if !source.is_dir() {
        return Err(PresetError::Unwritable {
            path: source.display().to_string(),
            detail: "source directory not found".to_owned(),
        });
    }
    let rendered = render_fragment();
    let (root_path, fragment_path) = preset_paths(workspace);
    let collisions =
        read_collisions(&root_path, &rendered).map_err(|error| PresetError::Unwritable {
            path: ".bazelrc".to_owned(),
            detail: error.to_string(),
        })?;
    if !collisions.is_empty() {
        return Err(collision_error(&collisions));
    }
    std::fs::write(&fragment_path, rendered.as_bytes()).map_err(|error| {
        PresetError::Unwritable {
            path: "tools/bazelrc/preset.bazelrc".to_owned(),
            detail: error.to_string(),
        }
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_track_canonical_versions() {
        assert_eq!(PRESET_BAZEL_VERSION, "9.2.0");
        assert_eq!(PRESET_DX_VERSION, "0.0.0");
        assert_eq!(UPSTREAM_FLAGS.len(), 3);
        assert_eq!(COVERAGE_FLAGS.len(), 8);
        assert_eq!(BUILD_PROFILES.len(), 5);
        assert_eq!(WINDOWS_FLAGS.len(), 1);
        assert_eq!(RUNFILES_FLAGS.len(), 2);
    }

    #[test]
    fn fragment_bytes_match_retired_python() {
        let rendered = render_fragment();
        let expected = "# Vendored Bazel execution preset -- GENERATED, do not edit.\n# Regenerate: `bazel run //tools/bazelrc:preset_update`.\ncommon --enable_bzlmod\nbuild --verbose_failures\ntest --test_output=errors\ncommon --enable_platform_specific_config\ncoverage --test_env=GENERATE_LLVM_LCOV=1\ncoverage --combined_report=lcov\ncoverage --test_tag_filters=-no-coverage\ncoverage --enable_runfiles\ncoverage:linux --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov\ncoverage:macos --test_env=COVERAGE_GCOV_PATH=/usr/bin/gcov\ncoverage --instrumentation_filter=^//\nbuild:dx_debug --compilation_mode=dbg\nbuild:dx_dev --compilation_mode=fastbuild\nbuild:dx_release --compilation_mode=opt\nbuild:dx_dev_remote --compilation_mode=fastbuild\nbuild:dx_toolchain --compilation_mode=fastbuild\ncommon --nolegacy_external_runfiles\nbuild:windows --nobuild_runfile_links\nbuild:windows --enable_runfiles\n";
        assert_eq!(rendered, expected);
        assert!(rendered.ends_with('\n'));
        assert!(!rendered.ends_with("\n\n"));
        for flag in UPSTREAM_FLAGS
            .iter()
            .chain(COVERAGE_FLAGS.iter())
            .chain(BUILD_PROFILES.iter())
            .chain(RUNFILES_FLAGS.iter())
            .chain(WINDOWS_FLAGS.iter())
        {
            assert!(rendered.contains(flag), "missing {flag}");
        }
    }

    #[test]
    fn flag_lines_skip_comments_and_blanks() {
        let rendered = render_fragment();
        let flags = rendered_flag_lines(&rendered);
        assert!(flags.contains("common --enable_bzlmod"));
        assert!(flags.contains("build:dx_dev --compilation_mode=fastbuild"));
        assert!(flags.contains("build:dx_dev_remote --compilation_mode=fastbuild"));
        assert!(flags.contains("build:dx_toolchain --compilation_mode=fastbuild"));
        assert!(flags.contains("common --nolegacy_external_runfiles"));
        assert!(flags.contains("build:windows --nobuild_runfile_links"));
        assert_eq!(flags.len(), 19);
        assert!(!flags.iter().any(|line| line.starts_with('#')));
        assert!(!flags.iter().any(String::is_empty));
    }

    #[test]
    fn collisions_ignore_imports_comments_and_blanks() {
        let rendered = render_fragment();
        let root = "# comment\n\nimport %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\nbuild --verbose_failures\n";
        let collisions = owned_collisions_in_content(root, &rendered);
        assert_eq!(collisions, vec!["build --verbose_failures".to_owned()]);
        let clean = "import %workspace%/tools/bazelrc/preset.bazelrc\nbuild --@rules_rust//rust/settings:extra_rustc_flags=--deny=warnings\n";
        assert!(owned_collisions_in_content(clean, &rendered).is_empty());
        let blank = "# only\n\n";
        assert!(owned_collisions_in_content(blank, &rendered).is_empty());
    }

    #[test]
    fn collisions_are_sorted_and_deduped() {
        let rendered = render_fragment();
        let root =
            "test --test_output=errors\nbuild --verbose_failures\ntest --test_output=errors\n";
        let collisions = owned_collisions_in_content(root, &rendered);
        assert_eq!(
            collisions,
            vec![
                "build --verbose_failures".to_owned(),
                "test --test_output=errors".to_owned()
            ]
        );
    }

    #[test]
    fn paths_join_workspace() {
        let workspace = Path::new("/tmp/ws");
        let (root, fragment) = preset_paths(workspace);
        assert_eq!(root, Path::new("/tmp/ws/.bazelrc"));
        assert_eq!(fragment, Path::new("/tmp/ws/tools/bazelrc/preset.bazelrc"));
        assert_eq!(source_dir(workspace), Path::new("/tmp/ws/tools/bazelrc"));
    }

    #[test]
    fn diff_marks_changed_lines() {
        let diff = unified_diff("a\nb\n", "a\nc\n");
        assert!(diff.contains("--- checked-in"));
        assert!(diff.contains("+++ regenerated"));
        assert!(diff.contains("-b"));
        assert!(diff.contains("+c"));
        let clean = unified_diff("same\n", "same\n");
        assert!(clean.contains("--- checked-in"));
        assert!(!clean.contains("-same"));
    }

    #[test]
    fn error_display_matches_python_messages() {
        let collision = PresetError::OwnedCollision {
            lines: "build --verbose_failures".to_owned(),
        };
        assert!(collision.to_string().contains("duplicates preset lines"));
        let stale = PresetError::Stale {
            detail: "preset.update: preset.bazelrc is stale; run the regen command".to_owned(),
        };
        assert!(stale.to_string().contains("is stale"));
        let unwritable = PresetError::Unwritable {
            path: "tools/bazelrc/preset.bazelrc".to_owned(),
            detail: "denied".to_owned(),
        };
        assert!(unwritable.to_string().contains("cannot write"));
    }

    #[test]
    fn check_and_update_round_trip_in_scratch() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(source_dir(root)).expect("source dir");
        std::fs::write(
            root.join(".bazelrc"),
            "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
        )
        .expect("bazelrc");
        let (_, fragment) = preset_paths(root);
        assert!(matches!(check_preset(root), Err(PresetError::Stale { .. })));
        update_preset(root).expect("update creates fragment");
        assert_eq!(check_preset(root), Ok(()));
        assert_eq!(
            std::fs::read_to_string(&fragment).expect("read"),
            render_fragment()
        );
        std::fs::write(&fragment, "# dirty\n").expect("dirty");
        match check_preset(root) {
            Err(PresetError::Stale { detail }) => {
                assert!(detail.contains("is stale"));
                assert!(detail.contains("checked-in"));
            }
            other => panic!("want Stale, got {other:?}"),
        }
        update_preset(root).expect("update fixes dirty");
        assert_eq!(check_preset(root), Ok(()));
    }

    #[test]
    fn missing_source_dir_fails_closed() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        assert!(matches!(
            check_preset(root),
            Err(PresetError::Unwritable { .. })
        ));
        assert!(matches!(
            update_preset(root),
            Err(PresetError::Unwritable { .. })
        ));
    }

    #[test]
    fn duplicates_fail_closed_on_both_paths() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(source_dir(root)).expect("source dir");
        std::fs::write(
            root.join(".bazelrc"),
            "import %workspace%/tools/bazelrc/preset.bazelrc\ntry-import %workspace%/user.bazelrc\n",
        )
        .expect("clean");
        update_preset(root).expect("update creates fragment");
        std::fs::write(
            root.join(".bazelrc"),
            "import %workspace%/tools/bazelrc/preset.bazelrc\ncommon --enable_bzlmod\n",
        )
        .expect("collision");
        assert!(matches!(
            check_preset(root),
            Err(PresetError::OwnedCollision { .. })
        ));
        assert!(matches!(
            update_preset(root),
            Err(PresetError::OwnedCollision { .. })
        ));
    }

    #[test]
    fn missing_root_bazelrc_means_no_collisions() {
        let scratch = tempfile::TempDir::new().expect("scratch");
        let root = scratch.path();
        std::fs::create_dir_all(source_dir(root)).expect("source dir");
        update_preset(root).expect("update without root");
        assert_eq!(check_preset(root), Ok(()));
    }
}
