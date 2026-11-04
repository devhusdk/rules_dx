//! The one-line notice for targets Bazel skipped as incompatible with the platform.

use super::common::{emit_built, CODE_TARGETS_SKIPPED};
use dx_bep::EventLedger;
use dx_output::{notice_event, NoticeEvent, OutputMode};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// Counts the targets the event file records as skipped.
pub(crate) fn count_skipped(bep: &Path) -> usize {
    let Ok(file) = std::fs::File::open(bep) else {
        return 0;
    };
    let mut ledger = EventLedger::default();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(serde_json::Value::Object(object)) = serde_json::from_str(&line) {
            ledger.observe(&object);
        }
    }
    ledger
        .gaps(&["targetCompleted"])
        .iter()
        .filter(|gap| gap.is_skipped() && gap.aspect.is_none())
        .count()
}

/// Tells the user how many targets were skipped. The exit code is not affected.
pub(crate) fn skipped_notice(
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
    count: usize,
) -> Result<(), i32> {
    if count == 0 {
        return Ok(());
    }
    let message = format!(
        "skipped {count} {} incompatible with this platform (docs/support-matrix.md)",
        if count == 1 { "target" } else { "targets" }
    );
    match output {
        OutputMode::Json => {
            emit_built(
                out,
                notice_event(&NoticeEvent::info(CODE_TARGETS_SKIPPED, message)),
            )?;
        }
        OutputMode::Text { .. } => {
            let _ = writeln!(err, "dx: {message}");
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skipped_line(label: &str) -> String {
        serde_json::json!({
            "id": {"targetCompleted": {"label": label, "configuration": {"id": "c"}}},
            "aborted": {"reason": "SKIPPED"},
        })
        .to_string()
    }

    #[test]
    fn counts_only_skipped_targets() {
        let dir = std::env::temp_dir().join(format!("dx-skipped-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bep = dir.join("bep.json");
        let done = serde_json::json!({
            "id": {"targetCompleted": {"label": "//a:ok", "configuration": {"id": "c"}}},
            "completed": {"success": true},
        });
        let text = format!(
            "{}\n{}\n{}\n",
            skipped_line("//a:x"),
            skipped_line("//a:y"),
            done
        );
        std::fs::write(&bep, text).unwrap();
        assert_eq!(count_skipped(&bep), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn text_notice_is_one_stderr_line() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let output = OutputMode::Text { quiet: false };
        skipped_notice(output, &mut out, &mut err, 2).unwrap();
        let err = String::from_utf8(err).unwrap();
        assert_eq!(
            err,
            "dx: skipped 2 targets incompatible with this platform (docs/support-matrix.md)\n"
        );
        assert!(out.is_empty());
    }

    #[test]
    fn json_notice_has_the_code_and_zero_prints_nothing() {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        skipped_notice(OutputMode::Json, &mut out, &mut err, 0).unwrap();
        assert!(out.is_empty() && err.is_empty());
        skipped_notice(OutputMode::Json, &mut out, &mut err, 1).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("\"targets_skipped\""), "{text}");
    }
}
