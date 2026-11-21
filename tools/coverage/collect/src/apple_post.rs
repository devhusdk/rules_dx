//! The rules_apple xctestrun post action: passes the test's exit code through and writes LCOV under coverage.

use crate::apple;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The llvm tools the post action runs.
#[derive(Debug, PartialEq, Eq)]
struct Tools {
    llvm_cov: PathBuf,
    llvm_profdata: PathBuf,
}

/// Reads `--llvm-cov=PATH` and `--llvm-profdata=PATH`; both are required.
fn parse_tools(args: &[String]) -> Result<Tools, String> {
    let mut llvm_cov = None;
    let mut llvm_profdata = None;
    for arg in args {
        match arg.split_once('=') {
            Some(("--llvm-cov", path)) => llvm_cov = Some(PathBuf::from(path)),
            Some(("--llvm-profdata", path)) => llvm_profdata = Some(PathBuf::from(path)),
            _ => return Err(format!("unknown argument {arg}")),
        }
    }
    Ok(Tools {
        llvm_cov: llvm_cov.ok_or("--llvm-cov=PATH is required")?,
        llvm_profdata: llvm_profdata.ok_or("--llvm-profdata=PATH is required")?,
    })
}

/// Names each coverage input the post action lacks, the profile first.
fn missing_inputs(profile: &Path, has_profile: bool, objects_env: Option<&str>) -> Vec<String> {
    let mut reasons = Vec::new();
    if !has_profile {
        reasons.push(format!(
            "no {} at {} (rules_apple copies it there when COLLECT_PROFDATA=1) and no .profraw under TEST_TMPDIR",
            apple::PROFDATA_NAME,
            profile.display()
        ));
    }
    match objects_env {
        None => reasons.push("TEST_BINARIES_FOR_LLVM_COV is not set".to_owned()),
        Some(list) if apple::covered_objects(list).is_empty() => {
            reasons.push("TEST_BINARIES_FOR_LLVM_COV lists no objects".to_owned());
        }
        Some(_) => {}
    }
    reasons
}

const CANDIDATE_DEPTH: usize = 8;
const CANDIDATE_LIMIT: usize = 20;

/// Lists `.profraw` and `.profdata` files under `root`, relative to it and sorted, without following symlinks.
fn profile_candidates(root: &Path) -> Vec<PathBuf> {
    let mut found = profile_files(root, &["profraw", "profdata"]);
    found.truncate(CANDIDATE_LIMIT);
    found
}

/// Lists the raw profiles under `root`, where xcodebuild leaves them when it writes no `Coverage.profdata`.
fn raw_profiles(root: &Path) -> Vec<PathBuf> {
    profile_files(root, &["profraw"])
        .into_iter()
        .map(|relative| root.join(relative))
        .collect()
}

/// Lists files under `root` with one of `extensions`, relative to it and sorted, without following symlinks.
fn profile_files(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0_usize)];
    while let Some((dir, depth)) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_dir() {
                if depth + 1 < CANDIDATE_DEPTH {
                    pending.push((path, depth + 1));
                }
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| extensions.iter().any(|wanted| ext == *wanted))
                && let Ok(relative) = path.strip_prefix(root)
            {
                found.push(relative.to_path_buf());
            }
        }
    }
    found.sort();
    found
}

/// Names the llvm-profdata that reads `raw` profiles from Xcode's clang runtime, or the pinned one otherwise.
fn merge_program(tools: &Tools, raw: bool) -> (PathBuf, Vec<&'static str>) {
    // Xcode's runtime writes an older raw profile version than the pinned llvm-profdata reads,
    // so raw profiles go through Xcode's own llvm-profdata.
    if raw {
        (PathBuf::from("xcrun"), vec!["llvm-profdata"])
    } else {
        (tools.llvm_profdata.clone(), Vec::new())
    }
}

