"""Metadata for native runtime libraries that a consumer archive may ship."""

REDISTRIBUTIONS = ["app_local", "os_supplied", "build_only"]

NativeRuntimeInfo = provider(
    doc = "A native runtime library with its version, source and redistribution rule.",
    fields = {
        "files": "List of File: the runtime files, empty for os_supplied.",
        "name": "String: runtime name.",
        "notice": "File or None: license notice shipped with app_local files.",
        "redistribution": "String: app_local, os_supplied or build_only.",
        "source": "String: where the runtime comes from.",
        "version": "String: runtime version.",
    },
)

def native_runtime_error(version, source, redistribution, file_count, has_notice):
    """Returns the reason a native runtime declaration is invalid, or empty."""
    if version == "":
        return "native_runtime: version must not be empty"
    if source == "":
        return "native_runtime: source must not be empty"
    if redistribution not in REDISTRIBUTIONS:
        return ("native_runtime: unsupported redistribution '" + str(redistribution) +
                "': want one of " + ", ".join(REDISTRIBUTIONS))
    if redistribution == "app_local" and file_count == 0:
        return "native_runtime: app_local needs at least one file in srcs"
    if redistribution == "app_local" and not has_notice:
        return "native_runtime: app_local needs a notice"
    if redistribution == "os_supplied" and file_count != 0:
        return "native_runtime: os_supplied must not ship files"
    return ""

def _native_runtime_impl(ctx):
    files = ctx.files.srcs
    error = native_runtime_error(
        ctx.attr.version,
        ctx.attr.source,
        ctx.attr.redistribution,
        len(files),
        ctx.file.notice != None,
    )
    if error != "":
        fail(error + " (in " + str(ctx.label) + ")")
    return [
        DefaultInfo(files = depset(files)),
        NativeRuntimeInfo(
            name = ctx.label.name,
            version = ctx.attr.version,
            source = ctx.attr.source,
            redistribution = ctx.attr.redistribution,
            files = files,
            notice = ctx.file.notice,
        ),
    ]

native_runtime = rule(
    implementation = _native_runtime_impl,
    doc = "Declares one native runtime library for consumer_archive.",
    attrs = {
        "notice": attr.label(
            doc = "License notice shipped with app_local files.",
            allow_single_file = True,
        ),
        "redistribution": attr.string(
            doc = "app_local ships the files, os_supplied ships nothing, build_only cannot ship.",
            mandatory = True,
        ),
        "source": attr.string(
            doc = "Where the runtime comes from, such as a toolchain or package.",
            mandatory = True,
        ),
        "srcs": attr.label_list(
            doc = "Runtime files. Empty for os_supplied.",
            allow_files = True,
        ),
        "version": attr.string(
            doc = "Runtime version recorded in IDENTITY.txt.",
            mandatory = True,
        ),
    },
)
