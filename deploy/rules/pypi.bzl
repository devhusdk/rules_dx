"""Local-first PyPI publisher for dx deploy."""

load("//libs/starlark:msvc_crt.bzl", "dx_msvc_crt_reset_transition")
load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
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
    wheel = ctx.attr.wheel[0]
    wheel_files = wheel[DefaultInfo].files.to_list()
    if len(wheel_files) != 1:
        fail("pypi_deploy " + str(ctx.label) + ": wheel " +
             str(wheel.label) + " provides " +
             str(len(wheel_files)) + " files, want exactly one .whl")
    wheel_file = wheel_files[0]
    wheel_error = pypi_wheel_error(wheel_file.basename)
    if wheel_error != "":
        fail(wheel_error + " (in " + str(ctx.label) + ")")
    wheel_rloc = rlocation_path(ctx, wheel_file)

    values = {
        "api_baseline": ctx.attr.api_baseline,
        "api_compat": ctx.attr.api_compat,
        "api_package": ctx.attr.api_package or ctx.attr.dist_name.replace("-", "_"),
        "api_src": ctx.attr.api_src,
        "dist_name": ctx.attr.dist_name,
        "repository_url": ctx.attr.repository_url,
        "wheel_rloc": wheel_rloc,
    }
    griffe = ctx.attr._griffe[DefaultInfo]
    if ctx.attr.api_compat != "off":
        values["griffe_rloc"] = rlocation_path(ctx, ctx.executable._griffe)
    files = [wheel_file]
    if ctx.attr.sdist:
        sdist = ctx.attr.sdist[0]
        sdist_files = sdist[DefaultInfo].files.to_list()
        if len(sdist_files) != 1:
            fail("pypi_deploy " + str(ctx.label) + ": sdist " +
                 str(sdist.label) + " provides " +
                 str(len(sdist_files)) + " files, want exactly one .tar.gz")
        sdist_file = sdist_files[0]
        if not sdist_file.basename.endswith(".tar.gz"):
            fail("pypi_deploy " + str(ctx.label) + ": sdist '" +
                 sdist_file.basename + "' must end in .tar.gz")
        values["sdist_rloc"] = rlocation_path(ctx, sdist_file)
        files.append(sdist_file)

    runfiles = ctx.runfiles(files = files)
    if ctx.attr.api_compat != "off":
        runfiles = runfiles.merge(griffe.default_runfiles)
    return deploy_config(ctx, "pypi", values, runfiles)

_pypi_config = rule(
    implementation = _pypi_config_impl,
    doc = "Writes the deploy config for one PyPI deployment.",
    attrs = API_COMPAT_ATTRS | {
        "api_package": attr.string(
            doc = "Import name of the package griffe checks; defaults to the distribution name.",
        ),
        "api_src": attr.string(
            doc = "Workspace-relative source directory griffe reads at the git baseline.",
        ),
        "_griffe": attr.label(
            default = Label("//quality/tools/python:griffe_check"),
            executable = True,
            cfg = "exec",
        ),
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
            cfg = dx_msvc_crt_reset_transition,
        ),
        "wheel": attr.label(
            doc = "Wheel file published as the main distribution.",
            allow_single_file = True,
            cfg = dx_msvc_crt_reset_transition,
            mandatory = True,
        ),
    },
)

def pypi_deploy(name, wheel, sdist = None, repository_url = PYPI_DEFAULT_REPOSITORY_URL, build_mode = "release", sbom = None, api_compat = "fail", api_baseline = "registry", api_package = "", api_src = ""):
    """Publishes one wheel plus an optional sdist as a local-first PyPI deployment.

    api_compat and api_baseline work as described under "Public API
    compatibility" in the README; the tool is griffe. api_package is the import
    name, which defaults to the distribution name, and api_src the
    workspace-relative source directory the git baseline needs.
    """
    name_error = pypi_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    api_error = api_compat_error("pypi_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    if api_compat != "off" and api_baseline == "git" and api_src == "":
        fail("pypi_deploy: api_baseline = \"git\" needs api_src (in " +
             native.package_name() + ":" + name + ")")
    repository_error = pypi_repository_error(repository_url)
    if repository_error != "":
        fail(repository_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _pypi_config(
        name = config_target,
        dist_name = name,
        api_baseline = api_baseline,
        api_compat = api_compat,
        api_package = api_package,
        api_src = api_src,
        repository_url = repository_url,
        sdist = sdist,
        wheel = wheel,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
    )
