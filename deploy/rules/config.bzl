"""Typed deploy configs that the shared deploy_tool reads at run time."""

DxDeployConfigInfo = provider(
    doc = "One deploy config JSON file that the shared deploy_tool reads through DX_DEPLOY_CONFIG.",
    fields = {
        "file": "File: the JSON config, {\"kind\": ..., \"config\": {...}}.",
        "kind": "String: the deploy kind the tool dispatches on.",
    },
)

DEPLOY_TOOL = Label("//deploy/rules:deploy_tool")

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

def deploy_config(ctx, kind, values, runfiles):
    """Writes one deploy config and returns its DefaultInfo and DxDeployConfigInfo."""
    out = ctx.actions.declare_file(ctx.label.name + ".deploy.json")
    ctx.actions.write(
        output = out,
        content = json.encode({"config": values, "kind": kind}) + "\n",
    )
    return [
        DefaultInfo(
            files = depset([out]),
            runfiles = ctx.runfiles(files = [out]).merge(runfiles),
        ),
        DxDeployConfigInfo(file = out, kind = kind),
    ]
