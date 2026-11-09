"""Local-first pub.dev publisher for dx deploy."""

load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "deploy_identity", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "LOWER_ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = LOWER_ALNUM + "_"

_VALID_VERSION_CHARS = VERSION_CHARS

_FLUTTER_TOOLCHAIN = "@rules_flutter//flutter:toolchain_type"

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
    package_files = ctx.files.srcs
    if len(package_files) == 0:
        fail("pub_deploy " + str(ctx.label) + ": srcs provide no files, want at least pubspec.yaml")
    rlocs = [rlocation_path(ctx, f) for f in package_files]
    values = {
        "hosted_url": ctx.attr.hosted_url,
        "package_name": ctx.attr.package_name,
        "package_rlocs": rlocs,
        "version": ctx.attr.version,
    }
    runfiles = ctx.runfiles(files = package_files).merge_all([src[DefaultInfo].default_runfiles for src in ctx.attr.srcs])
    if ctx.attr.api_compat != "off":
        package_dir = ""
        if ctx.attr.api_baseline == "git":
            pubspecs = [f for f in package_files if f.basename == "pubspec.yaml"]
            if len(pubspecs) == 0:
                fail("pub_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a pubspec.yaml in srcs of " +
                     str(ctx.label))
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
        flutter = ctx.toolchains[_FLUTTER_TOOLCHAIN]
        if flutter:
            sdk = flutter.flutter_sdk_info
            values["flutter_dart_rloc"] = rlocation_path(ctx, sdk.dart)
            values["flutter_root_rloc"] = rlocation_path(ctx, ctx.file._flutter_version_manifest)
            runfiles = runfiles.merge(ctx.runfiles(
                files = [ctx.file._flutter_version_manifest],
                transitive_files = depset(
                    ctx.files._flutter_root,
                    transitive = [sdk.tool_files],
                ),
            ))
    return deploy_config(ctx, "pub", values, runfiles)

_pub_config = rule(
    implementation = _pub_config_impl,
    doc = "Writes the deploy config for one pub.dev deployment.",
    toolchains = [config_common.toolchain_type(_FLUTTER_TOOLCHAIN, mandatory = False)],
    attrs = API_COMPAT_ATTRS | {
        "_flutter_root": attr.label(
            default = Label("@flutter_dev_root//:dev_root"),
            cfg = "exec",
        ),
        "_flutter_version_manifest": attr.label(
            default = Label("@flutter_dev_root//:version_manifest"),
            cfg = "exec",
            allow_single_file = True,
        ),
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
        "srcs": attr.label_list(
            doc = "Package targets whose files are published by the deployment.",
            allow_files = True,
            mandatory = True,
        ),
        "package_name": attr.string(
            doc = "Name the package is published under, taken from the deployment target name.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Package version the manifest must declare, empty to publish the manifest's version.",
        ),
    },
)

def pub_deploy(name, srcs, package = "", version = "", hosted_url = PUB_DEFAULT_HOSTED_URL, build_mode = "release", sbom = None, api_compat = "fail", api_baseline = "registry"):
    """Publishes a Dart or Flutter package as a local-first pub.dev deployment."""
    pkg, version = deploy_identity(name, package, version, pub_name_error, pub_version_error)
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
        api_compat = api_compat,
        hosted_url = hosted_url,
        srcs = srcs,
        package_name = pkg,
        version = version,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
    )
