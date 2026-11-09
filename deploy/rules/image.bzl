"""Builds an OCI image from one binary on a pinned distroless base."""

load("@rules_oci//oci:defs.bzl", "oci_image", "oci_image_index", "oci_load")
load("@rules_pkg//pkg:mappings.bzl", "pkg_files", "strip_prefix")
load("@rules_pkg//pkg:providers.bzl", "PackageFilesInfo")
load("@rules_pkg//pkg:tar.bzl", "pkg_tar")
load(":validate_image.bzl", "IMAGE_BASES", "IMAGE_LABEL_PLATFORMS", "IMAGE_PLATFORM_LABELS", "IMAGE_RUNTIMES", "image_options_error")

_RUNTIME_LAYOUT = {
    "dotnet": {"include": ["dotnet", "host/", "shared/Microsoft.NETCore.App/"], "strip": ""},
    "java": {"include": ["bin/", "conf/", "lib/", "release"], "strip": ""},
    "node": {"include": ["bin/"], "strip": "bin/nodejs/"},
    "python": {
        "drop": ["lib/libpython", "lib/libtcl", "lib/libtk", "lib/tcl", "lib/tk", "lib/itcl", "lib/thread", "lib/python3.12/config-", "lib/python3.12/test/"],
        "include": ["bin/python3", "lib/"],
        "strip": "",
    },
}

def image_binary_name(binary):
    """Returns the target name of a binary label."""
    if ":" in binary:
        return binary.rsplit(":", 1)[1]
    return binary.rsplit("/", 1)[-1]

def image_default_main(base, binary, package):
    """Returns the entry script or assembly a python, node or dotnet binary label implies, or None."""
    ext = {"python": ".py", "node": ".js", "dotnet": ".dll"}.get(base)
    if not ext:
        return None
    pkg = package
    if binary.startswith("//"):
        label = binary[2:]
        pkg = label.split(":")[0] if ":" in label else label
    elif not binary.startswith(":"):
        return None
    name = image_binary_name(binary)
    path = (pkg + "/" if pkg else "") + name
    if base == "dotnet":
        return path + "/net10.0/" + name + ext
    return path + ext

def image_entrypoint(base, app, main = None, main_class = None, runtime = False):
    """Returns the image entrypoint and environment for a base.

    With runtime the interpreter is the one the toolchain layer puts under /opt.
    """
    if base == "python":
        root = "/app/" + app + ".runfiles/_main"
        return [("/opt/python" if runtime else "/usr") + "/bin/python3", root + "/" + main], {"PYTHONPATH": root}
    if base == "node":
        return [("/opt/node" if runtime else "/nodejs") + "/bin/node", "/app/" + app + ".runfiles/_main/" + main], {}
    if base == "dotnet":
        if runtime:
            return ["/opt/dotnet/dotnet", "/app/" + app + ".dll.runfiles/_main/" + main], {
                "DOTNET_ROOT": "/opt/dotnet",
                "DOTNET_SYSTEM_GLOBALIZATION_INVARIANT": "1",
            }
        return ["dotnet", "/app/" + app + ".dll.runfiles/_main/" + main], {}
    if base == "java":
        return [("/opt/java" if runtime else "/usr") + "/bin/java", "-cp", "/app/lib/*", main_class or "@/app/lib/main.args"], {}
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

def image_runtime_dest(path, layout, prefix):
    """Returns where a toolchain file lands in the image, or None when the layer leaves it out.

    path is relative to the toolchain's repository.
    """
    if not path.startswith(layout["strip"]):
        return None
    path = path[len(layout["strip"]):]
    for drop in layout.get("drop", []):
        if path.startswith(drop):
            return None
    for keep in layout["include"]:
        if path == keep or (keep.endswith("/") and path.startswith(keep)):
            return prefix + "/" + path
    return None

def _runtime_files_impl(ctx):
    layout = {"drop": ctx.attr.drop, "include": ctx.attr.include, "strip": ctx.attr.strip_prefix}
    dest_src_map = {}
    for target in ctx.attr.srcs:
        files = depset(transitive = [target[DefaultInfo].files, target[DefaultInfo].default_runfiles.files])
        for f in files.to_list():
            parts = f.short_path.split("/")
            if parts[0] != "..":
                continue
            dest = image_runtime_dest("/".join(parts[2:]), layout, ctx.attr.prefix)
            if dest:
                dest_src_map[dest] = f
    return [
        PackageFilesInfo(attributes = {"mode": "0755"}, dest_src_map = dest_src_map),
        DefaultInfo(files = depset(dest_src_map.values())),
    ]

_runtime_files = rule(
    implementation = _runtime_files_impl,
    doc = "Maps the files of a toolchain repository to paths under prefix in an image.",
    attrs = {
        "drop": attr.string_list(doc = "Path prefixes left out."),
        "include": attr.string_list(doc = "Paths kept; a trailing / keeps a directory."),
        "prefix": attr.string(doc = "Directory in the image.", mandatory = True),
        "srcs": attr.label_list(doc = "Targets of the toolchain repository.", allow_files = True, mandatory = True),
        "strip_prefix": attr.string(doc = "Leading path removed before matching."),
    },
)

MainClassInfo = provider(
    doc = "The main class of a Java binary.",
    fields = {"main_class": "Main class name, or empty."},
)

