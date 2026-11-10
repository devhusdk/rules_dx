"""Stages a declared schema closure under a private root for codegen actions."""

def stage_closure(ctx, prefix, entries, kind):
    """Symlinks schema files under a private root."""
    staged = {}
    sources = {}
    for entry in entries:
        if entry.path.startswith("../"):
            fail("{kind} codegen {label}: schema {path} is in another Bazel repository; cross-repo {kind} schemas are unsupported".format(
                kind = kind,
                label = ctx.label,
                path = entry.path,
            ))
        if entry.path in sources:
            if sources[entry.path] != entry.file:
                fail(kind + " codegen: two different files claim the import path " + entry.path + ": " + sources[entry.path].path + " and " + entry.file.path)
            continue
        sources[entry.path] = entry.file
        link = ctx.actions.declare_file(prefix + "/" + entry.path)
        ctx.actions.symlink(output = link, target_file = entry.file)
        staged[entry.path] = link
    pkg = ctx.label.package
    root = ctx.bin_dir.path + "/" + (pkg + "/" if pkg else "") + prefix
    return struct(root = root, files = staged.values(), staged = staged)
