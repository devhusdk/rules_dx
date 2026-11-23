"""Stages one target's own schemas under its package's private import root."""

visibility(["//capnproto/...", "//flatbuffers/...", "//thrift/..."])

def stage_own(ctx, prefix, entries):
    """Symlinks each entry to <package>/<prefix>/<entry.path> and returns struct(root, staged)."""
    staged = []
    for entry in entries:
        link = ctx.actions.declare_file(prefix + "/" + entry.path)
        ctx.actions.symlink(output = link, target_file = entry.file)
        staged.append(link)
    root = "/".join([p for p in [ctx.bin_dir.path, ctx.label.workspace_root, ctx.label.package, prefix] if p])
    return struct(root = root, staged = staged)

def check_import_paths(kind, label, entries):
    """Fails when two different files claim one import path."""
    sources = {}
    for entry in entries:
        seen = sources.get(entry.path)
        if seen == None:
            sources[entry.path] = entry.file
        elif seen != entry.file:
            fail(kind + " codegen " + str(label) + ": two different files claim the import path " + entry.path + ": " + seen.path + " and " + entry.file.path)
