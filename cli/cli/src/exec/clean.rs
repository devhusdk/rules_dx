use super::common::*;
use dx_atomic_fs::commit::{DEFAULT_COMMIT_LOCK_TIMEOUT, ManagedRoot, acquire_commit_lock};
use dx_atomic_fs::lease;
use dx_atomic_fs::managed::{EntryKind, classify, remove_managed};
use dx_error::DxError;
use dx_output::{FinishedCounts, NoticeEvent, command_finished, notice_event};
use dx_path::state::{CLEAN_ENTRIES, LEASES, LOCAL_DIR, local_dir};
use dx_plan::args::Invocation;
use std::fs;
use std::io;
use std::path::Path;

struct Removed {
    path: String,
    bytes: u64,
}

pub(crate) fn execute_clean(invocation: &Invocation, env: Env<'_>) -> i32 {
    let Env {
        workspace,
        runner,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == dx_output::OutputMode::Json;
    if json && let Err(exit) = emit_started(invocation, out) {
        return exit;
    }
    let removed = match remove_local_state(workspace) {
        Ok(removed) => removed,
        Err(error) => {
            return report_dx_error(invocation.output, out, err, CODE_CLEAN_FAILED, &error);
        }
    };
    let total: u64 = removed.iter().map(|entry| entry.bytes).sum();
    if json {
        for entry in &removed {
            let notice = NoticeEvent {
                related_command: Some("clean".to_owned()),
                path: Some(entry.path.clone()),
                ..NoticeEvent::info(
                    CODE_CLEAN_REMOVED,
                    format!("removed {} ({} bytes)", entry.path, entry.bytes),
                )
            };
            if let Err(exit) = emit_built(out, notice_event(&notice)) {
                return exit;
            }
        }
        let summary = format!("removed {} paths ({total} bytes)", removed.len());
        let notice = NoticeEvent {
            related_command: Some("clean".to_owned()),
            ..NoticeEvent::info(CODE_CLEAN_REMOVED, summary)
        };
        if let Err(exit) = emit_built(out, notice_event(&notice)) {
            return exit;
        }
    } else if invocation.chatty() {
        for entry in &removed {
            let line = writeln!(out, "removed {} ({} bytes)", entry.path, entry.bytes);
            if let Err(exit) = check_stdout_write(line) {
                return exit;
            }
        }
        let line = writeln!(
            out,
            "dx clean: removed {} paths ({total} bytes)",
            removed.len()
        );
        if let Err(exit) = check_stdout_write(line) {
            return exit;
        }
    }
    if invocation.clean_bazel || invocation.expunge {
        #[allow(clippy::disallowed_methods)] // clean takes no build settings
        let mut argv = dx_process::startup_argv(&invocation.bazel_startup_options);
        argv.push("clean".to_owned());
        if invocation.expunge {
            argv.push("--expunge".to_owned());
        }
        match run_bazel_output(invocation.output, out, err, workspace, runner, &argv, &[]) {
            Ok(0) => {}
            Ok(code) => {
                let message = format!("`bazel clean` exited with {code}");
                return operational(invocation, out, err, CODE_BAZEL_FAILED, &message);
            }
            Err(exit) => return exit,
        }
    }
    if json {
        return match emit_event(out, &command_finished(0, &FinishedCounts::default())) {
            Ok(()) => 0,
            Err(exit) => exit,
        };
    }
    if invocation.chatty()
        && let Err(exit) = check_stdout_write(writeln!(out, "next: dx sync"))
    {
        return exit;
    }
    0
}

fn remove_local_state(workspace: &Path) -> Result<Vec<Removed>, DxError> {
    let local = local_dir(workspace);
    if !local.is_dir() {
        return Ok(Vec::new());
    }
    let _lock = acquire_commit_lock(
        &ManagedRoot::new(local.clone()),
        DEFAULT_COMMIT_LOCK_TIMEOUT,
    )
    .map_err(|error| DxError::failure(error.to_string()))?;
    let mut removed = Vec::new();
    for entry in CLEAN_ENTRIES {
        let path = local.join(entry);
        let display = format!("{LOCAL_DIR}/{entry}");
        let result = if entry == LEASES {
            remove_dead_leases(&path, &display, &mut removed)
        } else {
            remove_entry(&path, &display, &mut removed)
        };
        result.map_err(|error| {
            DxError::from_io(format!("cannot remove {display}: {error}"), &error)
        })?;
    }
    Ok(removed)
}

fn remove_entry(path: &Path, display: &str, removed: &mut Vec<Removed>) -> io::Result<()> {
    let kind = classify(path)?;
    let bytes = match kind {
        EntryKind::Missing => return Ok(()),
        EntryKind::Directory => tree_bytes(path),
        EntryKind::File | EntryKind::Link => fs::symlink_metadata(path)?.len(),
    };
    match kind {
        EntryKind::Directory => fs::remove_dir_all(path)?,
        _ => remove_managed(path)?,
    }
    removed.push(Removed {
        path: display.to_owned(),
        bytes,
    });
    Ok(())
}

fn remove_dead_leases(root: &Path, display: &str, removed: &mut Vec<Removed>) -> io::Result<()> {
    let Ok(kinds) = fs::read_dir(root) else {
        return Ok(());
    };
    for kind in kinds {
        let kind = kind?;
        if !kind.file_type()?.is_dir() {
            continue;
        }
        for file in fs::read_dir(kind.path())? {
            let file = file?;
            let path = file.path();
            let Some(held) = lease::try_lock_file(&path, false)? else {
                continue;
            };
            drop(held);
            let shown = format!(
                "{display}/{}/{}",
                kind.file_name().to_string_lossy(),
                file.file_name().to_string_lossy()
            );
            remove_entry(&path, &shown, removed)?;
        }
        let _ = fs::remove_dir(kind.path());
    }
    let _ = fs::remove_dir(root);
    Ok(())
}

fn tree_bytes(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => tree_bytes(&entry.path()),
            _ => entry.metadata().map_or(0, |meta| meta.len()),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_atomic_fs::commit::{ManagedRoot, acquire_commit_lock};
    use std::time::Duration;

    fn local(harness: &Harness, name: &str) -> std::path::PathBuf {
        harness.workspace.join(".dx/local").join(name)
    }

    fn fill(harness: &Harness, name: &str) {
        let dir = local(harness, name);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("data"), "x").expect("data");
    }

    #[test]
    fn clean_removes_rebuildable_entries_and_keeps_user_data() {
        let harness = Harness::new("clean-reset");
        commit_clean_pair(&harness, '1', '2');
        for name in ["bin", "bin.next", "bin.prev", "ide", "docs", "profile"] {
            fill(&harness, name);
        }
        for name in ["fuzz", "bench", "advisory", "launcher"] {
            fill(&harness, name);
        }
        std::fs::write(local(&harness, "results.json"), "{}").expect("results");
        std::fs::write(local(&harness, "fingerprint"), "f").expect("fingerprint");
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        for name in [
            "setups",
            "environments",
            "generated",
            "bin",
            "bin.next",
            "bin.prev",
            "ide",
            "docs",
            "profile",
            "results.json",
            "fingerprint",
        ] {
            assert!(
                std::fs::symlink_metadata(local(&harness, name)).is_err(),
                "{name} survived"
            );
        }
        for name in ["fuzz", "bench", "advisory", "launcher"] {
            assert!(local(&harness, name).join("data").exists(), "{name} lost");
        }
        assert!(out.contains("next: dx sync"), "{out}");
        assert!(out.contains("removed .dx/local/bin ("), "{out}");
        assert!(harness.build_argv.borrow().is_empty(), "no Bazel launch");
    }

    #[test]
    fn clean_without_local_state_succeeds() {
        let harness = Harness::new("clean-empty");
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(!local(&harness, "").exists());
    }

    #[test]
    fn clean_leaves_the_files_outside_dx_local() {
        let harness = Harness::new("clean-outside");
        fill(&harness, "bin");
        let committed = harness.workspace.join(".dx/advisory.toml");
        std::fs::write(&committed, "keep").expect("committed file");
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(committed.exists());
    }

    #[test]
    fn clean_keeps_a_lease_a_live_holder_has() {
        let harness = Harness::new("clean-leases");
        let root = local(&harness, "");
        let held = "a".repeat(64);
        let dead = "b".repeat(64);
        let _lease = dx_atomic_fs::lease::acquire_shared(
            &root,
            dx_atomic_fs::lease::GenerationUse::Environment,
            &held,
            Duration::ZERO,
        )
        .expect("live lease");
        drop(
            dx_atomic_fs::lease::try_acquire_exclusive(
                &root,
                dx_atomic_fs::lease::GenerationUse::Environment,
                &dead,
            )
            .expect("dead lease"),
        );
        let (code, out, err) = harness.run(&["clean"]);
        assert_eq!(code, 0, "{out}{err}");
        let dir = local(&harness, "leases/environments");
        assert!(dir.join(format!("{held}.lock")).exists());
        assert!(!dir.join(format!("{dead}.lock")).exists());
    }

    #[test]
    fn clean_json_emits_one_notice_per_path_and_a_total() {
        let harness = Harness::new("clean-json");
        fill(&harness, "bin");
        fill(&harness, "ide");
        let (code, out, err) = harness.run(&["clean", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        let notices: Vec<_> = events_of_kind(&events, "notice");
        assert_eq!(notices.len(), 3, "{out}");
        assert!(notices.iter().all(|n| n["code"] == "clean_removed"));
        assert_eq!(notices[0]["path"], ".dx/local/bin");
        assert_eq!(notices[0]["message"], "removed .dx/local/bin (1 bytes)");
        assert_eq!(notices[2]["message"], "removed 2 paths (2 bytes)");
        assert_eq!(events.last().expect("finished")["exit_code"], 0);
    }

    #[test]
    fn clean_exits_one_naming_the_holder_of_the_commit_lock() {
        let harness = Harness::new("clean-busy");
        fill(&harness, "bin");
        let _held = acquire_commit_lock(
            &ManagedRoot::new(local(&harness, "")),
            Duration::from_secs(10),
        )
        .expect("hold the lock");
        std::fs::write(local(&harness, ".commit.lock.holder"), "dx sync (pid 7)").expect("holder");
        let (code, out, err) = harness.run(&["clean", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(out.contains("clean_failed"), "{out}");
        assert!(out.contains("dx sync (pid 7)"), "{out}");
        assert!(
            local(&harness, "bin").exists(),
            "a busy lock deletes nothing"
        );
    }

    #[test]
    fn clean_bazel_and_expunge_run_bazel_clean_with_the_startup_options() {
        for (flag, want) in [
            ("--bazel", vec!["clean"]),
            ("--expunge", vec!["clean", "--expunge"]),
        ] {
            let harness = Harness::new("clean-bazel");
            let startup = "--bazel-startup-option=--output_base=/slot/ob";
            let (code, out, err) = harness.run(&["clean", flag, startup]);
            assert_eq!(code, 0, "{out}{err}");
            let calls = harness.build_argv.borrow();
            assert_eq!(calls.len(), 1, "{calls:?}");
            let argv = &calls[0];
            assert!(
                argv.iter().any(|arg| arg == "--output_base=/slot/ob"),
                "{argv:?}"
            );
            let tail: Vec<_> = argv.iter().rev().take(want.len()).rev().collect();
            assert_eq!(tail, want, "{argv:?}");
        }
    }

    #[test]
    fn clean_reports_a_failing_bazel_clean() {
        let mut harness = Harness::new("clean-bazel-fails");
        harness.bazel_code = 7;
        let (code, out, err) = harness.run(&["clean", "--bazel"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("bazel clean"), "{err}");
    }

    #[test]
    fn clean_refuses_apply_and_scopes() {
        let harness = Harness::new("clean-refusals");
        for words in [&["clean", "--apply"][..], &["clean", "//a:b"][..]] {
            let (code, out, err) = harness.run(words);
            assert_eq!(code, 2, "{out}{err}");
        }
    }
}
