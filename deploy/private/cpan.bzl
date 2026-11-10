"""Local-first CPAN publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_cpan.bzl", "cpan_dist_error", "cpan_name_error", "cpan_version_error")

def _cpan_config_impl(ctx):
    """Writes the deploy config for one CPAN deployment."""
    dist_files = ctx.attr.dist[DefaultInfo].files.to_list()
    if len(dist_files) != 1:
        fail("cpan_deploy " + str(ctx.label) + ": dist " +
             str(ctx.attr.dist.label) + " provides " +
             str(len(dist_files)) + " files, want exactly one .tar.gz")
    dist_file = dist_files[0]
    dist_error = cpan_dist_error(dist_file.basename)
    if dist_error != "":
        fail(dist_error + " (in " + str(ctx.label) + ")")

    return deploy_config(
        ctx,
        "cpan",
        {
            "deploy_name": ctx.attr.deploy_name,
            "dist_rloc": rlocation_path(ctx, dist_file),
            "package": ctx.attr.package,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = [dist_file]),
    )

_cpan_config = rule(
    implementation = _cpan_config_impl,
    doc = "Writes the deploy config for one CPAN deployment.",
    attrs = {
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "dist": attr.label(
            doc = "Single distribution tarball (.tar.gz) that is uploaded.",
            allow_single_file = True,
            mandatory = True,
        ),
        "package": attr.string(doc = "Distribution name the deploy config pins."),
        "version": attr.string(doc = "Distribution version the deploy config pins."),
    },
)

def cpan_deploy(name, dist, package = "", version = "", build_mode = "release", sbom = None, size_budget = None, tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Uploads one distribution tarball to PAUSE after checking its name and version."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    pkg, version = deploy_identity(name, package, version, cpan_name_error, cpan_version_error)

    config_target = name + "_config"
    _cpan_config(
        name = config_target,
        deploy_name = name,
        dist = dist,
        package = pkg,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [dist]) | attrs)
    )
