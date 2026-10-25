"""Shared deploy-launcher helpers."""

def rlocation_path(ctx, f):
    """Returns the runfiles rlocation for one file."""
    sp = f.short_path
    if sp.startswith("../"):
        return sp[3:]
    return ctx.workspace_name + "/" + sp

def package_rlocation(ctx, name):
    """Returns the runfiles rlocation for one file name in the rule's package."""
    repo = ctx.label.repo_name or ctx.workspace_name
    return repo + "/" + (ctx.label.package + "/" if ctx.label.package else "") + name
