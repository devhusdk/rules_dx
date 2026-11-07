"""Local-first Homebrew tap publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "DxDeployConfigInfo", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "LOWER_ALNUM", "NAME_CHARS", "only_chars")

def homebrew_formula_error(formula):
    """Validates one formula name value."""
    if type(formula) != "string" or formula == "":
        return ("homebrew_tap_deploy: invalid formula '" + str(formula) +
                "': want a non-empty name (for example 'my-tool')")
    if not only_chars(formula, LOWER_ALNUM + "-_"):
        return ("homebrew_tap_deploy: invalid formula '" + formula +
                "': want only [a-z0-9_-]")
    return ""

def homebrew_repo_error(field, value):
    """Validates one GitHub owner/repository value."""
    parts = value.split("/") if type(value) == "string" else []
    if len(parts) != 2 or parts[0] == "" or parts[1] == "" or not only_chars(value, NAME_CHARS + "/"):
        return ("homebrew_tap_deploy: invalid " + field + " '" + str(value) +
                "': want owner/repository (for example 'org/homebrew-tap')")
    return ""

def _homebrew_config_impl(ctx):
    """Writes the deploy config for one Homebrew tap deployment."""
    release = ctx.attr.release[DxDeployConfigInfo]
    if release.kind != "github":
        fail("homebrew_tap_deploy " + str(ctx.label) + ": release " +
             str(ctx.attr.release.label) + " is a " + release.kind +
             " deployment, want a github_deploy target")
    return deploy_config(
        ctx,
        "homebrew_tap",
        {
            "binary": ctx.attr.binary,
            "deploy_name": ctx.attr.deploy_name,
            "desc": ctx.attr.desc,
            "formula": ctx.attr.formula,
            "homepage": ctx.attr.homepage,
            "release_rloc": rlocation_path(ctx, release.file),
            "repo": ctx.attr.repo,
            "tap": ctx.attr.tap,
        },
        ctx.runfiles(files = [release.file]).merge(ctx.attr.release[DefaultInfo].default_runfiles),
    )

_homebrew_config = rule(
    implementation = _homebrew_config_impl,
    doc = "Writes the deploy config for one Homebrew tap deployment.",
    attrs = {
        "binary": attr.string(mandatory = True, doc = "Binary the archives hold, installed into bin."),
        "deploy_name": attr.string(mandatory = True, doc = "Deploy target name."),
        "desc": attr.string(mandatory = True, doc = "Formula description."),
        "formula": attr.string(mandatory = True, doc = "Formula name."),
        "homepage": attr.string(mandatory = True, doc = "Formula homepage URL."),
        "release": attr.label(
            mandatory = True,
            providers = [DxDeployConfigInfo],
            doc = "The github_deploy target whose archives the formula points at.",
        ),
        "repo": attr.string(mandatory = True, doc = "GitHub owner/repository that hosts the release."),
        "tap": attr.string(mandatory = True, doc = "GitHub owner/repository of the tap."),
    },
)

def homebrew_tap_deploy(
        name,
        release,
        repo,
        tap,
        formula = None,
        binary = None,
        desc = None,
        homepage = None,
        build_mode = "release",
        tags = [],
        sbom = None):
    """Renders a Homebrew formula from a github_deploy release and pushes it to a tap."""
    formula = formula or name
    where = " (in " + native.package_name() + ":" + name + ")"
    for error in [
        homebrew_formula_error(formula),
        homebrew_repo_error("repo", repo),
        homebrew_repo_error("tap", tap),
    ]:
        if error != "":
            fail(error + where)

    config_target = name + "_config"
    _homebrew_config(
        name = config_target,
        binary = binary or formula,
        deploy_name = name,
        desc = desc or formula,
        formula = formula,
        homepage = homepage or "https://github.com/" + repo,
        release = release,
        repo = repo,
        tap = tap,
        tags = tags,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        tags = tags,
    )
