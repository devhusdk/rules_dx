"""The dart_config rule and the provider the Dart quality aspects read from it."""

load("//dart/rules:providers.bzl", "DartInfo")
load("//quality:native_config.bzl", "native_config_impl")

DxDartConfigInfo = provider(
    doc = "Dart packages a dart_config pulls in for the analyzer.",
    fields = {
        "packages": "depset[DartPackageInfo]: packages of the config deps.",
        "resources": "depset[File]: non-Dart lib files of those packages.",
        "srcs": "depset[File]: Dart sources of those packages.",
    },
)

def _dart_config_impl(ctx):
    result = native_config_impl(ctx)
    infos = [dep[DartInfo] for dep in ctx.attr.deps]
    return result + [DxDartConfigInfo(
        packages = depset(transitive = [i.transitive_packages for i in infos]),
        resources = depset(transitive = [i.transitive_resources for i in infos]),
        srcs = depset(transitive = [i.transitive_srcs for i in infos]),
    )]

dart_config = rule(
    implementation = _dart_config_impl,
    doc = "Native config file for dart, with the packages its include: lines name.",
    attrs = {
        "allow_generated": attr.bool(
            doc = "Accepts generated config and data files, not only checked-in sources.",
            default = False,
        ),
        "data": attr.label_list(
            doc = "Extra files the tool reads alongside its config.",
            allow_files = True,
            default = [],
        ),
        "deps": attr.label_list(
            doc = "Dart packages the config includes, for example @my_pub_deps//:lints.",
            providers = [DartInfo],
            default = [],
        ),
        "src": attr.label(
            doc = "Config file the tool reads, normally a checked-in source.",
            allow_single_file = True,
            mandatory = True,
        ),
        "_tool_id": attr.string(
            default = "dart",
        ),
    },
)
