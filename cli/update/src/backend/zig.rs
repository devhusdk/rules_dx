use super::*;

pub(super) fn plan(
    workspace: &Path,
    set: &ResolvedSet,
    request: &Selection,
    offline: bool,
    frozen: bool,
    head: &dx_process::BazelHead,
) -> Result<BackendPlan, BackendError> {
    rehash::plan(
        "zig_digests",
        "add a zig_digests next to build.zig.zon; it rewrites zig.digests.json from the archives",
        workspace,
        set,
        request,
        offline,
        frozen,
        head,
    )
}
