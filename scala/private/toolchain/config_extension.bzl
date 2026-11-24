"""The `scala_config` extension: writes `@rules_scala_config` and the Scala toolchain hub for the root module's Scala versions."""

SCALA_VERSION = "2.13.18"
SCALA_VERSIONS = [SCALA_VERSION, "3.3.9"]

HUB_NAME = "dx_scala_toolchains"

def _suffix(version):
    return "_" + version.replace(".", "_")

def scala_versions(settings):
    """Returns the default Scala version and every version, from the root module's `settings` tags."""
    if not settings:
        return SCALA_VERSION, SCALA_VERSIONS
    if len(settings) > 1:
        fail("scala_config.settings may appear once in the root module, found {}".format(len(settings)))
    version = settings[0].scala_version
    versions = [version]
    for extra in settings[0].scala_versions:
        if extra not in versions:
            versions.append(extra)
    for v in versions:
        if len(v.split(".")) != 3:
            fail("scala_config.settings: Scala version {} is not <major>.<minor>.<patch>".format(repr(v)))
    return version, versions

_CONFIG_BUILD = """load("@bazel_skylib//rules:common_settings.bzl", "string_setting")
string_setting(
    name = "scala_version",
    build_setting_default = "{version}",
    scope = "universal",
    values = {versions},
    visibility = ["//visibility:public"],
)
"""

_CONFIG_SETTING = """config_setting(
    name = "scala_version{suffix}",
    flag_values = {{":scala_version": "{version}"}},
    visibility = ["//visibility:public"],
)
"""

_CONFIG_SRCS = """
filegroup(
    name = "dx_bzl_srcs",
    srcs = ["config.bzl"],
    visibility = ["//visibility:public"],
)
"""

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
    build = _CONFIG_BUILD.format(version = version, versions = versions)
    build += "".join([_CONFIG_SETTING.format(suffix = _suffix(v), version = v) for v in versions])
    build += _CONFIG_SRCS
    rctx.file("BUILD", build)
    return rctx.repo_metadata(reproducible = True)

_config_repository = repository_rule(
    implementation = _config_impl,
    attrs = {
        "scala_version": attr.string(mandatory = True, doc = "The default Scala version."),
        "scala_versions": attr.string_list(mandatory = True, doc = "Every Scala version the hub supports."),
    },
)

_TOOLCHAIN = """toolchain(
    name = "{name}",
    target_settings = {settings},
    toolchain = "{toolchain}",
    toolchain_type = "{toolchain_type}",
)
"""

def hub_build(versions):
    """Returns the load-free hub BUILD with a semanticdb, an upstream and a testing toolchain per Scala version."""
    toolchains_on = str(Label("//config:toolchains_on"))
    toolchain_type = str(Label("//scala/private/toolchain/hub:toolchain_type"))
    testing_toolchain_type = str(Label("//scala/private/toolchain/hub:testing_toolchain_type"))
    out = ['package(default_visibility = ["//visibility:public"])\n']

    # Bazel registers the toolchains of a package in name order, so semanticdb_* wins over upstream_* when both match.
    for v in versions:
        suffix = _suffix(v)
        version_setting = str(Label("@rules_scala_config//:scala_version" + suffix))
        semanticdb = "semanticdb_scala3_impl" if v.startswith("3.") else "semanticdb_impl"
        out.append(_TOOLCHAIN.format(
            name = "semanticdb" + suffix,
            settings = repr([toolchains_on, version_setting]),
            toolchain = str(Label("//scala/private/toolchain:" + semanticdb)),
            toolchain_type = toolchain_type,
        ))
        out.append(_TOOLCHAIN.format(
            name = "upstream" + suffix,
            settings = repr([version_setting]),
            toolchain = str(Label("@rules_scala_toolchains//scala:toolchain" + suffix + "_impl")),
            toolchain_type = toolchain_type,
        ))
        out.append(_TOOLCHAIN.format(
            name = "testing" + suffix,
            settings = repr([version_setting]),
            toolchain = str(Label("@rules_scala_toolchains//testing:testing_toolchain" + suffix + "_impl")),
            toolchain_type = testing_toolchain_type,
        ))
    return "\n".join(out)

def _hub_impl(rctx):
    rctx.file("BUILD.bazel", hub_build(rctx.attr.scala_versions))
    return rctx.repo_metadata(reproducible = True)

_hub_repository = repository_rule(
    implementation = _hub_impl,
    doc = "Writes the Scala toolchain hub for the given Scala versions.",
    attrs = {
        "scala_versions": attr.string_list(mandatory = True, doc = "Every Scala version that gets toolchains."),
    },
)

def _scala_config_impl(mctx):
    settings = [tag for mod in mctx.modules if mod.is_root for tag in mod.tags.settings]
    version, versions = scala_versions(settings)
    _config_repository(
        name = "rules_scala_config",
        scala_version = version,
        scala_versions = versions,
    )
    _hub_repository(
        name = HUB_NAME,
        scala_versions = versions,
    )
    return mctx.extension_metadata(reproducible = True)

_settings = tag_class(
    doc = "Chooses the Scala versions; only the root module's tag counts.",
    attrs = {
        "scala_version": attr.string(default = SCALA_VERSION, doc = "The default Scala version."),
        "scala_versions": attr.string_list(doc = "More Scala versions that targets select with `@rules_scala_config//:scala_version`."),
    },
)

scala_config = module_extension(
    implementation = _scala_config_impl,
    doc = "Writes `@rules_scala_config` and the Scala toolchain hub; Scala 2.13.18 and 3.3.9 unless the root module sets `settings`.",
    tag_classes = {"settings": _settings},
)
