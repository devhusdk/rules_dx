"""Local-first Packagist updater for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_packagist.bzl", "packagist_composer_error", "packagist_name_error", "packagist_repository_error")

def _packagist_config_impl(ctx):
    """Writes the deploy config for one Packagist deployment."""
    composer_file = ctx.file.composer_json
    composer_error = packagist_composer_error(composer_file.basename)
    if composer_error != "":
        fail(composer_error + " (in " + str(ctx.label) + ")")

    return deploy_config(
        ctx,
        "packagist",
        {
            "composer_rloc": rlocation_path(ctx, composer_file),
            "deploy_name": ctx.attr.deploy_name,
            "package": ctx.attr.package,
            "repository_url": ctx.attr.repository_url,
        },
        ctx.runfiles(files = [composer_file]),
    )

_packagist_config = rule(
    implementation = _packagist_config_impl,
    doc = "Writes the deploy config for one Packagist deployment.",
    attrs = {
        "composer_json": attr.label(
            doc = "The composer.json whose name must equal package.",
            allow_single_file = True,
            mandatory = True,
        ),
        "deploy_name": attr.string(doc = "Deploy target name.", mandatory = True),
        "package": attr.string(doc = "Package name vendor/name.", mandatory = True),
        "repository_url": attr.string(doc = "Repository URL registered on Packagist.", mandatory = True),
    },
)

def packagist_deploy(name, package, composer_json, repository_url, build_mode = "release", sbom = None, size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Asks Packagist to update one package from its git repository after the release tag is pushed."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    where = " (in " + native.package_name() + ":" + name + ")"
    for error in [packagist_name_error(package), packagist_repository_error(repository_url)]:
        if error != "":
            fail(error + where)

    config_target = name + "_config"
    _packagist_config(
        name = config_target,
        composer_json = composer_json,
        deploy_name = name,
        package = package,
        repository_url = repository_url,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [composer_json]) | attrs)
    )
