"""Hermetic npm pack feed publisher for dx deploy."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "only_chars")

_NPM_PACKER = Label("//deploy/rules:npm_packer")

_VALID_TAG_CHARS = NAME_CHARS
_VALID_PACKAGE_CHARS = _VALID_TAG_CHARS + "@/"

def npm_tag_error(tag):
    """Validates one npm dist-tag value."""
    if type(tag) != "string" or tag == "":
        return ("npm_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'latest')")
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("npm_deploy: invalid tag '" + tag +
                "': want only [A-Za-z0-9._-]")
    return ""

def npm_package_error(package):
    """Validates one npm package name value."""
    if type(package) != "string" or package == "":
        return ("npm_deploy: invalid package '" + str(package) +
                "': want a non-empty package name (for example 'npm-demo')")
    if not only_chars(package, _VALID_PACKAGE_CHARS):
        return ("npm_deploy: invalid package '" + package +
                "': want only [A-Za-z0-9@/_.-]")
    return ""

def npm_registry_error(registry):
    """Validates one npm registry URL value."""
    if type(registry) != "string" or not registry.startswith("https://"):
        return ("npm_deploy: invalid registry '" + str(registry) +
                "': want an https:// URL " +
                "(for example 'https://registry.npmjs.org')")
    return ""

def npm_access_error(package, access):
    """Validates one npm access value against the package scope."""
    if access not in ["public", "restricted"]:
        return ("npm_deploy: invalid access '" + str(access) +
                "': want 'public' or 'restricted'")
    scoped = package.startswith("@")
    if scoped and (package.count("/") != 1 or package.startswith("@/") or package.endswith("/")):
        return ("npm_deploy: invalid package '" + package +
                "': a scoped name is @scope/name")
    if not scoped and "/" in package:
        return ("npm_deploy: invalid package '" + package +
                "': only scoped names contain '/'")
    if access == "restricted" and not scoped:
        return ("npm_deploy: access 'restricted' needs a scoped package, " +
                "got unscoped '" + package + "'")
    return ""

def npm_provenance_error(access, registry, provenance):
    """Validates one npm provenance policy against access and registry."""
    if provenance not in ["required", "none"]:
        return ("npm_deploy: invalid provenance '" + str(provenance) +
                "': want 'required' or 'none'")
    if provenance == "required" and access != "public":
        return ("npm_deploy: provenance 'required' is not supported with access '" +
                access + "'; use access 'public' or provenance 'none'")
    if provenance == "required" and registry.removeprefix("https://").split("/")[0] != "registry.npmjs.org":
        return ("npm_deploy: provenance 'required' is not supported by registry '" +
                registry + "'; use https://registry.npmjs.org or provenance 'none'")
    return ""

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
            "feed_rloc": rlocation_path(ctx, ctx.file.feed),
            "tarball_rloc": rlocation_path(ctx, ctx.file.tarball),
        },
        ctx.runfiles(files = [ctx.file.feed, ctx.file.tarball]),
    )

_npm_config = rule(
    implementation = _npm_config_impl,
    doc = "Writes the deploy config for one npm deployment.",
    attrs = {
        "feed": attr.label(allow_single_file = True, mandatory = True, doc = "Packed feed JSON."),
        "tarball": attr.label(allow_single_file = True, mandatory = True, doc = "Packed tarball."),
    },
)

def npm_deploy(name, package, tag = "latest", srcs = [], registry = "https://registry.npmjs.org", access = "public", provenance = "", profile = "release"):
    """Packs files into a local npm feed with public or restricted access and a provenance policy."""
    pkg_error = npm_package_error(package)
    if pkg_error != "":
        fail(pkg_error + " (in " + native.package_name() + ":" + name + ")")
    tag_error = npm_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = npm_registry_error(registry)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")
    access_error = npm_access_error(package, access)
    if access_error != "":
        fail(access_error + " (in " + native.package_name() + ":" + name + ")")
    provenance = npm_effective_provenance(access, provenance)
    provenance_error = npm_provenance_error(access, registry, provenance)
    if provenance_error != "":
        fail(provenance_error + " (in " + native.package_name() + ":" + name + ")")
    if len(srcs) == 0:
        fail("npm_deploy " + native.package_name() + ":" + name +
             ": need at least one src")

    (tarball, feed) = npm_filenames(name)
    pack_target = name + "_pack"
    config_target = name + "_config"

    native.genrule(
        name = pack_target,
        srcs = srcs,
        outs = [tarball, feed],
        tools = [_NPM_PACKER],
        cmd = "$(location " + str(_NPM_PACKER) + ") " + shell.quote(package) + " " + shell.quote(tag) + " " + shell.quote(registry) + " " + shell.quote(access) + " " + shell.quote(provenance) + " $(OUTS) $(SRCS)",
    )

    _npm_config(
        name = config_target,
        feed = ":" + feed,
        tarball = ":" + tarball,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
