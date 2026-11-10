"""Metadata for native runtime libraries that a consumer archive may ship."""

load(":validate_native_runtime.bzl", "native_runtime_error")

NativeRuntimeInfo = provider(
    doc = "A native runtime library with its version, source and redistribution rule.",
    fields = {
        "files": "List of File: the runtime files, empty for os_supplied.",
        "name": "String: runtime name.",
        "notice": "File or None: license notice shipped with app_local files.",
        "redistribution": "String: app_local, os_supplied or build_only.",
        "provides": "List of String: library names the OS supplies for this runtime; a trailing * is a prefix.",
        "source": "String: where the runtime comes from.",
        "version": "String: runtime version.",
    },
)

def _native_runtime_impl(ctx):
    files = ctx.files.srcs
    error = native_runtime_error(
        ctx.attr.version,
        ctx.attr.source,
        ctx.attr.redistribution,
        len(files),
        ctx.file.notice != None,
        len(ctx.attr.provides),
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
            provides = ctx.attr.provides,
        ),
        OutputGroupInfo(notice = depset([ctx.file.notice]) if ctx.file.notice else depset()),
    ]

dx_native_runtime = rule(
    implementation = _native_runtime_impl,
    doc = "Declares one native runtime library for consumer_archive.",
    attrs = {
        "notice": attr.label(
            doc = "License notice shipped with app_local files.",
            allow_single_file = True,
        ),
        "provides": attr.string_list(
            doc = "os_supplied only: imported library names the OS supplies; a trailing * matches a prefix.",
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