// LCOV_EXCL_START - reason: process orchestration over env and the pinned llvm tools, issue: 9981, policy: docs/cli/commands/build-test-coverage.md
fn export(
    tools: &Tools,
    root: &Path,
    profiles: &[PathBuf],
    raw: bool,
    objects: &[PathBuf],
) -> Result<String, String> {
    let (profdata, profdata_args) = merge_program(tools, raw);
    let cov = &tools.llvm_cov;
    let scratch = PathBuf::from(env::var("TEST_TMPDIR").unwrap_or_else(|_| ".".into()));
    let merged = scratch.join("_dx_merged.profdata");
    let out = Command::new(&profdata)
        .args(profdata_args)
        .args(["merge", "--sparse"])
        .args(profiles)
        .arg("--output")
        .arg(&merged)
        .output()
        .map_err(|e| format!("cannot run {}: {e}", profdata.display()))?;
    if !out.status.success() {
        return Err(format!(
            "llvm-profdata merge failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let arch = apple::llvm_arch(env::consts::ARCH);
    let manifest = env::var("COVERAGE_MANIFEST").ok();
    let out = Command::new(cov)
        .args(apple::apple_export_args(
            &merged,
            root,
            objects,
            arch,
            manifest.as_deref(),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("cannot run {}: {e}", cov.display()))?;
    crate::remove_scratch(&merged);
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("no coverage data found") {
            eprintln!("dx apple coverage: llvm-cov found no coverage data");
            return Ok(String::new());
        }
        return Err(format!("llvm-cov export failed:\n{stderr}"));
    }
    let report = String::from_utf8(out.stdout)
        .map_err(|error| format!("llvm-cov wrote non-UTF-8: {error}"))?;
    Ok(apple::rewrite_paths(&report, root))
}

fn collect(tools: &Tools) -> Result<(), String> {
    let output = env::var("COVERAGE_OUTPUT_FILE")
        .map_err(|error| format!("COVERAGE_OUTPUT_FILE is not set: {error}"))?;
    let root = env::current_dir().map_err(|e| format!("cannot read the working directory: {e}"))?;
    let profile = env::var("TEST_UNDECLARED_OUTPUTS_DIR")
        .map(|dir| Path::new(&dir).join(apple::PROFDATA_NAME))
        .map_err(|error| format!("TEST_UNDECLARED_OUTPUTS_DIR is not set: {error}"))?;
    let objects_env = env::var("TEST_BINARIES_FOR_LLVM_COV").ok();
    let objects = apple::covered_objects(objects_env.as_deref().unwrap_or_default());
    let profile_exists = profile.exists();
    let scratch = env::var_os("TEST_TMPDIR").map(PathBuf::from);
    // xcodebuild 16 leaves the raw profile under TEST_TMPDIR/<simulator>/ and writes no
    // Coverage.profdata for rules_apple to copy, so merge the raw profiles instead.
    let profiles = if profile_exists {
        vec![profile.clone()]
    } else {
        scratch.as_deref().map(raw_profiles).unwrap_or_default()
    };
    let missing = missing_inputs(&profile, !profiles.is_empty(), objects_env.as_deref());
    let report = if missing.is_empty() {
        if !profile_exists {
            eprintln!(
                "dx apple coverage: no {} at {}; merging {} .profraw files under TEST_TMPDIR",
                apple::PROFDATA_NAME,
                profile.display(),
                profiles.len()
            );
        }
        let report = export(tools, &root, &profiles, !profile_exists, &objects);
        crate::remove_scratch(&profile);
        report?
    } else {
        eprintln!(
            "dx apple coverage: {}; writing an empty report",
            missing.join("; ")
        );
        if profiles.is_empty()
            && let Some(tmp) = &scratch
        {
            let candidates = profile_candidates(tmp);
            if candidates.is_empty() {
                eprintln!(
                    "dx apple coverage: no .profraw or .profdata under TEST_TMPDIR {}",
                    tmp.display()
                );
            } else {
                let listed: Vec<String> = candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
                eprintln!(
                    "dx apple coverage: profiles under TEST_TMPDIR: {}",
                    listed.join(", ")
                );
            }
        }
        String::new()
    };
    fs::write(&output, report).map_err(|e| format!("cannot write {output}: {e}"))
}

/// Runs the rules_apple xctestrun post action and returns the test's exit code.
pub fn run_process(args: &[String]) -> i32 {
    let tools = match parse_tools(args) {
        Ok(tools) => tools,
        Err(message) => {
            eprintln!("dx apple_post_action: {message}");
            return 2;
        }
    };
    let test_exit = env::var("TEST_EXIT_CODE")
        .ok()
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    if env::var("COVERAGE").as_deref() != Ok("1") {
        return test_exit;
    }
    let result = collect(&tools);
    if let Err(message) = &result {
        eprintln!("dx apple coverage: {message}");
    }
    apple::exit_code(test_exit, result.is_ok())
}
// LCOV_EXCL_STOP - reason: end process orchestration, issue: 9981, policy: docs/cli/commands/build-test-coverage.md

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn both_flags_give_the_tools() {
        assert_eq!(
            parse_tools(&args(&["--llvm-profdata=/p", "--llvm-cov=/c"])),
            Ok(Tools {
                llvm_cov: PathBuf::from("/c"),
                llvm_profdata: PathBuf::from("/p"),
            })
        );
    }

    #[test]
    fn a_missing_flag_is_an_error() {
        assert_eq!(
            parse_tools(&args(&["--llvm-cov=/c"])),
            Err("--llvm-profdata=PATH is required".to_owned())
        );
        assert_eq!(
            parse_tools(&args(&["--llvm-profdata=/p"])),
            Err("--llvm-cov=PATH is required".to_owned())
        );
    }

    #[test]
    fn an_unknown_argument_is_an_error() {
        assert_eq!(
            parse_tools(&args(&["--llvm-cov=/c", "--llvm-profdata=/p", "--x"])),
            Err("unknown argument --x".to_owned())
        );
    }

    #[test]
    fn raw_profiles_merge_with_xcode_llvm_profdata() {
        let tools = Tools {
            llvm_cov: PathBuf::from("/c"),
            llvm_profdata: PathBuf::from("/p"),
        };
        assert_eq!(
            merge_program(&tools, true),
            (PathBuf::from("xcrun"), vec!["llvm-profdata"])
        );
        assert_eq!(
            merge_program(&tools, false),
            (PathBuf::from("/p"), Vec::new())
        );
    }

    #[test]
    fn present_inputs_give_no_reasons() {
        assert!(missing_inputs(Path::new("/o/Coverage.profdata"), true, Some("a/app")).is_empty());
    }

    #[test]
    fn a_missing_profile_names_its_path() {
        let reasons = missing_inputs(Path::new("/o/Coverage.profdata"), false, Some("a/app"));
        assert_eq!(reasons.len(), 1, "{reasons:?}");
        assert!(
            reasons
                .iter()
                .all(|reason| reason.contains("/o/Coverage.profdata")),
            "{reasons:?}"
        );
    }

    #[test]
    fn an_unset_object_list_is_named() {
        assert_eq!(
            missing_inputs(Path::new("/o/Coverage.profdata"), true, None),
            vec!["TEST_BINARIES_FOR_LLVM_COV is not set".to_owned()]
        );
    }

    #[test]
    fn an_empty_object_list_is_named() {
        for list in ["", ";"] {
            assert_eq!(
                missing_inputs(Path::new("/o/Coverage.profdata"), true, Some(list)),
                vec!["TEST_BINARIES_FOR_LLVM_COV lists no objects".to_owned()]
            );
        }
    }

    #[test]
    fn both_missing_inputs_are_named_profile_first() {
        let reasons = missing_inputs(Path::new("/o/Coverage.profdata"), false, None);
        assert_eq!(reasons.len(), 2, "{reasons:?}");
        assert!(
            reasons
                .first()
                .is_some_and(|r| r.starts_with("no Coverage.profdata at /o/"))
        );
        assert_eq!(
            reasons.get(1).map(String::as_str),
            Some("TEST_BINARIES_FOR_LLVM_COV is not set")
        );
    }

    #[test]
    fn profile_candidates_lists_profiles_within_the_depth() {
        let base = env::var_os("TEST_TMPDIR").map_or_else(env::temp_dir, PathBuf::from);
        let root = base.join("apple_post_profile_candidates");
        let _ = fs::remove_dir_all(&root);
        let profdata = root.join("t/derived_data/x/Coverage.profdata");
        let deep = root.join("1/2/3/4/5/6/7/8/9.profraw");
        for file in [
            &profdata,
            &root.join("a.profraw"),
            &root.join("b.txt"),
            &deep,
        ] {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "").unwrap();
        }
        assert_eq!(
            profile_candidates(&root),
            vec![
                PathBuf::from("a.profraw"),
                PathBuf::from("t/derived_data/x/Coverage.profdata"),
            ]
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn raw_profiles_gives_absolute_profraw_paths_only() {
        let base = env::var_os("TEST_TMPDIR").map_or_else(env::temp_dir, PathBuf::from);
        let root = base.join("apple_post_raw_profiles");
        let _ = fs::remove_dir_all(&root);
        let raw = root.join("test_tmp_dir.x/SIM-ID/A-1.profraw");
        for file in [&raw, &root.join("t/Coverage.profdata"), &root.join("b.txt")] {
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "").unwrap();
        }
        assert_eq!(raw_profiles(&root), vec![raw]);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_missing_profile_mentions_the_raw_profiles() {
        let reasons = missing_inputs(Path::new("/o/Coverage.profdata"), false, Some("a/app"));
        assert!(
            reasons
                .iter()
                .all(|reason| reason.ends_with("no .profraw under TEST_TMPDIR")),
            "{reasons:?}"
        );
    }

    #[test]
    fn profile_candidates_of_a_missing_root_is_empty() {
        let base = env::var_os("TEST_TMPDIR").map_or_else(env::temp_dir, PathBuf::from);
        assert!(profile_candidates(&base.join("apple_post_no_such_root")).is_empty());
    }
}
