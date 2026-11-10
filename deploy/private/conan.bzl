"""Local-first Conan publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_conan.bzl", "conan_name_error", "conan_remote_error", "conan_remote_url_error", "conan_version_error")

def _conan_config_impl(ctx):
    """Writes the deploy config for one Conan deployment."""
    recipe_files = ctx.attr.recipe[DefaultInfo].files.to_list()
    if len(recipe_files) == 0:
        fail("conan_deploy " + str(ctx.label) + ": recipe " +
             str(ctx.attr.recipe.label) + " provides no files, want conanfile.py and its sources")
    conan = ctx.file.conan
    runfiles = ctx.runfiles(files = recipe_files + ctx.files.conan_files + ([conan] if conan else []))

    return deploy_config(
        ctx,
        "conan",
        {
            "conan_rloc": rlocation_path(ctx, conan) if conan else "",
            "deploy_name": ctx.attr.deploy_name,
            "package": ctx.attr.package,
            "recipe_rlocs": [rlocation_path(ctx, f) for f in recipe_files],
            "remote": ctx.attr.remote,
            "remote_url": ctx.attr.remote_url,
            "version": ctx.attr.version,
        },
        runfiles.merge(ctx.attr.recipe[DefaultInfo].default_runfiles),
    )

_conan_config = rule(
    implementation = _conan_config_impl,
    doc = "Writes the deploy config for one Conan deployment.",
    attrs = {
        "conan": attr.label(
            doc = "Pinned conan executable. Empty takes conan from PATH.",
            allow_single_file = True,
            cfg = "exec",
        ),
        "conan_files": attr.label(
            doc = "Files the pinned conan needs at run time.",
            allow_files = True,
            cfg = "exec",
        ),
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "package": attr.string(doc = "Package name the deploy config pins."),
        "recipe": attr.label(
            doc = "Target whose files (conanfile.py and its sources) are created and uploaded.",
            allow_files = True,
            mandatory = True,
        ),
        "remote": attr.string(doc = "Name the remote is added under.", mandatory = True),
        "remote_url": attr.string(doc = "https URL of the remote.", mandatory = True),
        "version": attr.string(doc = "Package version the recipe must declare, empty to publish the recipe's version."),
    },
)

def conan_deploy(name, recipe, remote, remote_url, package = "", version = "", conan = None, conan_files = None, build_mode = "release", sbom = None, size_budget = None, tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Runs conan create and conan upload for a recipe against a named remote."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    pkg, version = deploy_identity(name, package, version, conan_name_error, conan_version_error)
    where = " (in " + native.package_name() + ":" + name + ")"
    for error in [conan_remote_error(remote), conan_remote_url_error(remote_url)]:
        if error != "":
            fail(error + where)

    config_target = name + "_config"
    _conan_config(
        name = config_target,
        conan = conan,
        conan_files = conan_files,
        deploy_name = name,
        package = pkg,
        recipe = recipe,
        remote = remote,
        remote_url = remote_url,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [recipe]) | attrs)
    )
