//! Prunes setup generations a sync superseded. Never changes the exit code.

use std::io::Write;
use std::path::Path;

use dx_output::{NoticeEvent, OutputMode, notice_event};
use dx_setup::prune::{apply_plan, collect_inventory_with_scan_from};

use super::common::{emit_built, flush_out};
use super::notice_codes::CODE_PRUNE_FAILED;

fn prune(workspace: &Path, proc_root: &Path, previous: Option<&str>) -> Result<(), String> {
    let inventory = collect_inventory_with_scan_from(workspace, proc_root, previous)
        .map_err(|error| error.to_string())?;
    apply_plan(workspace, &inventory.plan()).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn prune_and_warn(
    workspace: &Path,
    proc_root: &Path,
    previous: Option<&str>,
    output: OutputMode,
    out: &mut dyn Write,
    err: &mut dyn Write,
) {
    let Err(message) = prune(workspace, proc_root, previous) else {
        return;
    };
    let message = format!("could not prune old setups: {message}");
    if output == OutputMode::Json {
        let event = notice_event(&NoticeEvent::warning(CODE_PRUNE_FAILED, &message));
        if emit_built(out, event).is_ok() {
            let _ = flush_out(out);
        }
    } else {
        let _ = writeln!(err, "dx: {CODE_PRUNE_FAILED}: {message}");
    }
}
