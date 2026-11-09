"""Local-first LuaRocks publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_luarocks.bzl", "luarocks_name_error", "luarocks_rockspec_error", "luarocks_version_error")

def _luarocks_config_impl(ctx):
    """Writes the deploy config for one LuaRocks deployment."""
    rockspec_files = ctx.attr.rockspec[DefaultInfo].files.to_list()
    if len(rockspec_files) != 1:
        fail("luarocks_deploy " + str(ctx.label) + ": rockspec " +
             str(ctx.attr.rockspec.label) + " provides " +
             str(len(rockspec_files)) + " files, want exactly one .rockspec")
    rockspec_file = rockspec_files[0]
    rockspec_error = luarocks_rockspec_error(rockspec_file.basename)
    if rockspec_error != "":
        fail(rockspec_error + " (in " + str(ctx.label) + ")")

    return deploy_config(
        ctx,
        "luarocks",
        {
            "deploy_name": ctx.attr.deploy_name,
            "package": ctx.attr.package,
            "rockspec_rloc": rlocation_path(ctx, rockspec_file),
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = [rockspec_file]),
    )

_luarocks_config = rule(
    implementation = _luarocks_config_impl,
    doc = "Writes the deploy config for one LuaRocks deployment.",
    attrs = {
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "package": attr.string(doc = "Rock package name the deploy config pins."),
        "rockspec": attr.label(
            doc = "Single rockspec file that is uploaded.",
            allow_single_file = True,
            mandatory = True,
        ),
        "version": attr.string(doc = "Rock version the deploy config pins."),
    },
)

def luarocks_deploy(name, rockspec, package = "", version = "", build_mode = "release", sbom = None, size_budget = None, tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Uploads one rockspec after checking its package and version."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    pkg, version = deploy_identity(name, package, version, luarocks_name_error, luarocks_version_error)

    config_target = name + "_config"
    _luarocks_config(
        name = config_target,
        deploy_name = name,
        package = pkg,
        rockspec = rockspec,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [rockspec]) | attrs)
    )