def _main_class_aspect_impl(target, ctx):
    main_class = getattr(ctx.rule.attr, "main_class", "")
    if main_class:
        return [MainClassInfo(main_class = main_class)]
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if upstream != None:
        upstream = upstream[0] if type(upstream) == "list" else upstream
        if MainClassInfo in upstream:
            return [upstream[MainClassInfo]]
    return [MainClassInfo(main_class = "")]

_main_class_aspect = aspect(
    implementation = _main_class_aspect_impl,
    attr_aspects = ["upstream"],
)

def _jars_for_platform_impl(ctx):
    binary = ctx.attr.binary[0]
    runfiles = binary[DefaultInfo].default_runfiles.files.to_list()
    files = [f for f in runfiles if f.extension == "jar" and f.basename != "jrt-fs.jar"]
    if ctx.attr.read_main_class:
        main_class = binary[MainClassInfo].main_class
        if not main_class:
            fail(ctx.label.name + ": " + str(binary.label) + " has no main_class; set main_class on oci_image_from_binary")
        argfile = ctx.actions.declare_file(ctx.label.name + "/main.args")
        ctx.actions.write(argfile, main_class + "\n")
        files.append(argfile)
    return [DefaultInfo(files = depset(files))]

_jars_for_platform = rule(
    implementation = _jars_for_platform_impl,
    doc = "Collects the runtime jars of a Java binary built for a single Linux platform.",
    attrs = {
        "binary": attr.label(
            doc = "Java binary built for the platform.",
            aspects = [_main_class_aspect],
            cfg = _platform_transition,
            mandatory = True,
        ),
        "read_main_class": attr.bool(
            doc = "Whether to write the binary's main class to main.args next to the jars.",
        ),
        "platform": attr.label(
            doc = "Bazel platform the binary is built for.",
            mandatory = True,
        ),
    },
)

def oci_image_from_binary(name, binary, base = "static", platforms = ["@rules_dx//deploy/rules/platforms:linux_amd64"], tags = [], main = None, main_class = None, runtime = False, visibility = None, testonly = None):
    """Builds an OCI image holding binary and its runfiles under /app, ready for oci_deploy.

    One platform gives an oci_image and an oci_load target named name + "_load".
    Several platforms give an oci_image_index of one image per platform, and
    name + "_load" loads the image of the first platform. tags apply to the
    image, index and load targets.

    The python, java and node bases run on the distroless interpreter images
    and the dotnet base on the Microsoft runtime image.
    The python, node and dotnet bases need main, the workspace path of the entry script
    or assembly, and run it from the binary's runfiles. python, node and dotnet default main to
    the binary's package and name with .py or .js, and a dotnet binary to its net10.0 assembly. The java base puts the binary's runtime jars under /app/lib and
    runs main_class, which defaults to the main class of the java_binary.

    With runtime = True the python, java, node or dotnet base is replaced by the distroless cc image
    and a layer from the managed toolchain for each platform, installed under /opt, so the image
    runs the interpreter the build used instead of the one in the base image. The dotnet runtime
    runs with invariant globalization.
    """
    if not main and type(binary) == "string":
        main = image_default_main(base, binary, native.package_name())
    if type(platforms) == "list":
        platforms = [str(native.package_relative_label(platform)) for platform in platforms]
    error = image_options_error(binary, base, platforms, main, main_class, runtime)
    if error != "":
        fail(error + " (in " + native.package_name() + ":" + name + ")")
    app = image_binary_name(binary)
    entrypoint, env = image_entrypoint(base, app, main, main_class, runtime)
    single = len(platforms) == 1

    images = []
    for label in platforms:
        platform = IMAGE_LABEL_PLATFORMS[label]
        suffix = "" if single else "_" + platform.split("/")[1]
        if base == "java":
            _jars_for_platform(
                name = name + "_jars" + suffix,
                binary = binary,
                platform = label,
                read_main_class = not main_class,
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
                platform = label,
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
        layers = [name + "_layer" + suffix]
        if runtime:
            layout = _RUNTIME_LAYOUT[base]
            _runtime_files(
                name = name + "_runtime_files" + suffix,
                srcs = [str(Label(IMAGE_RUNTIMES[base][platform]))],
                drop = layout.get("drop", []),
                include = layout["include"],
                prefix = "/opt/" + base,
                strip_prefix = layout["strip"],
                tags = ["manual"],
            )
            pkg_tar(
                name = name + "_runtime" + suffix,
                srcs = [name + "_runtime_files" + suffix],
                tags = ["manual"],
            )
            layers = [name + "_runtime" + suffix] + layers
        image = name if single else name + "_image" + suffix
        oci_image(
            name = image,
            base = IMAGE_BASES["cc" if runtime else base],
            entrypoint = entrypoint,
            env = env,
            tags = tags,
            tars = layers,
            testonly = testonly,
            visibility = visibility if single else None,
        )
        images.append(image)

    oci_load(
        name = name + "_load",
        image = images[0],
        repo_tags = [name + ":latest"],
        tags = tags,
        testonly = testonly,
        visibility = visibility,
    )

    if single:
        return

    oci_image_index(
        name = name,
        images = images,
        tags = tags,
        testonly = testonly,
        visibility = visibility,
    )
