"""Runfiles-relative paths of files, as the runfiles libraries expect them."""

def rlocation_path(ctx, file):
    """Returns the rlocation path of file, which starts with the repository name."""
    return runfiles_path(ctx.workspace_name, file.short_path)

def runfiles_path(workspace_name, short_path):
    """Returns one file's path below the runfiles root."""
    if short_path.startswith("../"):
        return short_path[3:]
    return workspace_name + "/" + short_path
