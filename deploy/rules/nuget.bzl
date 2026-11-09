"""Local-first NuGet publisher for dx deploy."""

load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

_VALID_ID_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

NUGET_DEFAULT_REGISTRY_URL = "https://api.nuget.org/v3/index.json"

def nuget_id_error(package_id):
    """Validates one package id value."""
    if type(package_id) != "string" or package_id == "":
        return ("nuget_deploy: invalid package id '" + str(package_id) +
                "': want a non-empty id (for example 'nuget_demo')")
    if not only_chars(package_id, _VALID_ID_CHARS):
        return ("nuget_deploy: invalid package id '" + package_id +
                "': want only [A-Za-z0-9._-]")
    return ""

def nuget_version_error(version):
    """Validates one package version value."""
    if type(version) != "string" or version == "":
        return ("nuget_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("nuget_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def nuget_nupkg_error(filename):
    """Validates one nupkg filename value."""
    if type(filename) != "string" or filename == "":
        return ("nuget_deploy: invalid nupkg '" + str(filename) +
                "': want a non-empty .nupkg filename")
    if not filename.endswith(".nupkg"):
        return ("nuget_deploy: invalid nupkg '" + filename +
                "': want a filename ending in .nupkg")
    return ""

def nuget_registry_url_error(registry_url):
    """Validates one NuGet registry URL value."""
    if type(registry_url) != "string" or registry_url == "":
        return ("nuget_deploy: invalid registry_url '" + str(registry_url) +
                "': want a non-empty https URL (for example '" +
                NUGET_DEFAULT_REGISTRY_URL + "')")
    if not registry_url.startswith("https://"):
        return ("nuget_deploy: invalid registry_url '" + registry_url +
                "': want an https URL so pushes never go over plaintext")
    return ""

def _nuget_config_impl(ctx):
    """Writes the deploy config for one NuGet deployment."""
    nupkg_files = ctx.attr.nupkg[DefaultInfo].files.to_list()
    if len(nupkg_files) != 1:
        fail("nuget_deploy " + str(ctx.label) + ": nupkg " +
             str(ctx.attr.nupkg.label) + " provides " +
             str(len(nupkg_files)) + " files, want exactly one .nupkg")
    nupkg_file = nupkg_files[0]
    nupkg_error = nuget_nupkg_error(nupkg_file.basename)
    if nupkg_error != "":
        fail(nupkg_error + " (in " + str(ctx.label) + ")")
    nupkg_rloc = rlocation_path(ctx, nupkg_file)

    values = {
        "deploy_name": ctx.attr.deploy_name,
        "nupkg_rloc": nupkg_rloc,
        "package_id": ctx.attr.package,
        "registry_url": ctx.attr.registry_url,
        "version": ctx.attr.version,
    }
    runfiles = ctx.runfiles(files = [nupkg_file])
    if ctx.attr.api_compat != "off":
        values["api_compat"] = ctx.attr.api_compat
        values["api_baseline"] = ctx.attr.api_baseline
        dll = [f for f in ctx.files._apicompat if f.basename == "Microsoft.DotNet.ApiCompat.Tool.dll"]
        values["apicompat_rloc"] = rlocation_path(ctx, dll[0]) if dll else ""
        toolchain = ctx.toolchains[_DOTNET_TOOLCHAIN]
        values["dotnet_rloc"] = ""
        if toolchain:
            runtime = toolchain.runtime
            values["dotnet_rloc"] = rlocation_path(ctx, runtime.files_to_run.executable)
            runfiles = runfiles.merge(ctx.runfiles(
                files = ctx.files._apicompat + [runtime.files_to_run.executable],
                transitive_files = runtime.default_runfiles.files,
            ))
    return deploy_config(ctx, "nuget", values, runfiles)

_DOTNET_TOOLCHAIN = "@rules_dotnet//dotnet:toolchain_type"

_nuget_config = rule(
    implementation = _nuget_config_impl,
    doc = "Writes the deploy config for one NuGet deployment.",
    toolchains = [config_common.toolchain_type(_DOTNET_TOOLCHAIN, mandatory = False)],
    attrs = API_COMPAT_ATTRS | {
        "_apicompat": attr.label(default = Label("//deploy/rules/pins:apicompat")),
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "nupkg": attr.label(
            doc = "Single NuGet package file that is published.",
            allow_single_file = True,
            mandatory = True,
        ),
        "package": attr.string(doc = "NuGet package id the deploy config pins."),
        "registry_url": attr.string(
            doc = "HTTPS NuGet feed URL the deploy tool publishes to.",
            mandatory = True,
        ),
        "version": attr.string(doc = "NuGet package version the deploy config pins."),
    },
)

def nuget_deploy(name, nupkg, package = "", version = "", registry_url = NUGET_DEFAULT_REGISTRY_URL, build_mode = "release", sbom = None, api_compat = "fail", api_baseline = "registry", size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Publishes one nupkg after checking its nuspec id and version. api_compat and api_baseline work as described under "Public API compatibility" in the README; the tool is Microsoft.DotNet.ApiCompat."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    pkg, version = deploy_identity(name, package, version, nuget_id_error, nuget_version_error)
    api_error = api_compat_error("nuget_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = nuget_registry_url_error(registry_url)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _nuget_config(
        name = config_target,
        api_baseline = api_baseline,
        api_compat = api_compat,
        deploy_name = name,
        nupkg = nupkg,
        package = pkg,
        registry_url = registry_url,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [nupkg]) | attrs)
    )
