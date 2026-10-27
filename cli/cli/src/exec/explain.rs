//! `dx explain`: why actions in the newer execution log missed the cache.

use std::io::Write;
use std::path::{Path, PathBuf};

use super::common::*;
use super::explain_store;
use dx_execlog::{CacheMiss, ExeclogError};
use dx_output::{
    cache_miss_event, command_finished, CacheMissChange, CacheMissEvent, FinishedCounts, OutputMode,
};
use dx_plan::args::{Command, Invocation};

const CODE_EXPLAIN_LOG_MISSING: &str = "explain_log_missing";
const CODE_EXPLAIN_LOG_UNREADABLE: &str = "explain_log_unreadable";

struct Problem {
    code: &'static str,
    message: String,
}

impl Problem {
    fn missing(message: String) -> Problem {
        Problem {
            code: CODE_EXPLAIN_LOG_MISSING,
            message,
        }
    }

    fn unreadable(path: &Path, error: &ExeclogError) -> Problem {
        match error {
            ExeclogError::Missing(path) => {
                Problem::missing(format!("no execution log at {}", path.display()))
            }
            _ => Problem {
                code: CODE_EXPLAIN_LOG_UNREADABLE,
                message: format!("cannot read {}: {error}", path.display()),
            },
        }
    }
}

