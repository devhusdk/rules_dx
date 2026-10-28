"""BCR submission tooling for rules_dx."""

load("//deploy/release:tool.bzl", "RELEASE_DEPLOY_TOOL")
load("//deploy/rules:config.bzl", "deploy_config", "rlocation_path")
load("//deploy/rules:defs.bzl", "dx_deployment")
load("//deploy/rules:validate.bzl", "genrule_arg")

def bcr_source_error(module_name, version):
    """Validates the BCR module name + version pair."""
    if module_name != "rules_dx":
        return ("bcr: invalid module '" + str(module_name) +
                "': want 'rules_dx'")
    if type(version) != "string" or version == "":
        return ("bcr: invalid version '" + str(version) +
                "': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')")
    parts = version.split(".")
    if len(parts) != 3:
        return ("bcr: invalid version '" + version +
                "': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')")
    for part in parts:
        if part == "" or not part[0].isdigit():
            return ("bcr: invalid version '" + version +
                    "': want SemVer MAJOR.MINOR.PATCH (for example '1.0.0')")
    return ""

def _bcr_config_impl(ctx):
    """Writes the owner-gated BCR deploy config."""
    rlocs = []
    files = []
    runfiles = ctx.runfiles()
    for target in ctx.attr.inputs:
        info = target[DefaultInfo]
        fl = info.files.to_list()
        if len(fl) != 1:
            fail("bcr_check " + str(ctx.label) + ": input " +
                 str(target.label) + " provides " + str(len(fl)) +
                 " files, want exactly one")
        rlocs.append(rlocation_path(ctx, fl[0]))
        files.append(fl[0])
        runfiles = runfiles.merge(info.default_runfiles)
    return deploy_config(
        ctx,
        "bcr",
        {
            "input_rlocs": rlocs,
            "module": ctx.attr.module_name,
            "version": ctx.attr.version,
        },
        runfiles.merge(ctx.runfiles(files = files)),
    )

_bcr_config = rule(
    implementation = _bcr_config_impl,
    doc = "Writes the owner-gated BCR deploy config.",
    attrs = {
        "inputs": attr.label_list(
            doc = "Targets whose single output file is submitted with the module.",
            mandatory = True,
        ),
        "module_name": attr.string(
            doc = "Bazel module name being submitted, which must be rules_dx.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Dotted numeric version of the module being submitted.",
            mandatory = True,
        ),
    },
)

def bcr_check(name, module_name = "rules_dx", version = "0.0.0", inputs = [], profile = "release"):
    """Creates an owner-gated BCR shape-check deploy target."""
    src_err = bcr_source_error(module_name, version)
    if src_err != "":
        fail(src_err + " (in " + native.package_name() + ":" + name + ")")

    native.genrule(
        name = name + "_source",
        outs = [name + ".source.json"],
        tools = ["//deploy/release:release_tool"],
        cmd = "$(location //deploy/release:release_tool) bcr-source $(OUTS) " + genrule_arg(module_name) + " " + genrule_arg(version),
    )

    config_target = name + "_config"
    _bcr_config(
        name = config_target,
        inputs = [":" + name + "_source"] + inputs,
        module_name = module_name,
        version = version,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = RELEASE_DEPLOY_TOOL,
        profile = profile,
    )
