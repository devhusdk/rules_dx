"""A dx target that fails analysis with a message, for platforms with no usable prebuilt dx."""

def _dx_unavailable_impl(ctx):
    fail(ctx.attr.message.replace("{os}", ctx.attr.os).replace("{cpu}", ctx.attr.cpu))

dx_unavailable = rule(
    implementation = _dx_unavailable_impl,
    doc = "Stands in for dx where no prebuilt binary can be used; any build of it fails.",
    attrs = {
        "cpu": attr.string(doc = "The configured cpu, substituted for {cpu}.", mandatory = True),
        "message": attr.string(doc = "The failure text; {os} and {cpu} are substituted.", mandatory = True),
        "os": attr.string(doc = "The configured os, substituted for {os}.", mandatory = True),
    },
    executable = True,
)
