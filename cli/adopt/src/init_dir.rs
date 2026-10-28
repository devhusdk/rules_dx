//! The directory `dx init <dir>` starts or adopts, and the files a fresh one gets.

use std::path::Path;

use super::AdoptError;
use crate::scaffold::ScaffoldFile;

/// The module name `dx init` uses for the current directory.
pub const DEFAULT_INIT_MODULE: &str = "my_project";

/// The Bazel version a fresh workspace pins in `.bazelversion`; matches this repository's.
pub const FRESH_BAZEL_VERSION: &str = "9.2.0";

const RESERVED_STEMS: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn segment_reason(segment: &str) -> Option<String> {
    if segment.is_empty() || segment == "." || segment == ".." {
        return Some(
            "directories stay inside the current directory (no empty, '.' or '..' segments)"
                .to_owned(),
        );
    }
    if segment.chars().any(char::is_control) {
        return Some("directories never contain control characters".to_owned());
    }
    if segment.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return Some("directories never contain '<', '>', ':', '\"', '|', '?' or '*'".to_owned());
    }
    if segment.ends_with(' ') || segment.ends_with('.') {
        return Some("directory segments never end with a space or '.'".to_owned());
    }
    let stem = segment.split('.').next().unwrap_or(segment);
    if RESERVED_STEMS
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Some(format!(
            "directory segment {segment:?} is a reserved device name"
        ));
    }
    None
}

/// The `/`-separated directory `dx init <dir>` works in, or `""` for the current one.
///
/// The directory stays inside the current directory: no absolute paths, `..`, or `\`.
pub fn validate_init_dir(dir: &str) -> Result<String, AdoptError> {
    if dir.is_empty() || dir == "." {
        return Ok(String::new());
    }
    let refused = |reason: &str| AdoptError::InitInvalidDir {
        dir: dir.to_owned(),
        reason: reason.to_owned(),
    };
    if dir.contains('\\') {
        return Err(refused(
            "directories never contain '\\' (use '/' for nesting)",
        ));
    }
    let bytes = dir.as_bytes();
    if dir.starts_with('/')
        || (bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic())
    {
        return Err(refused(
            "directories stay inside the current directory (no absolute paths)",
        ));
    }
    let dir = dir.strip_prefix("./").unwrap_or(dir);
    let dir = dir.strip_suffix('/').unwrap_or(dir);
    for segment in dir.split('/') {
        if let Some(reason) = segment_reason(segment) {
            return Err(refused(&reason));
        }
    }
    Ok(dir.to_owned())
}

/// The module name for `dir`: its last segment folded to `[a-z0-9._-]`, or
/// [`DEFAULT_INIT_MODULE`] for the current directory.
pub fn init_dir_module(dir: &str) -> String {
    let Some(stem) = dir.rsplit('/').next().filter(|stem| !stem.is_empty()) else {
        return DEFAULT_INIT_MODULE.to_owned();
    };
    let mut folded = String::with_capacity(stem.len());
    let mut gap = true;
    for c in stem.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
            folded.push(c);
            gap = true;
        } else if gap {
            folded.push('_');
            gap = false;
        }
    }
    folded.trim_matches(['-', '_', '.']).to_owned()
}

/// Whether `root` is missing or holds nothing but `.git`: a workspace to start, not adopt.
pub fn is_fresh_dir(root: &Path) -> bool {
    match std::fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .all(|entry| entry.file_name() == ".git"),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    }
}

/// The files only a fresh workspace gets: a `MODULE.bazel` that depends on rules_dx and
/// a `.bazelversion`. Language rules come from rules_dx once sources appear.
pub fn plan_fresh_files(module: &str) -> Vec<ScaffoldFile> {
    vec![
        ScaffoldFile {
            path: "MODULE.bazel".to_owned(),
            content: format!(
                "module(name = \"{module}\")\n\nbazel_dep(name = \"rules_dx\", version = \"{}\")\ninclude(\"{}\")\n",
                super::DX_VERSION,
                crate::overrides::OVERRIDES_INCLUDE
            ),
        },
        ScaffoldFile {
            path: ".bazelversion".to_owned(),
            content: format!("{FRESH_BAZEL_VERSION}\n"),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_directory_spellings_mean_no_subdirectory() {
        for dir in ["", "."] {
            assert_eq!(validate_init_dir(dir).expect(dir), "");
        }
        assert_eq!(init_dir_module(""), DEFAULT_INIT_MODULE);
    }

    #[test]
    fn nested_directories_keep_their_segments() {
        assert_eq!(
            validate_init_dir("teams/demo").expect("nested"),
            "teams/demo"
        );
        assert_eq!(validate_init_dir("./demo/").expect("trimmed"), "demo");
    }

    #[test]
    fn directories_reject_escape_and_hostile_shapes() {
        for hostile in [
            "../evil",
            "a/../../evil",
            "/tmp/absolute",
            "C:/absolute",
            "C:\\absolute",
            "a\\b",
            "a//b",
            "..",
            "a/.",
            "with\nnewline",
            "with\"quote",
            "with*star",
            "with:colon",
            "endswith ",
            "endswith.",
            "nul",
            "teams/COM1.txt",
        ] {
            let error = validate_init_dir(hostile).expect_err(hostile);
            assert!(
                error
                    .to_string()
                    .starts_with("invalid directory for dx init: "),
                "{error}"
            );
        }
    }

    #[test]
    fn module_name_folds_the_last_segment() {
        assert_eq!(init_dir_module("demo"), "demo");
        assert_eq!(init_dir_module("teams/My App"), "my_app");
        assert_eq!(init_dir_module("_hidden_"), "hidden");
        assert_eq!(init_dir_module("+++"), "");
    }

    #[test]
    fn fresh_means_missing_or_only_git() {
        let scratch = dx_test_scratch::scratch("dx-init-fresh-");
        let root = scratch.path().join("ws");
        assert!(is_fresh_dir(&root));
        std::fs::create_dir_all(root.join(".git")).expect("git");
        assert!(is_fresh_dir(&root));
        std::fs::write(root.join("README.md"), "mine\n").expect("readme");
        assert!(!is_fresh_dir(&root));
        scratch.close().expect("cleanup");
    }

    #[test]
    fn fresh_module_depends_on_rules_dx_and_includes_the_overrides() {
        let files = plan_fresh_files("demo");
        let module = &files[0];
        assert_eq!(module.path, "MODULE.bazel");
        let parsed = dx_module_bazel::ModuleFile::parse(&module.content).expect("parses");
        assert!(parsed
            .include_labels()
            .contains(&crate::overrides::OVERRIDES_INCLUDE));
        assert!(module.content.contains(&format!(
            "bazel_dep(name = \"rules_dx\", version = \"{}\")",
            super::super::DX_VERSION
        )));
        assert!(module.content.starts_with("module(name = \"demo\")\n"));
    }

    #[test]
    fn fresh_bazel_version_matches_this_repository() {
        let pinned = crate::version::repo_text(".bazelversion");
        assert_eq!(pinned.trim(), FRESH_BAZEL_VERSION);
        assert_eq!(
            plan_fresh_files("demo")[1].content,
            format!("{FRESH_BAZEL_VERSION}\n")
        );
    }
}
