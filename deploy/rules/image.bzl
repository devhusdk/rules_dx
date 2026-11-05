"""Builds an OCI image from one binary on a pinned distroless base."""

load("@rules_oci//oci:defs.bzl", "oci_image", "oci_image_index", "oci_load")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")
load(":oci.bzl", "OCI_PLATFORMS", "oci_platform_error")

IMAGE_BASES = {
    "cc": "@distroless_cc",
    "static": "@distroless_static",
}

_PLATFORM_LABELS = {
    "linux/amd64": "//deploy/rules/platforms:linux_amd64",
    "linux/arm64": "//deploy/rules/platforms:linux_arm64",
}

def image_binary_name(binary):
    """Returns the target name of a binary label."""
    if ":" in binary:
        return binary.rsplit(":", 1)[1]
    return binary.rsplit("/", 1)[-1]

def image_options_error(binary, base, platforms):
    """Validates the oci_image_from_binary arguments."""
    if type(binary) != "string" or binary == "":
        return "oci_image_from_binary: invalid binary '" + str(binary) + "': want a binary target label"
    if base not in IMAGE_BASES:
        return ("oci_image_from_binary: invalid base '" + str(base) +
                "': want one of " + ", ".join(sorted(IMAGE_BASES)))
    if type(platforms) != "list" or len(platforms) == 0:
        return "oci_image_from_binary: want at least one platform, one of " + ", ".join(OCI_PLATFORMS)
    for platform in platforms:
        error = oci_platform_error(platform)
        if error != "":
            return error.replace("oci_deploy", "oci_image_from_binary")
    if len(depset(platforms).to_list()) != len(platforms):
        return "oci_image_from_binary: want each platform once, got " + ", ".join(platforms)
    return ""

def _platform_transition_impl(_settings, attr):
    return {"//command_line_option:platforms": str(attr.platform)}

_platform_transition = transition(
    implementation = _platform_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _for_platform_impl(ctx):
    binary = ctx.attr.binary[0]
    return [DefaultInfo(
        files = binary[DefaultInfo].files,
        runfiles = ctx.runfiles().merge(binary[DefaultInfo].default_runfiles),
    )]

_for_platform = rule(
    implementation = _for_platform_impl,
    doc = "Builds one binary for a single Linux platform.",
    attrs = {
        "binary": attr.label(
            doc = "Binary built for the platform.",
            cfg = _platform_transition,
            mandatory = True,
        ),
        "platform": attr.label(
            doc = "Bazel platform the binary is built for.",
            mandatory = True,
        ),
    },
)

def oci_image_from_binary(name, binary, base = "static", platforms = ["linux/amd64"]):
    """Builds an OCI image holding binary and its runfiles under /app, ready for oci_deploy.

    One platform gives an oci_image and an oci_load target named name + "_load".
    Several platforms give an oci_image_index built once per platform.
    """
    error = image_options_error(binary, base, platforms)
    if error != "":
        fail(error + " (in " + native.package_name() + ":" + name + ")")
    app = image_binary_name(binary)
    single = len(platforms) == 1

    layer_src = binary
    if single:
        layer_src = name + "_binary"
        _for_platform(
            name = layer_src,
            binary = binary,
            platform = _PLATFORM_LABELS[platforms[0]],
            tags = ["manual"],
        )

    pkg_tar(
        name = name + "_layer",
        srcs = [layer_src],
        include_runfiles = True,
        mode = "0755",
        package_dir = "/app",
        tags = ["manual"],
    )

    image = name if single else name + "_image"
    oci_image(
        name = image,
        base = IMAGE_BASES[base],
        entrypoint = ["/app/" + app],
        tars = [name + "_layer"],
    )

    if single:
        oci_load(
            name = name + "_load",
            image = name,
            repo_tags = [name + ":latest"],
        )
        return

    oci_image_index(
        name = name,
        images = [image],
        platforms = [_PLATFORM_LABELS[p] for p in platforms],
    )
