"""Typed deploy configs that the shared deploy_tool reads at run time."""

load(":validate.bzl", "size_budget_error")

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

def deploy_identity(name, package, version, name_error, version_error):
    """Returns the (package, version) a deploy macro publishes. The package defaults to the target name. An empty version means the launcher reads it from the manifest."""
    pkg = package or name
    where = " (in " + native.package_name() + ":" + name + ")"
    error = name_error(pkg)
    if error != "":
        fail(error + where)
    if version != "":
        error = version_error(version)
        if error != "":
            fail(error + where)
    return pkg, version

def deployment_budget(name, size_budget, artifacts):
    """Returns the dx_deployment size_budget and artifacts arguments, failing on a bad budget."""
    error = size_budget_error(size_budget)
    if error != "":
        fail(error + " (in " + native.package_name() + ":" + name + ")")
    return {"artifacts": artifacts, "size_budget": size_budget}

def deploy_attrs(tags, visibility, testonly, target_compatible_with):
    """Returns the standard attributes every *_deploy macro forwards to each target it generates."""
    return {
        "tags": tags,
        "target_compatible_with": target_compatible_with,
        "testonly": testonly,
        "visibility": visibility,
    }
