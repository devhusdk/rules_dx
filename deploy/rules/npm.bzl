"""Hermetic npm pack feed publisher for dx deploy."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deploy_identity", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_npm.bzl", "npm_access_error", "npm_package_error", "npm_provenance_error", "npm_registry_url_error", "npm_tag_error")

_NPM_PACKER = Label("//deploy/rules:npm_packer")

def npm_effective_provenance(access, provenance):
    """Resolves the default provenance policy for an access mode."""
    if provenance == "":
        return "required" if access == "public" else "none"
    return provenance

def npm_filenames(name):
    """Returns the deterministic (tarball, feed) output names."""
    return (name + ".tgz", name + ".feed.json")

def _npm_config_impl(ctx):
    """Writes the deploy config for one npm deployment."""
    return deploy_config(
        ctx,
        "npm",
        {
            "api_baseline": ctx.attr.api_baseline,
            "api_compat": ctx.attr.api_compat,
            "api_extractor_rloc": rlocation_path(ctx, ctx.executable._api_extractor),
            "feed_rloc": rlocation_path(ctx, ctx.file.feed),
            "tarball_rloc": rlocation_path(ctx, ctx.file.tarball),
        },
        ctx.runfiles(files = [ctx.file.feed, ctx.file.tarball]).merge(ctx.attr._api_extractor[DefaultInfo].default_runfiles),
    )

_npm_config = rule(
    implementation = _npm_config_impl,
    doc = "Writes the deploy config for one npm deployment.",
    attrs = API_COMPAT_ATTRS | {
        "_api_extractor": attr.label(
            default = Label("//quality/tools/javascript/bin:api_extractor"),
            executable = True,
            cfg = "exec",
        ),
        "feed": attr.label(allow_single_file = True, mandatory = True, doc = "Packed feed JSON."),
        "tarball": attr.label(allow_single_file = True, mandatory = True, doc = "Packed tarball."),
    },
)

def npm_deploy(name, package = "", dist_tag = "latest", srcs = [], registry_url = "https://registry.npmjs.org", access = "public", provenance = "", build_mode = "release", sbom = None, api_compat = "fail", api_baseline = "registry", size_budget = "", tags = [], visibility = None, testonly = False, target_compatible_with = []):
    """Packs files into a local npm feed with public or restricted access and a provenance policy. api_compat and api_baseline work as described under "Public API compatibility" in the README; the tool is api-extractor on the types entry."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    package, _ = deploy_identity(name, package, "", npm_package_error, npm_package_error)
    tag_error = npm_tag_error(dist_tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    registry_url_error = npm_registry_url_error(registry_url)
    if registry_url_error != "":
        fail(registry_url_error + " (in " + native.package_name() + ":" + name + ")")
    access_error = npm_access_error(package, access)
    if access_error != "":
        fail(access_error + " (in " + native.package_name() + ":" + name + ")")
    provenance = npm_effective_provenance(access, provenance)
    provenance_error = npm_provenance_error(access, registry_url, provenance)
    if provenance_error != "":
        fail(provenance_error + " (in " + native.package_name() + ":" + name + ")")
    if len(srcs) == 0:
        fail("npm_deploy " + native.package_name() + ":" + name +
             ": need at least one src")

    api_error = api_compat_error("npm_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    (tarball, feed) = npm_filenames(name)
    pack_target = name + "_pack"
    config_target = name + "_config"

    native.genrule(
        name = pack_target,
        srcs = srcs,
        outs = [tarball, feed],
        tools = [_NPM_PACKER],
        cmd = "$(location " + str(_NPM_PACKER) + ") " + shell.quote(package) + " " + shell.quote(dist_tag) + " " + shell.quote(registry_url) + " " + shell.quote(access) + " " + shell.quote(provenance) + " $(OUTS) $(SRCS)",
        **attrs
    )

    _npm_config(
        name = config_target,
        feed = ":" + feed,
        tarball = ":" + tarball,
        api_compat = api_compat,
        api_baseline = api_baseline,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [":" + tarball]) | attrs)
    )
