"""Hermetic npm pack feed publisher for dx deploy."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")

_VALID_TAG_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"
_VALID_PACKAGE_CHARS = _VALID_TAG_CHARS + "@/"

def npm_tag_error(tag):
    """Validates one npm dist-tag value."""
    if type(tag) != "string" or tag == "":
        return ("npm_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'latest')")
    for c in tag.elems():
        if c not in _VALID_TAG_CHARS:
            return ("npm_deploy: invalid tag '" + tag +
                    "': want only [A-Za-z0-9._-] so the tag embeds " +
                    "safely in the deploy program")
    return ""

def npm_package_error(package):
    """Validates one npm package name value."""
    if type(package) != "string" or package == "":
        return ("npm_deploy: invalid package '" + str(package) +
                "': want a non-empty package name (for example 'npm-demo')")
    for c in package.elems():
        if c not in _VALID_PACKAGE_CHARS:
            return ("npm_deploy: invalid package '" + package +
                    "': want only [A-Za-z0-9@/_.-] so the package name " +
                    "embeds safely in the deploy program")
    return ""

def npm_registry_error(registry):
    """Validates one npm registry URL value."""
    if type(registry) != "string" or not registry.startswith("https://"):
        return ("npm_deploy: invalid registry '" + str(registry) +
                "': want an https:// URL " +
                "(for example 'https://registry.npmjs.org')")
    for c in registry.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("npm_deploy: invalid registry '" + registry +
                    "': want an https:// URL " +
                    "(for example 'https://registry.npmjs.org')")
    return ""

def npm_filenames(name):
    """Returns the deterministic (tarball, feed) output names."""
    return (name + ".tgz", name + ".feed.json")

def _npm_launcher_impl(ctx):
    """Expands the rust_binary launcher for one npm deployment."""
    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "fn main() { std::process::exit(dx_deploy_tools::npm_deploy_main(&std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_npm_launcher = rule(
    implementation = _npm_launcher_impl,
    attrs = {},
)

def npm_deploy(name, package, tag = "latest", srcs = [], registry = "https://registry.npmjs.org", profile = "release"):
    """Publishes packed files to a local npm folder feed."""
    pkg_error = npm_package_error(package)
    if pkg_error != "":
        fail(pkg_error + " (in " + native.package_name() + ":" + name + ")")
    tag_error = npm_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = npm_registry_error(registry)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")
    if len(srcs) == 0:
        fail("npm_deploy " + native.package_name() + ":" + name +
             ": need at least one src")

    (tarball, feed) = npm_filenames(name)
    pack_target = name + "_pack"
    program_target = name + "_program"
    launcher_target = program_target + "_launcher"

    native.genrule(
        name = pack_target,
        srcs = srcs,
        outs = [tarball, feed],
        tools = ["//deploy/rules:npm_packer"],
        cmd = "$(location //deploy/rules:npm_packer) " + shell.quote(package) + " " + shell.quote(tag) + " " + shell.quote(registry) + " $(OUTS) $(SRCS)",
    )

    _npm_launcher(
        name = launcher_target,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [":" + pack_target],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
