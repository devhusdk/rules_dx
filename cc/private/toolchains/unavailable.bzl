"""Stand-in for a C++ toolchain that cannot be used."""

GCC_UNSUPPORTED = "--@rules_dx//cc:compiler=gcc has no GCC toolchain for this target platform; GCC supports Linux x86_64 and Linux arm64 targets only. Drop the flag to use LLVM."

def _cc_toolchain_unavailable_impl(ctx):
    fail(ctx.attr.error)

cc_toolchain_unavailable = rule(
    implementation = _cc_toolchain_unavailable_impl,
    doc = "Fails the build with the reason the C++ toolchain is unavailable.",
    attrs = {
        "error": attr.string(mandatory = True, doc = "Why the toolchain is unavailable."),
    },
)
