"""Local-first opam-repository pull request opener for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_opam.bzl", "opam_file_error", "opam_name_error", "opam_version_error")

def _opam_config_impl(ctx):
    """Writes the deploy config for one opam deployment."""
    opam_file = ctx.file.opam_file
    file_error = opam_file_error(opam_file.basename)
    if file_error != "":
        fail(file_error + " (in " + str(ctx.label) + ")")

    opam_files = ctx.attr.opam[DefaultInfo].files.to_list()
    return deploy_config(
        ctx,
        "opam",
        {
            "deploy_name": ctx.attr.deploy_name,
            "opam_rloc": rlocation_path(ctx, opam_files[0]) if opam_files else "",
            "opam_file_rloc": rlocation_path(ctx, opam_file),
            "package": ctx.attr.package,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = opam_files + [opam_file]),
    )

_opam_config = rule(
    implementation = _opam_config_impl,
    doc = "Writes the deploy config for one opam deployment.",
    attrs = {
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "opam": attr.label(
            doc = "Pinned opam executable.",
            allow_files = True,
            cfg = "exec",
            mandatory = True,
        ),
        "opam_file": attr.label(
            doc = "The opam file to submit.",
            allow_single_file = True,
            mandatory = True,
        ),
        "package": attr.string(doc = "Package name.", mandatory = True),
        "version": attr.string(doc = "Package version.", mandatory = True),
    },
)

def opam_deploy(name, package, version, opam_file, build_mode = "release", sbom = None, size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Opens a pull request to ocaml/opam-repository that adds one package version."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    where = " (in " + native.package_name() + ":" + name + ")"
    for error in [opam_name_error(package), opam_version_error(version)]:
        if error != "":
            fail(error + where)

    config_target = name + "_config"
    _opam_config(
        name = config_target,
        deploy_name = name,
        opam = str(Label("//deploy/private/pins:opam")),
        opam_file = opam_file,
        package = package,
        version = version,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [opam_file]) | attrs)
    )
