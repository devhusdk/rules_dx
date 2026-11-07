"""Local-first pub.dev publisher for dx deploy."""

load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error", "api_compat_mode")
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
    values = {
        "hosted_url": ctx.attr.hosted_url,
        "package_name": ctx.attr.package_name,
        "package_rlocs": rlocs,
        "version": ctx.attr.version,
    }
    runfiles = ctx.runfiles(files = package_files).merge(ctx.attr.package[DefaultInfo].default_runfiles)
    if ctx.attr.api_compat != "off":
        package_dir = ""
        if ctx.attr.api_baseline == "git":
            pubspecs = [f for f in package_files if f.basename == "pubspec.yaml"]
            if len(pubspecs) == 0:
                fail("pub_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a pubspec.yaml in package " +
                     str(ctx.attr.package.label))
            pubspec = pubspecs[0]
            if not pubspec.is_source:
                fail("pub_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a checked-in pubspec.yaml, " +
                     "but " + pubspec.short_path + " is generated; use api_baseline = \"registry\"")
            if pubspec.owner.workspace_name != "":
                fail("pub_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a pubspec.yaml in this " +
                     "repository, but " + pubspec.short_path + " comes from an external repository")
            package_dir = pubspec.short_path.rpartition("/")[0]
        dill = [f for f in ctx.files._dart_apitool if f.extension == "dill"]
        values["api_compat"] = ctx.attr.api_compat
        values["api_baseline"] = ctx.attr.api_baseline
        values["apitool_rloc"] = rlocation_path(ctx, dill[0]) if dill else ""
        values["dart_rloc"] = rlocation_path(ctx, ctx.executable._dart)
        values["package_dir"] = package_dir
        runfiles = runfiles.merge_all([
            ctx.runfiles(files = dill),
            ctx.attr._dart[DefaultInfo].default_runfiles,
        ])
    return deploy_config(ctx, "pub", values, runfiles)

_pub_config = rule(
    implementation = _pub_config_impl,
    doc = "Writes the deploy config for one pub.dev deployment.",
    attrs = API_COMPAT_ATTRS | {
        "_dart": attr.label(
            default = Label("//dart/tools:dart"),
            cfg = "exec",
            executable = True,
        ),
        "_dart_apitool": attr.label(
            default = Label("//dart/tools/dart_apitool"),
            cfg = "exec",
        ),
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

def pub_deploy(name, package, version = "0.0.0", hosted_url = PUB_DEFAULT_HOSTED_URL, build_mode = "release", sbom = None, api_compat = "fail", api_baseline = "registry"):
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
    api_error = api_compat_error("pub_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _pub_config(
        name = config_target,
        api_baseline = api_baseline,
        api_compat = api_compat_mode(api_compat),
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
        build_mode = build_mode,
    )
