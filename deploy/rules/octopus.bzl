"""Octopus Deploy publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

_VALID_PROJECT_CHARS = NAME_CHARS

_VALID_CHANNEL_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

_VALID_ENVIRONMENT_CHARS = NAME_CHARS

OCTOPUS_DEFAULT_URL = "https://octopus.example.invalid"

def octopus_project_error(project):
    """Validates one Octopus project name value."""
    if type(project) != "string" or project == "":
        return ("octopus_deploy: invalid project '" + str(project) +
                "': want a non-empty project (for example 'octopus_demo')")
    if not only_chars(project, _VALID_PROJECT_CHARS):
        return ("octopus_deploy: invalid project '" + project +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_channel_error(channel):
    """Validates one Octopus channel name value."""
    if type(channel) != "string" or channel == "":
        return ("octopus_deploy: invalid channel '" + str(channel) +
                "': want a non-empty channel (for example 'Default')")
    if not only_chars(channel, _VALID_CHANNEL_CHARS):
        return ("octopus_deploy: invalid channel '" + channel +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_version_error(version):
    """Validates one Octopus release version value."""
    if type(version) != "string" or version == "":
        return ("octopus_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("octopus_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def octopus_environment_error(environment):
    """Validates one Octopus environment (--deployTo) value."""
    if type(environment) != "string" or environment == "":
        return ("octopus_deploy: invalid environment '" + str(environment) +
                "': want a non-empty environment (for example 'Production')")
    if not only_chars(environment, _VALID_ENVIRONMENT_CHARS):
        return ("octopus_deploy: invalid environment '" + environment +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_space_error(space):
    """Validates one Octopus space name value (empty means the default space)."""
    if space == "":
        return ""
    if type(space) != "string":
        return ("octopus_deploy: invalid space '" + str(space) +
                "': want a space name (for example 'Default') or empty for the default space")
    if not only_chars(space, _VALID_ENVIRONMENT_CHARS):
        return ("octopus_deploy: invalid space '" + space +
                "': want only [A-Za-z0-9._-]")
    return ""

def octopus_url_error(octopus_url):
    """Validates one Octopus server URL value."""
    if type(octopus_url) != "string" or octopus_url == "":
        return ("octopus_deploy: invalid octopus_url '" + str(octopus_url) +
                "': want a non-empty https URL (for example '" +
                OCTOPUS_DEFAULT_URL + "')")
    if not octopus_url.startswith("https://"):
        return ("octopus_deploy: invalid octopus_url '" + octopus_url +
                "': want an https URL so pushes never go over plaintext")
    return ""

def octopus_package_error(filename):
    """Validates one Octopus package filename value."""
    if type(filename) != "string" or filename == "":
        return ("octopus_deploy: invalid package '" + str(filename) +
                "': want a non-empty .tar.gz filename from archive_deploy")
    if not filename.endswith(".tar.gz"):
        return ("octopus_deploy: invalid package '" + filename +
                "': want a filename ending in .tar.gz (archive_deploy output)")
    return ""

def _octopus_config_impl(ctx):
    """Writes the deploy config for one Octopus deployment."""
    package_files = ctx.attr.package[DefaultInfo].files.to_list()
    if len(package_files) != 1:
        fail("octopus_deploy " + str(ctx.label) + ": package " +
             str(ctx.attr.package.label) + " provides " +
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
            "octopus_url": ctx.attr.octopus_url,
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
        "octopus_url": attr.string(
            doc = "Default Octopus Server URL the deploy tool uses.",
            mandatory = True,
        ),
        "package": attr.label(
            doc = "Archive package to publish as the deployment.",
            allow_single_file = True,
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
        "version": attr.string(
            doc = "Release version sent with the Octopus deployment.",
            mandatory = True,
        ),
    },
)

def octopus_deploy(name, package, project, channel = "Default", version = "0.0.0", deploy_to = [], space = "", octopus_url = OCTOPUS_DEFAULT_URL, profile = "release"):
    """Publishes one archive package as a local-first Octopus deployment."""
    project_error = octopus_project_error(project)
    if project_error != "":
        fail(project_error + " (in " + native.package_name() + ":" + name + ")")
    channel_error = octopus_channel_error(channel)
    if channel_error != "":
        fail(channel_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = octopus_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")
    for env in deploy_to:
        env_error = octopus_environment_error(env)
        if env_error != "":
            fail(env_error + " (in " + native.package_name() + ":" + name + ")")
    space_error = octopus_space_error(space)
    if space_error != "":
        fail(space_error + " (in " + native.package_name() + ":" + name + ")")
    url_error = octopus_url_error(octopus_url)
    if url_error != "":
        fail(url_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _octopus_config(
        name = config_target,
        channel = channel,
        deploy_to = deploy_to,
        drop_name = name,
        octopus_url = octopus_url,
        package = package,
        project = project,
        space = space,
        version = version,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
