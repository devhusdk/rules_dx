"""Local-first crates.io publisher for dx deploy."""

load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":crates_validate.bzl", "crates_name_error", "crates_version_error")
load(":defs.bzl", "dx_deployment")

def _crates_config_impl(ctx):
    """Writes the deploy config for one crates.io deployment."""
    crate_files = ctx.attr.crate[DefaultInfo].files.to_list()
    if len(crate_files) == 0:
        fail("crates_deploy " + str(ctx.label) + ": crate " +
             str(ctx.attr.crate.label) + " provides no files, want at least one source file")
    rlocs = [rlocation_path(ctx, f) for f in crate_files]
    semver_files = ctx.attr._semver_checks[DefaultInfo].files.to_list()
    semver_rloc = rlocation_path(ctx, semver_files[0]) if semver_files else ""

    crate_dir = ""
    if ctx.attr.api_baseline == "git" and ctx.attr.api_compat != "off":
        manifests = [f for f in crate_files if f.basename == "Cargo.toml"]
        if len(manifests) == 0:
            fail("crates_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a Cargo.toml in crate " +
                 str(ctx.attr.crate.label))
        manifest = manifests[0]
        if not manifest.is_source:
            fail("crates_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a checked-in Cargo.toml, " +
                 "but " + manifest.short_path + " is generated; use api_baseline = \"registry\"")
        if manifest.owner.workspace_name != "":
            fail("crates_deploy " + str(ctx.label) + ": api_baseline = \"git\" needs a Cargo.toml in this " +
                 "repository, but " + manifest.short_path + " comes from an external repository")
        crate_dir = manifest.short_path.rpartition("/")[0]

    return deploy_config(
        ctx,
        "crates",
        {
            "crate_dir": crate_dir,
            "crate_name": ctx.attr.crate_name,
            "api_baseline": ctx.attr.api_baseline,
            "api_compat": ctx.attr.api_compat,
            "crate_rlocs": rlocs,
            "semver_checks_rloc": semver_rloc,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = crate_files + semver_files).merge(ctx.attr.crate[DefaultInfo].default_runfiles),
    )

_crates_config = rule(
    implementation = _crates_config_impl,
    doc = "Writes the deploy config for one crates.io deployment.",
    attrs = API_COMPAT_ATTRS | {
        "crate": attr.label(
            doc = "Crate target whose files are published by the deployment.",
            allow_files = True,
            mandatory = True,
        ),
        "_semver_checks": attr.label(
            default = Label("//deploy/rules/pins:cargo_semver_checks"),
            allow_files = True,
        ),
        "crate_name": attr.string(
            doc = "Name the crate is published under on crates.io.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Crate version the manifest must declare, empty to publish the manifest's version.",
        ),
    },
)

def crates_deploy(name, crate, package = "", version = "", build_mode = "release", api_compat = "fail", api_baseline = "registry", tags = [], sbom = None, size_budget = ""):
    """Publishes staged crate sources as a local-first crates.io deployment.

    api_compat and api_baseline work as described under "Public API
    compatibility" in the README; the tool is cargo-semver-checks, and the
    git baseline needs a checked-in, standalone Cargo.toml.
    """
    pkg, version = deploy_identity(name, package, version, crates_name_error, crates_version_error)

    api_error = api_compat_error("crates_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    config_target = name + "_config"
    _crates_config(
        name = config_target,
        crate = crate,
        crate_name = pkg,
        version = version,
        api_compat = api_compat,
        api_baseline = api_baseline,
        tags = tags,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        tags = tags,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **deployment_budget(name, size_budget, [crate]),
    )
