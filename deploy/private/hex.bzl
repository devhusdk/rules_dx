"""Local-first Hex publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_hex.bzl", "hex_name_error", "hex_tool_error", "hex_version_error")

def _hex_config_impl(ctx):
    """Writes the deploy config for one Hex deployment."""
    package_files = ctx.files.srcs
    if len(package_files) == 0:
        fail("hex_deploy " + str(ctx.label) + ": srcs provide no files, want at least one source file")
    rlocs = [rlocation_path(ctx, f) for f in package_files]

    return deploy_config(
        ctx,
        "hex",
        {
            "package_name": ctx.attr.package_name,
            "package_rlocs": rlocs,
            "tool": ctx.attr.tool,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = package_files).merge_all([src[DefaultInfo].default_runfiles for src in ctx.attr.srcs]),
    )

_hex_config = rule(
    implementation = _hex_config_impl,
    doc = "Writes the deploy config for one Hex deployment.",
    attrs = {
        "srcs": attr.label_list(
            doc = "Package targets whose files are published by the deployment.",
            allow_files = True,
            mandatory = True,
        ),
        "package_name": attr.string(
            doc = "Name the package is published under on Hex.",
            mandatory = True,
        ),
        "tool": attr.string(
            doc = "Build tool that packs and pushes the package, mix or rebar3.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Package version the manifest must declare, empty to publish the manifest's version.",
        ),
    },
)

def hex_deploy(name, srcs, package = "", version = "", tool = "mix", build_mode = "release", sbom = None, size_budget = None, tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Publishes an Elixir or Erlang package as a local-first Hex deployment."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    pkg, version = deploy_identity(name, package, version, hex_name_error, hex_version_error)
    tool_error = hex_tool_error(tool)
    if tool_error != "":
        fail(tool_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _hex_config(
        name = config_target,
        srcs = srcs,
        package_name = pkg,
        tool = tool,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, srcs) | attrs)
    )
