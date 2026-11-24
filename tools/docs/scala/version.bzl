"""The Scala versions the Scaladoc extractor can read."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")

_SCALADOC_VERSION_PREFIX = "2.13."

SCALA_VERSION_ATTRS = {
    "_scala_version": attr.label(default = Label("@rules_scala_config//:scala_version")),
}

def unsupported_scala_version(ctx):
    """Returns the configured Scala version when Scaladoc cannot read it, else None."""
    version = ctx.attr._scala_version[BuildSettingInfo].value
    if version.startswith(_SCALADOC_VERSION_PREFIX):
        return None
    return version
