"""The Go toolchain's go binary, for apidiff."""

def _go_binary_impl(ctx):
    sdk = ctx.toolchains["@rules_go//go:toolchain"].sdk
    runfiles = ctx.runfiles(transitive_files = depset([sdk.go], transitive = [sdk.srcs, sdk.libs, sdk.tools, sdk.headers]))
    return [DefaultInfo(files = depset([sdk.go]), runfiles = runfiles)]

dx_go_binary_file = rule(
    implementation = _go_binary_impl,
    doc = "Exposes the go binary of the registered Go SDK, with the SDK files as runfiles.",
    toolchains = ["@rules_go//go:toolchain"],
)
