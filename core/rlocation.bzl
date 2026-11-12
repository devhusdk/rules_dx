"""Runfiles-relative paths of files, as the runfiles libraries expect them."""

def rlocation_path(ctx, file):
    """Returns the rlocation path of file, which starts with the repository name."""
    return runfiles_path(ctx.workspace_name, file.short_path)

def runfiles_path(workspace_name, short_path):
    """Returns one file's path below the runfiles root."""
    if short_path.startswith("../"):
        return short_path[3:]
    return workspace_name + "/" + short_path

def external_path(short_path):
    """Returns a File.short_path below the autoload root: external/<repo>/... for other repositories."""
    if short_path.startswith("../"):
        return "external/" + short_path[3:]
    return short_path

def repo_relative_path(short_path):
    """Returns a File.short_path relative to the root of its own repository."""
    if short_path.startswith("../"):
        rest = short_path[3:]
        slash = rest.find("/")
        return rest[slash + 1:] if slash >= 0 else ""
    return short_path
