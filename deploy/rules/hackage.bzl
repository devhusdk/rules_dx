"""Local-first Hackage publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "ALNUM", "only_chars")

_VALID_NAME_CHARS = ALNUM + "-"

_VALID_VERSION_CHARS = "0123456789."

def hackage_name_error(package):
    """Validates one Hackage package name value."""
    if type(package) != "string" or package == "":
        return ("hackage_deploy: invalid package name '" + str(package) +
                "': want a non-empty name (for example 'hackage_demo')")
    if not only_chars(package, _VALID_NAME_CHARS):
        return ("hackage_deploy: invalid package name '" + package +
                "': want only [A-Za-z0-9-]")
    return ""

def hackage_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("hackage_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.1.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("hackage_deploy: invalid version '" + version +
                "': want only [0-9.]")
    return ""

def hackage_sdist_error(filename):
    """Validates one sdist filename value."""
    if type(filename) != "string" or filename == "":
        return ("hackage_deploy: invalid sdist '" + str(filename) +
                "': want a non-empty .tar.gz filename")
    if not filename.endswith(".tar.gz"):
        return ("hackage_deploy: invalid sdist '" + filename +
                "': want a filename ending in .tar.gz")
    return ""

def _hackage_config_impl(ctx):
    """Writes the deploy config for one Hackage deployment."""
    sdist_files = ctx.attr.sdist[DefaultInfo].files.to_list()
    if len(sdist_files) != 1:
        fail("hackage_deploy " + str(ctx.label) + ": sdist " +
             str(ctx.attr.sdist.label) + " provides " +
             str(len(sdist_files)) + " files, want exactly one .tar.gz")
    sdist_file = sdist_files[0]
    sdist_error = hackage_sdist_error(sdist_file.basename)
    if sdist_error != "":
        fail(sdist_error + " (in " + str(ctx.label) + ")")

    return deploy_config(
        ctx,
        "hackage",
        {
            "deploy_name": ctx.attr.deploy_name,
            "package": ctx.attr.package,
            "sdist_rloc": rlocation_path(ctx, sdist_file),
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = [sdist_file]),
    )

_hackage_config = rule(
    implementation = _hackage_config_impl,
    doc = "Writes the deploy config for one Hackage deployment.",
    attrs = {
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "package": attr.string(doc = "Package name the deploy config pins."),
        "sdist": attr.label(
            doc = "Single source distribution (.tar.gz) that is uploaded.",
            allow_single_file = True,
            mandatory = True,
        ),
        "version": attr.string(doc = "Package version the deploy config pins."),
    },
)

def hackage_deploy(name, sdist, package = "", version = "", profile = "release", sbom = None):
    """Uploads one cabal sdist after checking its .cabal name and version."""
    name_error = hackage_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    if package != "":
        package_error = hackage_name_error(package)
        if package_error != "":
            fail(package_error + " (in " + native.package_name() + ":" + name + ")")
    if version != "":
        version_error = hackage_version_error(version)
        if version_error != "":
            fail(version_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _hackage_config(
        name = config_target,
        deploy_name = name,
        package = package,
        sdist = sdist,
        version = version,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
