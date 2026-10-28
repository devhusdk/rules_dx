"""Local-first crates.io publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "ALNUM", "VERSION_CHARS", "only_chars")

_VALID_NAME_CHARS = ALNUM + "_-"

_VALID_VERSION_CHARS = VERSION_CHARS

def crates_name_error(crate_name):
    """Validates one crate name value."""
    if type(crate_name) != "string" or crate_name == "":
        return ("crates_deploy: invalid crate name '" + str(crate_name) +
                "': want a non-empty name (for example 'crates_demo')")
    if not only_chars(crate_name, _VALID_NAME_CHARS):
        return ("crates_deploy: invalid crate name '" + crate_name +
                "': want only [A-Za-z0-9_-]")
    return ""

def crates_version_error(version):
    """Validates one crate version value."""
    if type(version) != "string" or version == "":
        return ("crates_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '0.0.0')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("crates_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
    return ""

def _crates_config_impl(ctx):
    """Writes the deploy config for one crates.io deployment."""
    crate_files = ctx.attr.crate[DefaultInfo].files.to_list()
    if len(crate_files) == 0:
        fail("crates_deploy " + str(ctx.label) + ": crate " +
             str(ctx.attr.crate.label) + " provides no files, want at least one source file")
    rlocs = [rlocation_path(ctx, f) for f in crate_files]

    return deploy_config(
        ctx,
        "crates",
        {
            "crate_name": ctx.attr.crate_name,
            "crate_rlocs": rlocs,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = crate_files).merge(ctx.attr.crate[DefaultInfo].default_runfiles),
    )

_crates_config = rule(
    implementation = _crates_config_impl,
    doc = "Writes the deploy config for one crates.io deployment.",
    attrs = {
        "crate": attr.label(
            doc = "Crate target whose files are published by the deployment.",
            allow_files = True,
            mandatory = True,
        ),
        "crate_name": attr.string(
            doc = "Name the crate is published under on crates.io.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Crate version published by the deployment.",
            mandatory = True,
        ),
    },
)

def crates_deploy(name, crate, version = "0.0.0", profile = "release"):
    """Publishes staged crate sources as a local-first crates.io deployment."""
    name_error = crates_name_error(name)
    if name_error != "":
        fail(name_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = crates_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _crates_config(
        name = config_target,
        crate = crate,
        crate_name = name,
        version = version,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
