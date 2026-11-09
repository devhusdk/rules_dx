"""Local-first PyPI publisher for dx deploy."""

load("//libs/starlark:msvc_crt.bzl", "dx_msvc_crt_reset_transition")
load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_pypi.bzl", "PYPI_DEFAULT_REGISTRY_URL", "pypi_name_error", "pypi_registry_url_error", "pypi_wheel_error")

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
        "registry_url": ctx.attr.registry_url,
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
        "registry_url": attr.string(
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

def pypi_deploy(name, wheel, sdist = None, registry_url = PYPI_DEFAULT_REGISTRY_URL, build_mode = "release", sbom = None, api_compat = "fail", api_baseline = "registry", api_package = "", api_src = "", size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Publishes one wheel plus an optional sdist as a local-first PyPI deployment. api_compat and api_baseline work as described under "Public API compatibility" in the README; the tool is griffe. api_package is the import name, which defaults to the distribution name, and api_src the workspace-relative source directory the git baseline needs."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    name_error = pypi_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    api_error = api_compat_error("pypi_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    if api_compat != "off" and api_baseline == "git" and api_src == "":
        fail("pypi_deploy: api_baseline = \"git\" needs api_src (in " +
             native.package_name() + ":" + name + ")")
    registry_error = pypi_registry_url_error(registry_url)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _pypi_config(
        name = config_target,
        dist_name = name,
        api_baseline = api_baseline,
        api_compat = api_compat,
        api_package = api_package,
        api_src = api_src,
        registry_url = registry_url,
        sdist = sdist,
        wheel = wheel,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [wheel] + ([sdist] if sdist else [])) | attrs)
    )
