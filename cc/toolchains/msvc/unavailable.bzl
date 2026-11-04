"""Stand-in for the MSVC toolchain when its license is not accepted."""

def _msvc_unavailable_impl(ctx):
    fail(ctx.attr.error)

msvc_unavailable = rule(
    implementation = _msvc_unavailable_impl,
    doc = "Fails the build with the reason the MSVC toolchain is unavailable.",
    attrs = {
        "error": attr.string(mandatory = True, doc = "Why the toolchain is unavailable."),
    },
)
