"""Local-first pub.dev publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "LOWER_ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = LOWER_ALNUM + "_"

_VALID_VERSION_CHARS = VERSION_CHARS

PUB_DEFAULT_HOSTED_URL = "https://pub.dev"

def pub_name_error(package_name):
    """Validates one pub package name value."""
    if type(package_name) != "string" or package_name == "":
        return ("pub_deploy: invalid package name '" + str(package_name) +
                "': want a non-empty name (for example 'pub_demo')")
    if not only_chars(package_name, _VALID_NAME_CHARS):
        return ("pub_deploy: invalid package name '" + package_name +
                "': want only [a-z0-9_]")
    return ""

def pub_version_error(version):
    """Validates one pub package version value."""
    if type(version) != "string" or version == "":
        return ("pub_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("pub_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def pub_hosted_url_error(hosted_url):
    """Validates one pub server URL value."""
    if type(hosted_url) != "string" or hosted_url == "":
        return ("pub_deploy: invalid hosted_url '" + str(hosted_url) +
                "': want a non-empty https URL (for example '" +
                PUB_DEFAULT_HOSTED_URL + "')")
    if not hosted_url.startswith("https://"):
        return ("pub_deploy: invalid hosted_url '" + hosted_url +
                "': want an https URL so publishes never go over plaintext")
    return ""

def _pub_config_impl(ctx):
    """Writes the deploy config for one pub.dev deployment."""
    package_files = ctx.attr.package[DefaultInfo].files.to_list()
    if len(package_files) == 0:
        fail("pub_deploy " + str(ctx.label) + ": package " +
             str(ctx.attr.package.label) + " provides no files, want at least pubspec.yaml")
    rlocs = [rlocation_path(ctx, f) for f in package_files]

    return deploy_config(
        ctx,
        "pub",
        {
            "hosted_url": ctx.attr.hosted_url,
            "package_name": ctx.attr.package_name,
            "package_rlocs": rlocs,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = package_files).merge(ctx.attr.package[DefaultInfo].default_runfiles),
    )

_pub_config = rule(
    implementation = _pub_config_impl,
    doc = "Writes the deploy config for one pub.dev deployment.",
    attrs = {
        "hosted_url": attr.string(
            doc = "HTTPS URL of the pub server.",
            mandatory = True,
        ),
        "package": attr.label(
            doc = "Package target whose files are published by the deployment.",
            allow_files = True,
            mandatory = True,
        ),
        "package_name": attr.string(
            doc = "Name the package is published under, taken from the deployment target name.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Package version published by the deployment.",
            mandatory = True,
        ),
    },
)

def pub_deploy(name, package, version = "0.0.0", hosted_url = PUB_DEFAULT_HOSTED_URL, profile = "release", sbom = None):
    """Publishes a Dart or Flutter package as a local-first pub.dev deployment."""
    name_error = pub_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = pub_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")
    hosted_error = pub_hosted_url_error(hosted_url)
    if hosted_error != "":
        fail(hosted_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _pub_config(
        name = config_target,
        hosted_url = hosted_url,
        package = package,
        package_name = name,
        version = version,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