fn resolve(workspace: &Path, given: &str) -> PathBuf {
    let path = Path::new(given);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

const NEED_TWO: &str =
    "dx explain needs two execution logs; run `dx build --explain` twice or pass --old and --new";

/// The `(old, new)` log pair: explicit paths first, then the saved logs.
fn pick(workspace: &Path, invocation: &Invocation) -> Result<(PathBuf, PathBuf), Problem> {
    let old = invocation.old_log.as_deref().map(|p| resolve(workspace, p));
    let new = invocation.new_log.as_deref().map(|p| resolve(workspace, p));
    if let (Some(old), Some(new)) = (&old, &new) {
        return Ok((old.clone(), new.clone()));
    }
    let saved = explain_store::latest(workspace, usize::MAX).map_err(|error| Problem {
        code: CODE_EXPLAIN_LOG_UNREADABLE,
        message: format!(
            "cannot list {}: {error}",
            explain_store::dir(workspace).display()
        ),
    })?;
    let other = |given: &Path| {
        saved
            .iter()
            .find(|path| !same_file(path, given))
            .cloned()
            .ok_or_else(|| Problem::missing(NEED_TWO.to_owned()))
    };
    match (old, new) {
        (Some(old), None) => {
            let new = other(&old)?;
            Ok((old, new))
        }
        (None, Some(new)) => {
            let old = other(&new)?;
            Ok((old, new))
        }
        _ => match saved.as_slice() {
            [newest, older, ..] => Ok((older.clone(), newest.clone())),
            _ => Err(Problem::missing(NEED_TWO.to_owned())),
        },
    }
}

fn misses(old: &Path, new: &Path) -> Result<Vec<CacheMiss>, Problem> {
    let old_log = dx_execlog::read(old).map_err(|error| Problem::unreadable(old, &error))?;
    let new_log = dx_execlog::read(new).map_err(|error| Problem::unreadable(new, &error))?;
    dx_execlog::explain(&old_log, &new_log).map_err(|error| Problem::unreadable(new, &error))
}

fn event(miss: &CacheMiss) -> CacheMissEvent {
    CacheMissEvent {
        mnemonic: miss.mnemonic.clone(),
        target: miss.target.clone(),
        primary_output: miss.primary_output.clone(),
        changes: miss
            .changes
            .iter()
            .map(|change| CacheMissChange {
                kind: change.kind.as_str().to_owned(),
                change: change
                    .delta
                    .map(|delta| delta.as_str().to_owned())
                    .unwrap_or_default(),
                subject: change.subject.clone(),
            })
            .collect(),
        changes_omitted: miss.changes_omitted as u64,
    }
}

fn text(misses: &[CacheMiss], out: &mut dyn Write) -> std::io::Result<()> {
    for miss in misses {
        let target = if miss.target.is_empty() {
            "-"
        } else {
            miss.target.as_str()
        };
        writeln!(out, "{} {target} {}", miss.mnemonic, miss.primary_output)?;
        for change in &miss.changes {
            let mut line = format!("  {}", change.kind.as_str());
            if let Some(delta) = change.delta {
                line.push(' ');
                line.push_str(delta.as_str());
            }
            if let Some(subject) = &change.subject {
                line.push(' ');
                line.push_str(subject);
            }
            writeln!(out, "{line}")?;
        }
        if miss.changes_omitted > 0 {
            writeln!(out, "  ... {} more changes", miss.changes_omitted)?;
        }
    }
    let noun = if misses.len() == 1 {
        "action"
    } else {
        "actions"
    };
    writeln!(out, "{} {noun} ran", misses.len())
}

pub(crate) fn execute_explain(invocation: &Invocation, env: Env<'_>) -> i32 {
    debug_assert!(
        invocation.command == Command::Explain,
        "explain dispatch guards commands"
    );
    let Env {
        workspace,
        out,
        err,
        ..
    } = env;
    let json = invocation.output == OutputMode::Json;
    if json {
        if let Err(exit) = emit_started(invocation, out) {
            return exit;
        }
    }
    let found = pick(workspace, invocation).and_then(|(old, new)| misses(&old, &new));
    let misses = match found {
        Ok(misses) => misses,
        Err(problem) => return operational(invocation, out, err, problem.code, &problem.message),
    };
    if !json {
        return match check_stdout_write(text(&misses, out)) {
            Ok(()) => 0,
            Err(exit) => exit,
        };
    }
    for miss in &misses {
        if let Err(exit) = emit_built(out, cache_miss_event(&event(miss))) {
            return exit;
        }
    }
    if let Err(exit) = emit_event(out, &command_finished(0, &FinishedCounts::default())) {
        return exit;
    }
    match flush_out(out) {
        Ok(()) => 0,
        Err(exit) => exit,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use dx_execlog::proto::exec_log_entry::{
        output, File, InputSet, Invocation as Inv, Output, Spawn, Type,
    };
    use dx_execlog::proto::{Digest, ExecLogEntry};
    use std::path::Path;

    fn entry(id: u32, payload: Type) -> ExecLogEntry {
        ExecLogEntry {
            id,
            r#type: Some(payload),
        }
    }

    fn file(id: u32, path: &str, hash: &str) -> ExecLogEntry {
        entry(
            id,
            Type::File(File {
                path: path.to_owned(),
                digest: Some(Digest {
                    hash: hash.to_owned(),
                    size_bytes: 1,
                    hash_function_name: String::new(),
                }),
            }),
        )
    }

    fn log_with(source_hash: &str, output: &str) -> Vec<u8> {
        dx_execlog::encode(&[
            entry(
                0,
                Type::Invocation(Inv {
                    hash_function_name: "SHA-256".to_owned(),
                    workspace_runfiles_directory: "_main".to_owned(),
                    id: "run".to_owned(),
                }),
            ),
            file(1, "src/lib.rs", source_hash),
            file(2, output, "out"),
            entry(
                3,
                Type::InputSet(InputSet {
                    input_ids: vec![1],
                    transitive_set_ids: vec![],
                }),
            ),
            entry(
                0,
                Type::Spawn(Spawn {
                    args: vec!["rustc".to_owned()],
                    input_set_id: 3,
                    outputs: vec![Output {
                        r#type: Some(output::Type::OutputId(2)),
                    }],
                    target_label: "//app:lib".to_owned(),
                    mnemonic: "Rustc".to_owned(),
                    cacheable: true,
                    remote_cacheable: true,
                    ..Spawn::default()
                }),
            ),
        ])
    }

    fn save(harness: &Harness, name: &str, bytes: &[u8]) {
        let dir = harness.workspace.join(".dx").join("explain");
        std::fs::create_dir_all(&dir).expect("store");
        std::fs::write(dir.join(name), bytes).expect("log");
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("log");
        path.to_string_lossy().into_owned()
    }

    const OUT: &str = "bazel-out/k8/bin/app/liblib.rlib";

    #[test]
    fn default_pair_is_the_two_newest_saved_logs() {
        let harness = Harness::new("explain-default");
        save(&harness, "1.binpb.zst", b"not a log");
        save(&harness, "2.binpb.zst", &log_with("h1", OUT));
        save(&harness, "3.binpb.zst", &log_with("h2", OUT));
        let (code, out, err) = harness.run(&["explain"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            format!("Rustc //app:lib {OUT}\n  input modified src/lib.rs\n1 action ran\n")
        );
    }

    #[test]
    fn explicit_pair_resolves_against_the_workspace() {
        let harness = Harness::new("explain-explicit");
        write(&harness.workspace, "a.zst", &log_with("h", OUT));
        let absolute = write(&harness.temp, "b.zst", &log_with("h", OUT));
        let (code, out, err) =
            harness.run(&["explain", "--old=a.zst", &format!("--new={absolute}")]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(out, format!("Rustc //app:lib {OUT}\n1 action ran\n"));
    }

    #[test]
    fn one_sided_pair_uses_the_newest_other_saved_log() {
        let harness = Harness::new("explain-one-sided");
        save(&harness, "4.binpb.zst", &log_with("h1", OUT));
        save(&harness, "5.binpb.zst", &log_with("h2", OUT));
        let (code, out, err) = harness.run(&["explain", "--old=.dx/explain/5.binpb.zst"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("input modified src/lib.rs"), "{out}");
        let (code, out, err) = harness.run(&["explain", "--new=.dx/explain/4.binpb.zst"]);
        assert_eq!(code, 0, "{out}{err}");
        assert!(out.contains("input modified src/lib.rs"), "{out}");
    }

    #[test]
    fn missing_logs_fail_with_explain_log_missing() {
        let harness = Harness::new("explain-missing");
        let (code, out, err) = harness.run(&["explain"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("explain_log_missing"), "{err}");
        save(&harness, "1.binpb.zst", &log_with("h", OUT));
        let (code, _, err) = harness.run(&["explain"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("explain_log_missing"), "{err}");
        let (code, _, err) = harness.run(&["explain", "--old=gone.zst"]);
        assert_eq!(code, 1, "{err}");
        assert!(err.contains("explain_log_missing"), "{err}");
        assert!(err.contains("gone.zst"), "{err}");
    }

    #[test]
    fn corrupt_logs_fail_with_explain_log_unreadable() {
        let harness = Harness::new("explain-corrupt");
        save(&harness, "1.binpb.zst", &log_with("h", OUT));
        save(&harness, "2.binpb.zst", b"plain text");
        let (code, out, err) = harness.run(&["explain", "--output=json"]);
        assert_eq!(code, 1, "{out}{err}");
        assert!(err.contains("explain_log_unreadable"), "{err}");
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            ["command_started", "error", "command_finished"]
        );
    }

    #[test]
    fn unmatched_actions_are_not_in_old_log() {
        let harness = Harness::new("explain-unmatched");
        save(&harness, "1.binpb.zst", &dx_execlog::encode(&[]));
        save(&harness, "2.binpb.zst", &log_with("h", OUT));
        let (code, out, err) = harness.run(&["explain"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            out,
            format!("Rustc //app:lib {OUT}\n  not_in_old_log\n1 action ran\n")
        );
    }

    #[test]
    fn json_emits_one_cache_miss_per_action() {
        let harness = Harness::new("explain-json");
        save(&harness, "1.binpb.zst", &log_with("h1", OUT));
        save(&harness, "2.binpb.zst", &log_with("h2", OUT));
        let (code, out, err) = harness.run(&["explain", "--output=json"]);
        assert_eq!(code, 0, "{out}{err}");
        let events = json_events(&out);
        assert_eq!(
            event_kinds(&events),
            ["command_started", "cache_miss", "command_finished"]
        );
        let miss = &events[1];
        assert_eq!(miss["mnemonic"], "Rustc");
        assert_eq!(miss["target"], "//app:lib");
        assert_eq!(miss["primary_output"], OUT);
        assert_eq!(
            miss["changes"],
            serde_json::json!([{"kind": "input", "change": "modified", "path": "src/lib.rs"}])
        );
        assert_eq!(events[2]["exit_code"], 0);
    }
}
