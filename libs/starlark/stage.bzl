"""Stages a declared schema closure under a private root for codegen actions."""

def stage_closure(ctx, prefix, entries, kind):
    """Symlinks schema files under a private root.

    Args:
      ctx: the rule or aspect context.
      prefix: package-relative directory to stage under; must be unique per target.
      entries: list of struct(path, file) to stage.
      kind: schema language name, used in failure messages.

    Returns:
      A struct(root, files, staged): root is the exec path of the staging root, files
      the staged symlinks, and staged maps each path to its symlink.
    """
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
