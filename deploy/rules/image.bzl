"""Builds an OCI image from one binary on a pinned distroless base."""

load("@rules_oci//oci:defs.bzl", "oci_image", "oci_image_index", "oci_load")
load("@rules_pkg//pkg:mappings.bzl", "pkg_files", "strip_prefix")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")
load(":oci.bzl", "OCI_PLATFORMS", "oci_platform_error")

IMAGE_BASES = {
    "cc": "@distroless_cc",
    "dotnet": "@dotnet_runtime",
    "java": "@distroless_java",
    "node": "@distroless_nodejs",
    "python": "@distroless_python",
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

def image_default_main(base, binary, package):
    """Returns the entry script a python or node binary label implies, or None."""
    ext = {"python": ".py", "node": ".js"}.get(base)
    if not ext:
        return None
    pkg = package
    if binary.startswith("//"):
        label = binary[2:]
        pkg = label.split(":")[0] if ":" in label else label.rpartition("/")[0]
    elif not binary.startswith(":"):
        return None
    name = image_binary_name(binary)
    return (pkg + "/" if pkg else "") + name + ext

def image_options_error(binary, base, platforms, main = None, main_class = None):
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
    if base in ("python", "node", "dotnet") and not main:
        return "oci_image_from_binary: base '" + base + "' needs main, the workspace path of the entry script"
    if base == "java" and not main_class:
        return "oci_image_from_binary: base 'java' needs main_class"
    if base not in ("python", "java", "node", "dotnet") and (main or main_class):
        return "oci_image_from_binary: main and main_class only apply to the python, java, node and dotnet bases"
    return ""

def image_entrypoint(base, app, main = None, main_class = None):
    """Returns the image entrypoint and environment for a base."""
    if base == "python":
        root = "/app/" + app + ".runfiles/_main"
        return ["/usr/bin/python3", root + "/" + main], {"PYTHONPATH": root}
    if base == "node":
        return ["/nodejs/bin/node", "/app/" + app + ".runfiles/_main/" + main], {}
    if base == "dotnet":
        return ["dotnet", "/app/" + app + ".runfiles/_main/" + main], {}
    if base == "java":
        return ["/usr/bin/java", "-cp", "/app/lib/*", main_class], {}
    return ["/app/" + app], {}

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

def _jars_for_platform_impl(ctx):
    runfiles = ctx.attr.binary[0][DefaultInfo].default_runfiles.files.to_list()
    jars = [f for f in runfiles if f.extension == "jar" and f.basename != "jrt-fs.jar"]
    return [DefaultInfo(files = depset(jars))]

_jars_for_platform = rule(
    implementation = _jars_for_platform_impl,
    doc = "Collects the runtime jars of a Java binary built for a single Linux platform.",
    attrs = {
        "binary": attr.label(
            doc = "Java binary built for the platform.",
            cfg = _platform_transition,
            mandatory = True,
        ),
        "platform": attr.label(
            doc = "Bazel platform the binary is built for.",
            mandatory = True,
        ),
    },
)

def oci_image_from_binary(name, binary, base = "static", platforms = ["linux/amd64"], tags = [], main = None, main_class = None):
    """Builds an OCI image holding binary and its runfiles under /app, ready for oci_deploy.

    One platform gives an oci_image and an oci_load target named name + "_load".
    Several platforms give an oci_image_index of one image per platform, and
    name + "_load" loads the image of the first platform. tags apply to the
    image, index and load targets.

    The python, java and node bases run on the distroless interpreter images
    and the dotnet base on the Microsoft runtime image.
    The python, node and dotnet bases need main, the workspace path of the entry script
    or assembly, and run it from the binary's runfiles. python and node default main to
    the binary's package and name with .py or .js. The java base needs main_class and puts the
    binary's runtime jars under /app/lib.
    """
    if not main and type(binary) == "string":
        main = image_default_main(base, binary, native.package_name())
    error = image_options_error(binary, base, platforms, main, main_class)
    if error != "":
        fail(error + " (in " + native.package_name() + ":" + name + ")")
    app = image_binary_name(binary)
    entrypoint, env = image_entrypoint(base, app, main, main_class)
    single = len(platforms) == 1

    images = []
    for platform in platforms:
        suffix = "" if single else "_" + platform.split("/")[1]
        if base == "java":
            _jars_for_platform(
                name = name + "_jars" + suffix,
                binary = binary,
                platform = _PLATFORM_LABELS[platform],
                tags = ["manual"],
            )
            pkg_files(
                name = name + "_files" + suffix,
                srcs = [name + "_jars" + suffix],
                prefix = "/app/lib",
                strip_prefix = strip_prefix.files_only(),
                tags = ["manual"],
            )
            pkg_tar(
                name = name + "_layer" + suffix,
                srcs = [name + "_files" + suffix],
                tags = ["manual"],
            )
        else:
            _for_platform(
                name = name + "_binary" + suffix,
                binary = binary,
                platform = _PLATFORM_LABELS[platform],
                tags = ["manual"],
            )
            pkg_tar(
                name = name + "_layer" + suffix,
                srcs = [name + "_binary" + suffix],
                include_runfiles = True,
                mode = "0755",
                package_dir = "/app",
                tags = ["manual"],
            )
        image = name if single else name + "_image" + suffix
        oci_image(
            name = image,
            base = IMAGE_BASES[base],
            entrypoint = entrypoint,
            env = env,
            tags = tags,
            tars = [name + "_layer" + suffix],
        )
        images.append(image)

    oci_load(
        name = name + "_load",
        image = images[0],
        repo_tags = [name + ":latest"],
        tags = tags,
    )

    if single:
        return

    oci_image_index(
        name = name,
        images = images,
        tags = tags,
    )
