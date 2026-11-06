"""Runfiles-relative paths of files, as the runfiles libraries expect them."""

def rlocation_path(ctx, file):
    """Returns the rlocation path of file, which starts with the repository name."""
    path = file.short_path
    if path.startswith("../"):
        return path[3:]
    return ctx.workspace_name + "/" + path
