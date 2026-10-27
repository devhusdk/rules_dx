"""Octopus Deploy publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "EMBED_UNSAFE", "NAME_CHARS", "VERSION_CHARS", "has_any", "only_chars")

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
                "': want only [A-Za-z0-9._-] so the project embeds " +
                "safely in the deploy launcher")
    return ""

def octopus_channel_error(channel):
    """Validates one Octopus channel name value."""
    if type(channel) != "string" or channel == "":
        return ("octopus_deploy: invalid channel '" + str(channel) +
                "': want a non-empty channel (for example 'Default')")
    if not only_chars(channel, _VALID_CHANNEL_CHARS):
        return ("octopus_deploy: invalid channel '" + channel +
                "': want only [A-Za-z0-9._-] so the channel embeds " +
                "safely in the deploy launcher")
    return ""

def octopus_version_error(version):
    """Validates one Octopus release version value."""
    if type(version) != "string" or version == "":
        return ("octopus_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("octopus_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+] so the version embeds " +
                "safely in the deploy launcher")
    return ""

def octopus_environment_error(environment):
    """Validates one Octopus environment (--deployTo) value."""
    if type(environment) != "string" or environment == "":
        return ("octopus_deploy: invalid environment '" + str(environment) +
                "': want a non-empty environment (for example 'Production')")
    if not only_chars(environment, _VALID_ENVIRONMENT_CHARS):
        return ("octopus_deploy: invalid environment '" + environment +
                "': want only [A-Za-z0-9._-] so the environment embeds " +
                "safely in the deploy launcher")
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
                "': want only [A-Za-z0-9._-] so the space embeds " +
                "safely in the deploy launcher")
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
    if has_any(octopus_url, EMBED_UNSAFE):
        return ("octopus_deploy: invalid octopus_url '" + octopus_url +
                "': want no quotes, backslashes, spaces, or newlines " +
                "so the URL embeds safely in the deploy launcher")
    return ""

def octopus_package_error(filename):
    """Validates one Octopus package filename value."""
    if type(filename) != "string" or filename == "":
        return ("octopus_deploy: invalid package '" + str(filename) +
                "': want a non-empty .tar.gz filename from archive_deploy")
    if not filename.endswith(".tar.gz"):
        return ("octopus_deploy: invalid package '" + filename +
                "': want a filename ending in .tar.gz (archive_deploy output)")
    if has_any(filename, EMBED_UNSAFE):
        return ("octopus_deploy: invalid package '" + filename +
                "': want no quotes, backslashes, spaces, or newlines " +
                "so the filename embeds safely in the deploy launcher")
    return ""

def _octopus_launcher_impl(ctx):
    """Expands the rust_binary launcher for one Octopus deployment."""
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
    package_rloc = rlocation_path(ctx, package_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const PACKAGE_RLOC: &str = \"" + package_rloc + "\";\n" +
                  "const DEPLOY_NAME: &str = \"" + ctx.attr.drop_name + "\";\n" +
                  "const PROJECT: &str = \"" + ctx.attr.project + "\";\n" +
                  "const CHANNEL: &str = \"" + ctx.attr.channel + "\";\n" +
                  "const VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "const DEPLOY_TO: &str = \"" + ",".join(ctx.attr.deploy_to) + "\";\n" +
                  "const SPACE: &str = \"" + ctx.attr.space + "\";\n" +
                  "const OCTOPUS_URL: &str = \"" + ctx.attr.octopus_url + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::octopus_main(&dx_deploy_tools::OctopusLaunch { package_rloc: PACKAGE_RLOC, deploy_name: DEPLOY_NAME, project: PROJECT, channel: CHANNEL, version: VERSION, deploy_to_raw: DEPLOY_TO, space_default: SPACE, url_default: OCTOPUS_URL }, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_octopus_launcher = rule(
    implementation = _octopus_launcher_impl,
    doc = "Writes the launcher Rust source for one Octopus deployment.",
    attrs = {
        "channel": attr.string(
            mandatory = True,
        ),
        "deploy_to": attr.string_list(),
        "drop_name": attr.string(
            mandatory = True,
        ),
        "octopus_url": attr.string(
            mandatory = True,
        ),
        "package": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "project": attr.string(
            mandatory = True,
        ),
        "space": attr.string(
            mandatory = True,
        ),
        "version": attr.string(
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

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _octopus_launcher(
        name = launcher_target,
        channel = channel,
        deploy_to = deploy_to,
        drop_name = name,
        octopus_url = octopus_url,
        package = package,
        project = project,
        space = space,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [package],
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
