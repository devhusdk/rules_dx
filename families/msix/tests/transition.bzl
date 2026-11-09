"""Builds dx_msix targets for another Windows cell or with extra copts."""

def _variant_impl(settings, attr):
    result = {
        "//command_line_option:copt": settings["//command_line_option:copt"] + attr.copts,
        "//command_line_option:platforms": settings["//command_line_option:platforms"],
    }
    if attr.platform:
        result["//command_line_option:platforms"] = [str(attr.platform)]
    return result

_variant = transition(
    implementation = _variant_impl,
    inputs = [
        "//command_line_option:copt",
        "//command_line_option:platforms",
    ],
    outputs = [
        "//command_line_option:copt",
        "//command_line_option:platforms",
    ],
)

def _variant_files_impl(ctx):
    src = ctx.attr.src[0] if type(ctx.attr.src) == "list" else ctx.attr.src
    files = []
    for file in src[DefaultInfo].files.to_list():
        link = ctx.actions.declare_file(ctx.label.name + "/" + file.basename)
        ctx.actions.symlink(output = link, target_file = file)
        files.append(link)
    return [DefaultInfo(files = depset(files), runfiles = ctx.runfiles(files = files))]

msix_variant = rule(
    implementation = _variant_files_impl,
    doc = "Links the files of one target built with another platform or extra copts under its own name.",
    attrs = {
        "copts": attr.string_list(doc = "Copts added for the target."),
        "platform": attr.label(doc = "Target platform. Unset keeps the current one."),
        "src": attr.label(doc = "The target to build.", mandatory = True, cfg = _variant),
    },
)
