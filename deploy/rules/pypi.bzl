"""Local-first PyPI publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_NAME_CHARS = NAME_CHARS

PYPI_DEFAULT_REPOSITORY_URL = "https://upload.pypi.org/legacy/"

def pypi_name_error(dist_name):
    """Validates one distribution name value."""
    if type(dist_name) != "string" or dist_name == "":
        return ("pypi_deploy: invalid distribution name '" + str(dist_name) +
                "': want a non-empty name (for example 'pypi_demo')")
    if not only_chars(dist_name, _VALID_NAME_CHARS):
        return ("pypi_deploy: invalid distribution name '" + dist_name +
                "': want only [A-Za-z0-9._-]")
    return ""

def pypi_repository_error(repository_url):
    """Validates one PyPI repository URL value."""
    if type(repository_url) != "string" or repository_url == "":
        return ("pypi_deploy: invalid repository_url '" + str(repository_url) +
                "': want a non-empty https URL (for example '" +
                PYPI_DEFAULT_REPOSITORY_URL + "')")
    if not repository_url.startswith("https://"):
        return ("pypi_deploy: invalid repository_url '" + repository_url +
                "': want an https URL so uploads never go over plaintext")
    return ""

def pypi_wheel_error(filename):
    """Validates one wheel filename value."""
    if type(filename) != "string" or filename == "":
        return ("pypi_deploy: invalid wheel '" + str(filename) +
                "': want a non-empty .whl filename")
    if not filename.endswith(".whl"):
        return ("pypi_deploy: invalid wheel '" + filename +
                "': want a filename ending in .whl")
    return ""

def _pypi_config_impl(ctx):
    """Writes the deploy config for one PyPI deployment."""
    wheel_files = ctx.attr.wheel[DefaultInfo].files.to_list()
    if len(wheel_files) != 1:
        fail("pypi_deploy " + str(ctx.label) + ": wheel " +
             str(ctx.attr.wheel.label) + " provides " +
             str(len(wheel_files)) + " files, want exactly one .whl")
    wheel_file = wheel_files[0]
    wheel_error = pypi_wheel_error(wheel_file.basename)
    if wheel_error != "":
        fail(wheel_error + " (in " + str(ctx.label) + ")")
    wheel_rloc = rlocation_path(ctx, wheel_file)

    values = {
        "dist_name": ctx.attr.dist_name,
        "repository_url": ctx.attr.repository_url,
        "wheel_rloc": wheel_rloc,
    }
    files = [wheel_file]
    if ctx.attr.sdist:
        sdist_files = ctx.attr.sdist[DefaultInfo].files.to_list()
        if len(sdist_files) != 1:
            fail("pypi_deploy " + str(ctx.label) + ": sdist " +
                 str(ctx.attr.sdist.label) + " provides " +
                 str(len(sdist_files)) + " files, want exactly one .tar.gz")
        sdist_file = sdist_files[0]
        if not sdist_file.basename.endswith(".tar.gz"):
            fail("pypi_deploy " + str(ctx.label) + ": sdist '" +
                 sdist_file.basename + "' must end in .tar.gz")
        values["sdist_rloc"] = rlocation_path(ctx, sdist_file)
        files.append(sdist_file)

    return deploy_config(ctx, "pypi", values, ctx.runfiles(files = files))

_pypi_config = rule(
    implementation = _pypi_config_impl,
    doc = "Writes the deploy config for one PyPI deployment.",
    attrs = {
        "dist_name": attr.string(
            doc = "PyPI distribution name, taken from the deployment target name.",
            mandatory = True,
        ),
        "repository_url": attr.string(
            doc = "Upload endpoint URL of the package index.",
            mandatory = True,
        ),
        "sdist": attr.label(
            doc = "Optional source distribution, a single .tar.gz file.",
            allow_single_file = True,
        ),
        "wheel": attr.label(
            doc = "Wheel file published as the main distribution.",
            allow_single_file = True,
            mandatory = True,
        ),
    },
)

def pypi_deploy(name, wheel, sdist = None, repository_url = PYPI_DEFAULT_REPOSITORY_URL, profile = "release"):
    """Publishes one wheel plus an optional sdist as a local-first PyPI deployment."""
    name_error = pypi_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    repository_error = pypi_repository_error(repository_url)
    if repository_error != "":
        fail(repository_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _pypi_config(
        name = config_target,
        dist_name = name,
        repository_url = repository_url,
        sdist = sdist,
        wheel = wheel,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
