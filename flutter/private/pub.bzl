"""The flutter pub hub extension, built from the patched rules_flutter copy."""

load("@dx_rules_flutter_pub//flutter/private:flutter_pub_lock_hub.bzl", "flutter_pub_lock_hub")
load("@dx_rules_flutter_pub//flutter/private:flutter_pub_package.bzl", "flutter_pub_package")
load("@dx_rules_flutter_pub//flutter/private:flutter_sdk_package.bzl", "flutter_sdk_package")
load("@dx_rules_flutter_pub//flutter/private:pub_lock_sdks.bzl", "parse_lock_sdk_constraints", "version_below_lower_bound")
load("@dx_rules_flutter_pub//flutter/private:versions.bzl", "flutter_source_sha256")
load("@rules_dart//dart/ext:registry.bzl", "curated_code_assets")
load("@rules_dart//dart/pub:yaml_parser.bzl", "parse_pubspec_lock")
load("//core/modules:versions.bzl", "FLUTTER_SDK_VERSION")

_SDK_PACKAGES = ["flutter", "flutter_test", "flutter_driver", "flutter_localizations", "flutter_web_plugins"]

_pub_tag = tag_class(attrs = {
    "flutter_version": attr.string(
        doc = "Flutter SDK version the lock resolves against.",
        default = "",
    ),
    "ignore_hooks": attr.string_list(
        doc = "Pub packages whose hook/build.dart is never reached.",
        default = [],
    ),
    "lock": attr.label(
        doc = "The Flutter pub lock file to resolve.",
        mandatory = True,
        allow_single_file = True,
    ),
    "name": attr.string(
        doc = "Name of the generated hub repository.",
        mandatory = True,
    ),
})

def _pub_impl(mctx):
    for mod in mctx.modules:
        for tag in mod.tags.pub:
            _hub(mctx, tag)
    return mctx.extension_metadata(reproducible = True)

def _hub(mctx, tag):
    flutter_version = tag.flutter_version or FLUTTER_SDK_VERSION
    lock_content = mctx.read(tag.lock)
    constraint = parse_lock_sdk_constraints(lock_content).get("flutter", "")
    if constraint and version_below_lower_bound(flutter_version, constraint):
        fail("{lock} requires flutter {constraint}, and flutter_version is {version}".format(
            lock = str(tag.lock),
            constraint = constraint,
            version = flutter_version,
        ))
    hosted = {}
    sdk = []
    for name, info in parse_pubspec_lock(lock_content).items():
        source = info.get("source", "unknown")
        if source == "hosted":
            hosted[name] = info
        elif source == "sdk":
            if name in _SDK_PACKAGES:
                sdk.append(name)
        else:
            fail("flutter pub: package {} has unsupported source {}".format(name, source))
    names = sorted(hosted.keys() + sdk)
    ignore_hooks = {name: True for name in tag.ignore_hooks}
    for name, info in hosted.items():
        desc = info.get("description", {})
        version = info.get("version", "")
        flutter_pub_package(
            name = tag.name + "__" + name,
            package_name = name,
            version = version,
            sha256 = desc.get("sha256", "") if type(desc) == "dict" else "",
            base_url = desc.get("url", "https://pub.dev") if type(desc) == "dict" else "https://pub.dev",
            hub_name = tag.name,
            lock_packages = names,
            overlay_roots = ["@rules_flutter//ext:BUILD.bazel"],
            code_assets = curated_code_assets(name, version),
            ignore_hook = name in ignore_hooks,
        )
    source_sha256 = flutter_source_sha256(flutter_version)
    for name in sdk:
        flutter_sdk_package(
            name = tag.name + "__" + name,
            package_name = name,
            flutter_version = flutter_version,
            sha256 = source_sha256,
            hub_name = tag.name,
            lock_packages = names,
        )
    flutter_pub_lock_hub(
        name = tag.name,
        hub_name = tag.name,
        packages = names,
    )

pub = module_extension(
    implementation = _pub_impl,
    tag_classes = {"pub": _pub_tag},
    doc = "Resolves a Flutter pub lock into a hub of Flutter packages fetched with the pinned bsdtar.",
)
