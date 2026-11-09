"""Octopus Deploy publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_octopus.bzl", "OCTOPUS_DEFAULT_REGISTRY_URL", "octopus_channel_error", "octopus_environment_error", "octopus_package_error", "octopus_package_id_error", "octopus_project_error", "octopus_registry_url_error", "octopus_space_error", "octopus_version_error")

def _octopus_config_impl(ctx):
    """Writes the deploy config for one Octopus deployment."""
    package_files = ctx.attr.srcs[DefaultInfo].files.to_list()
    if len(package_files) != 1:
        fail("octopus_deploy " + str(ctx.label) + ": srcs " +
             str(ctx.attr.srcs.label) + " provides " +
             str(len(package_files)) + " files, want exactly one .tar.gz " +
             "(archive_deploy output)")
    package_file = package_files[0]
    package_error = octopus_package_error(package_file.basename)
    if package_error != "":
        fail(package_error + " (in " + str(ctx.label) + ")")

    return deploy_config(
        ctx,
        "octopus",
        {
            "channel": ctx.attr.channel,
            "deploy_name": ctx.attr.drop_name,
            "deploy_to": ctx.attr.deploy_to,
            "registry_url": ctx.attr.registry_url,
            "package": ctx.attr.package,
            "package_rloc": rlocation_path(ctx, package_file),
            "project": ctx.attr.project,
            "space": ctx.attr.space,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = [package_file]),
    )

_octopus_config = rule(
    implementation = _octopus_config_impl,
    doc = "Writes the deploy config for one Octopus deployment.",
    attrs = {
        "channel": attr.string(
            doc = "Octopus channel the deployment publishes to.",
            mandatory = True,
        ),
        "deploy_to": attr.string_list(doc = "Octopus environments the release deploys to."),
        "drop_name": attr.string(
            doc = "Deployment name recorded in the Octopus config.",
            mandatory = True,
        ),
        "registry_url": attr.string(
            doc = "Default Octopus Server URL the deploy tool uses.",
            mandatory = True,
        ),
        "package": attr.string(
            doc = "Octopus package id the archive filename starts with.",
            mandatory = True,
        ),
        "project": attr.string(
            doc = "Octopus project the deployment is published under.",
            mandatory = True,
        ),
        "space": attr.string(
            doc = "Default Octopus space the deploy tool targets.",
            mandatory = True,
        ),
        "srcs": attr.label(
            doc = "Archive package to publish as the deployment.",
            allow_single_file = True,
            mandatory = True,
        ),
        "version": attr.string(doc = "Release version the archive filename must carry, when set."),
    },
)

def octopus_deploy(name, srcs, project, package = "", channel = "Default", version = "", deploy_to = [], space = "", registry_url = OCTOPUS_DEFAULT_REGISTRY_URL, build_mode = "release", sbom = None, size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Publishes one `<package>.<version>.tar.gz` archive as a local-first Octopus deployment. The package id defaults to the target name. An empty version means the launcher reads it from the archive filename."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    project_error = octopus_project_error(project)
    if project_error != "":
        fail(project_error + " (in " + native.package_name() + ":" + name + ")")
    channel_error = octopus_channel_error(channel)
    if channel_error != "":
        fail(channel_error + " (in " + native.package_name() + ":" + name + ")")
    package, version = deploy_identity(name, package, version, octopus_package_id_error, octopus_version_error)
    for env in deploy_to:
        env_error = octopus_environment_error(env)
        if env_error != "":
            fail(env_error + " (in " + native.package_name() + ":" + name + ")")
    space_error = octopus_space_error(space)
    if space_error != "":
        fail(space_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = octopus_registry_url_error(registry_url)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _octopus_config(
        name = config_target,
        channel = channel,
        deploy_to = deploy_to,
        drop_name = name,
        registry_url = registry_url,
        package = package,
        project = project,
        srcs = srcs,
        space = space,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [srcs]) | attrs)
    )
