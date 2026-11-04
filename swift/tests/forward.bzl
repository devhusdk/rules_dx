"""Test-only forwarders that build upstream Swift targets for the Swift Linux platform."""

load("//libs/starlark:wrapper.bzl", "dx_forwarded_runtime_providers", "dx_symlink_default_info")
load("//swift/platforms:transition.bzl", "swift_linux_platform", "swift_linux_transition")

def _impl(ctx):
    upstream = ctx.attr.upstream[0]
    return [
        dx_symlink_default_info(ctx, "swift_linux_*"),
    ] + dx_forwarded_runtime_providers(upstream, "swift_linux_*")

_ATTRS = {
    "swift_platform": attr.string(doc = "Swift platform to build for."),
    "upstream": attr.label(
        cfg = swift_linux_transition,
        executable = True,
        mandatory = True,
        doc = "Upstream Swift target.",
    ),
}

_swift_linux_binary = rule(
    implementation = _impl,
    attrs = _ATTRS,
    executable = True,
    doc = "Runs an upstream Swift binary built for the Swift platform.",
)

_swift_linux_test = rule(
    implementation = _impl,
    attrs = _ATTRS,
    test = True,
    doc = "Runs an upstream Swift test built for the Swift platform.",
)

def swift_linux_binary(name, upstream, **kwargs):
    """Declares a Swift binary forwarder."""
    _swift_linux_binary(name = name, upstream = upstream, swift_platform = swift_linux_platform(), **kwargs)

def swift_linux_test(name, upstream, **kwargs):
    """Declares a Swift test forwarder."""
    _swift_linux_test(name = name, upstream = upstream, swift_platform = swift_linux_platform(), **kwargs)
