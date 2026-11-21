"""The `scala_config` extension: writes the `@rules_scala_config` repository rules_scala reads."""

SCALA_VERSION = "2.13.18"
SCALA_VERSIONS = [SCALA_VERSION, "3.3.9"]

def _suffix(version):
    return "_" + version.replace(".", "_")

def _config_impl(rctx):
    version = rctx.attr.scala_version
    versions = rctx.attr.scala_versions
    major, minor = version.rsplit(".", 1)
    rctx.file("config.bzl", "\n".join([
        "SCALA_VERSION='" + version + "'",
        "SCALA_VERSIONS=" + str(versions),
        "SCALA_MAJOR_VERSION='" + major + "'",
        "SCALA_MINOR_VERSION='" + minor + "'",
        "ENABLE_COMPILER_DEPENDENCY_TRACKING=False",
    ]))
    build = """load("@bazel_skylib//rules:common_settings.bzl", "string_setting")
string_setting(
    name = "scala_version",
    build_setting_default = "{version}",
    scope = "universal",
    values = {versions},
    visibility = ["//visibility:public"],
)
""".format(version = version, versions = versions)
    build += "".join(["""config_setting(
    name = "scala_version{suffix}",
    flag_values = {{":scala_version": "{version}"}},
    visibility = ["//visibility:public"],
)
""".format(suffix = _suffix(v), version = v) for v in versions])
    build += """
filegroup(
    name = "dx_bzl_srcs",
    srcs = ["config.bzl"],
    visibility = ["//visibility:public"],
)
"""
    rctx.file("BUILD", build)

_config_repository = repository_rule(
    implementation = _config_impl,
    attrs = {
        "scala_version": attr.string(mandatory = True),
        "scala_versions": attr.string_list(mandatory = True),
    },
)

def _scala_config_impl(mctx):
    _config_repository(
        name = "rules_scala_config",
        scala_version = SCALA_VERSION,
        scala_versions = SCALA_VERSIONS,
    )
    return mctx.extension_metadata(reproducible = True)

scala_config = module_extension(
    implementation = _scala_config_impl,
    doc = "Writes `@rules_scala_config` with Scala 2.13.18 as the default and 3.3.9 as a cross version.",
)
