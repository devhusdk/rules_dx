# Copyright (c) Joby Aviation 2022
# Original authors: Thulio Ferraz Assis (thulio@aspect.dev), Aspect.dev
#
# Copyright (c) Thulio Ferraz Assis 2024
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Vendored from gcc_toolchain 0.13.0 with patches/gcc_toolchain/*.patch applied.

"""This module provides the definitions for registering a GCC toolchain for C and C++.
"""

load("@bazel_skylib//lib:dicts.bzl", "dicts")
load("@bazel_skylib//lib:paths.bzl", "paths")
load("@bazel_tools//tools/build_defs/repo:utils.bzl", "read_user_netrc", "use_netrc")

_AUTO_BINARY_PREFIX = "auto"

_TRIPLE_BINARY_PREFIXES = {
    "aarch64": "aarch64-linux-",
    "x86_64": "x86_64-linux-",
}

def _detect_binary_prefix(rctx, target_arch):
    """Resolves the prefix that `bin/` binaries carry in the extracted toolchain.

    Some gcc-builds releases lay the binaries out under the target triple prefix and
    others leave them unprefixed. `as` tells the two layouts apart, since the
    unprefixed layout never provides a prefixed alias for it.

    Args:
        rctx: The repository context, with the toolchain already extracted.
        target_arch: The target architecture of the toolchain.

    Returns:
        The binary prefix, either the target triple prefix or the empty string.
    """
    triple_prefix = _TRIPLE_BINARY_PREFIXES[target_arch]
    if rctx.path("bin/{}as".format(triple_prefix)).exists:
        return triple_prefix
    return ""

_GLIBC_LINKER_SCRIPTS = ["sysroot/usr/lib/libc.so", "sysroot/usr/lib/libm.so"]

def _sysroot_relative_linker_scripts(rctx):
    """Prefixes absolute paths in the glibc linker scripts with the sysroot marker."""
    for name in _GLIBC_LINKER_SCRIPTS:
        script = rctx.path(name)
        if not script.exists:
            continue
        content = rctx.read(script)
        if not content.startswith("/* GNU ld script"):
            continue
        rctx.delete(script)
        rctx.file(
            name,
            content.replace(" /lib/", " =/lib/").replace(" /usr/lib/", " =/usr/lib/"),
            executable = False,
        )

def _gcc_toolchain_impl(rctx):
    versions = json.decode(rctx.attr.gcc_versions)
    url = versions[rctx.attr.gcc_version][rctx.attr.target_arch]["url"]
    sha256 = versions[rctx.attr.gcc_version][rctx.attr.target_arch]["sha256"]
    rctx.download_and_extract(
        url = url,
        sha256 = sha256,
        auth = use_netrc(read_user_netrc(rctx), [url], {}),
    )
    _sysroot_relative_linker_scripts(rctx)

    absolute_toolchain_root = str(rctx.path("."))
    execroot = paths.normalize(paths.join(absolute_toolchain_root, "..", ".."))
    toolchain_root = paths.relativize(absolute_toolchain_root, execroot)

    def _format_flags(flags):
        return [
            flag.replace("%workspace%", toolchain_root)
            for flag in flags
        ]

    def _format_builtins(builtins):
        return [d.replace("%workspace%", toolchain_root) for d in builtins]

    target_arch = rctx.attr.target_arch

    binary_prefix = rctx.attr.binary_prefix
    if binary_prefix == _AUTO_BINARY_PREFIX:
        binary_prefix = _detect_binary_prefix(rctx, target_arch)
    tool_paths, path_env = _render_tool_paths(rctx, toolchain_root, binary_prefix)
    rctx.file("tool_paths.bzl", "tool_paths = {}".format(str(tool_paths)))

    include_prefix = None
    if target_arch == ARCHS.aarch64:
        include_prefix = "aarch64-linux/"
    elif target_arch == ARCHS.x86_64:
        include_prefix = "x86_64-linux/"

    c_builtin_includes = [
        include.format(
            gcc_version = rctx.attr.gcc_version,
            include_prefix = include_prefix,
        )
        for include in [
            "%workspace%/lib/gcc/{include_prefix}{gcc_version}/include",
            "%workspace%/lib/gcc/{include_prefix}{gcc_version}/include-fixed",
        ] + ([
            "%workspace%/{include_prefix}include",
        ] if target_arch != ARCHS.x86_64 else []) + [
            "%workspace%/sysroot/usr/include",
        ]
    ]

    cxx_builtin_includes = []
    if target_arch == ARCHS.x86_64 or not rctx.path("{}include/c++".format(include_prefix)).exists:
        cxx_builtin_includes.extend([
            include.format(
                gcc_version = rctx.attr.gcc_version,
                include_prefix = include_prefix,
            )
            for include in [
                "%workspace%/include/c++/{gcc_version}",
                "%workspace%/include/c++/{gcc_version}/{include_prefix}",
            ]
        ])
    else:
        cxx_builtin_includes.extend([
            include.format(
                gcc_version = rctx.attr.gcc_version,
                include_prefix = include_prefix,
            )
            for include in [
                "%workspace%/{include_prefix}include/c++/{gcc_version}",
                "%workspace%/{include_prefix}include/c++/{gcc_version}/{include_prefix}",
            ]
        ])

    builtin_include_directories = []
    builtin_include_directories.extend(c_builtin_includes)
    builtin_include_directories.extend(cxx_builtin_includes)
    builtin_include_directories.extend(rctx.attr.includes)

    extra_cflags = [
        "-nostdinc",
        "-B%workspace%/bin",
        "-B%workspace%/xbin",
    ]
    extra_cflags.extend([
        "-isystem{}".format(include)
        for include in c_builtin_includes
    ])
    extra_cflags.extend([
        "-I{}".format(include)
        for include in rctx.attr.includes
    ])
    extra_cflags.extend(rctx.attr.extra_cflags)

    extra_cxxflags = [
        "-nostdinc",
        "-nostdinc++",
        "-B%workspace%/bin",
        "-B%workspace%/xbin",
    ]
    extra_cxxflags.extend([
        "-isystem{}".format(include)
        for include in cxx_builtin_includes
    ])
    extra_cxxflags.extend([
        "-isystem{}".format(include)
        for include in c_builtin_includes
    ])
    extra_cxxflags.extend([
        "-I{}".format(include)
        for include in rctx.attr.includes
    ])
    extra_cxxflags.extend(rctx.attr.extra_cxxflags)

    # No sysroot -B/-L flags: relative paths make lld reject glibc's absolute linker-script entries.
    extra_ldflags = [
        lib.format(
            include_prefix = include_prefix,
        )
        for lib in [
            "-B%workspace%/bin",
            "-B%workspace%/xbin",
            "-B%workspace%/lib",
            "-B%workspace%/{include_prefix}lib",
            "-B%workspace%/lib64",
            "-B%workspace%/{include_prefix}lib64",
            "-L%workspace%/lib",
            "-L%workspace%/{include_prefix}lib",
            "-L%workspace%/lib64",
            "-L%workspace%/{include_prefix}lib64",
        ]
    ]
    extra_ldflags.extend(rctx.attr.extra_ldflags)

    extra_asmflags = []
    extra_asmflags.extend([
        "-isystem{}".format(include)
        for include in c_builtin_includes
    ])
    extra_asmflags.extend([
        "-I{}".format(include)
        for include in rctx.attr.includes
    ])
    extra_asmflags.extend(rctx.attr.extra_asmflags)

    rctx.file("BUILD.bazel", _TOOLCHAIN_BUILD_FILE_CONTENT.format(
        cc_toolchain_config_bzl = str(Label("//cc/private/toolchain/gcc_repo:cc_toolchain_config.bzl")),
        binary_prefix = binary_prefix,
        include_prefix = include_prefix,

        # Includes
        cxx_builtin_include_directories = _format_builtins(builtin_include_directories),

        # Flags
        extra_cflags = _format_flags(extra_cflags),
        extra_cxxflags = _format_flags(extra_cxxflags),
        extra_ldflags = _format_flags(extra_ldflags),
        extra_asmflags = _format_flags(extra_asmflags),

        # Features
        extra_enabled_features = [str(feature_label) for feature_label in rctx.attr.extra_enabled_features],
        extra_known_features = [str(feature_label) for feature_label in rctx.attr.extra_known_features],

        path_env = path_env,

        # cc_toolchain attributes
        supports_param_files = 1 if rctx.attr.supports_param_files else 0,
        target_libc = rctx.attr.target_libc,
        target_system_name = "{}-linux-gnu".format(rctx.attr.target_arch),
    ))
    return rctx.repo_metadata(reproducible = True)

_FEATURE_ATTRS = {
    "binary_prefix": attr.string(
        doc = "An explicit prefix used by each binary in bin/. Defaults to detecting the prefix from the extracted toolchain.",
        default = _AUTO_BINARY_PREFIX,
    ),
    "extra_cflags": attr.string_list(
        doc = "Extra flags for compiling C.",
        default = [],
    ),
    "extra_cxxflags": attr.string_list(
        doc = "Extra flags for compiling C++.",
        default = [],
    ),
    "extra_ldflags": attr.string_list(
        doc = "Extra flags for linking." +
              " %workspace% is rendered to the toolchain root path." +
              " See https://github.com/bazelbuild/bazel/blob/a48e246e/src/main/java/com/google/devtools/build/lib/rules/cpp/CcToolchainProviderHelper.java#L234-L254.",
        default = [],
    ),
    "extra_asmflags": attr.string_list(
        doc = "Extra flags for the assembly preprocessor.",
        default = [],
    ),
    "extra_enabled_features": attr.label_list(
        doc = ("Extra `cc_feature` features to add to this toolchain in an initially " +
               "enabled state. This attribute has limited integration with `cc_feature`, " +
               "and does not run additional correctness checks or handle things like `data` " +
               "files. This is only offered as a migration bridge for projects transitioning " +
               "to rule-based toolchain configurations, or sharing of simple argument sets " +
               "with older toolchains."),
        default = [],
    ),
    "extra_known_features": attr.label_list(
        doc = ("Extra `cc_feature` features to add to this toolchain in an initially " +
               "disabled state. This attribute has limited integration with `cc_feature`, " +
               "and does not run additional correctness checks or handle things like `data` " +
               "files. This is only offered as a migration bridge for projects transitioning " +
               "to rule-based toolchain configurations, or sharing of simple argument sets " +
               "with older toolchains."),
        default = [],
    ),
    "supports_param_files": attr.bool(
        doc = "Set `supports_param_files = 1` on the generated `cc_toolchain`," +
              " which lets Bazel pass linker arguments via an `@params` file." +
              " Enable this when link command lines for large targets overflow `ARG_MAX`" +
              " (e.g. `collect2: posix_spawn: Argument list too long`)." +
              " Off by default to preserve historical behavior.",
        default = False,
    ),
    "gcc_version": attr.string(
        doc = "The version of GCC.",
        mandatory = True,
    ),
    "gcc_versions": attr.string(
        mandatory = True,
        doc = "A JSON dictionary of GCC versions to their download URLs and SHA256 hashes." +
              " The structure is {<gcc_version>: {<target_arch>: {url: <url>, sha256: <sha256>}}}.",
    ),
    "includes": attr.string_list(
        doc = "Extra includes for compiling C and C++." +
              " %workspace% is rendered to the toolchain root path." +
              " See https://github.com/bazelbuild/bazel/blob/a48e246e/src/main/java/com/google/devtools/build/lib/rules/cpp/CcToolchainProviderHelper.java#L234-L254.",
        default = [],
    ),
    "target_arch": attr.string(
        doc = "The target architecture this toolchain produces. E.g. x86_64.",
        mandatory = True,
    ),
    "target_libc": attr.string(
        doc = "The libc name and version reported in the cc toolchain identity. E.g. glibc_2.28.",
        default = "local",
    ),
}

# Attributes of the hub's `toolchain` declarations.
_TOOLCHAIN_DECLARATION_ATTRS = {
    "extra_target_compatible_with": attr.label_list(
        doc = "Additional constraint_values appended to target_compatible_with of the toolchain," +
              " on top of the values from the target_compatible_with attribute (including its defaults)." +
              " Unlike target_compatible_with, {target_arch} is not rendered.",
        mandatory = False,
    ),
    "extra_target_settings": attr.label_list(
        doc = "Additional config_settings appended to target_settings of the toolchain, on top of" +
              " the GCC version selection and the target_settings attribute." +
              " Unlike target_settings, {target_arch} is not rendered.",
        mandatory = False,
    ),
    "target_compatible_with": attr.string_list(
        default = [
            "@platforms//os:linux",
            "@platforms//cpu:{target_arch}",
        ],
        doc = "constraint_values passed to target_compatible_with of the toolchain. {target_arch} is rendered to the target_arch attribute value.",
        mandatory = False,
    ),
    "target_settings": attr.string_list(
        default = [],
        doc = "Additional config_settings passed to target_settings of the toolchain, on top of the GCC version selection. {target_arch} is rendered to the target_arch attribute value.",
        mandatory = False,
    ),
}

gcc_toolchain = repository_rule(
    _gcc_toolchain_impl,
    attrs = _FEATURE_ATTRS,
)

def _sanitize_version(gcc_version):
    return gcc_version.replace(".", "_")

_HUB_UNSET_SETTING_TEMPLATE = '''config_setting(
    name = "_gcc_version_unset",
    flag_values = {{"{gcc_version_flag}": ""}},
)'''

_HUB_VERSION_SETTING_TEMPLATE = '''config_setting(
    name = "_gcc_version_{suffix}",
    flag_values = {{"{gcc_version_flag}": "{gcc_version}"}},
)'''

_HUB_DEFAULT_SETTING_TEMPLATE = '''selects.config_setting_group(
    name = "_gcc_version_default",
    match_any = [
        ":_gcc_version_unset",
        ":_gcc_version_{suffix}",
    ],
)'''

_HUB_ALIAS_TEMPLATE = '''alias(
    name = "{target}",
    actual = select({actual}),
)'''

def _gcc_toolchains_hub_impl(rctx):
    target_arch = rctx.attr.target_arch
    gcc_version_flag = str(rctx.attr._gcc_version_flag)

    target_compatible_with = [
        v.format(target_arch = target_arch)
        for v in rctx.attr.target_compatible_with
    ]
    target_compatible_with.extend([str(c) for c in rctx.attr.extra_target_compatible_with])

    extra_target_settings = [
        v.format(target_arch = target_arch)
        for v in rctx.attr.target_settings
    ]
    extra_target_settings.extend([str(c) for c in rctx.attr.extra_target_settings])

    toolchain_repos = rctx.attr.toolchain_repos
    default_gcc_version = rctx.attr.default_gcc_version
    if default_gcc_version not in toolchain_repos:
        fail("The default GCC version {} has no toolchain for {}.".format(default_gcc_version, target_arch))

    content = [_HUB_BUILD_FILE_HEADER]

    content.append(_HUB_UNSET_SETTING_TEMPLATE.format(gcc_version_flag = gcc_version_flag))

    for gcc_version in sorted(toolchain_repos.keys()):
        content.append(_HUB_VERSION_SETTING_TEMPLATE.format(
            suffix = _sanitize_version(gcc_version),
            gcc_version = gcc_version,
            gcc_version_flag = gcc_version_flag,
        ))

    # The default version matches both the empty flag and its explicit value.
    content.append(_HUB_DEFAULT_SETTING_TEMPLATE.format(suffix = _sanitize_version(default_gcc_version)))

    for gcc_version in sorted(toolchain_repos.keys()):
        is_default = gcc_version == default_gcc_version
        suffix = "" if is_default else "_" + _sanitize_version(gcc_version)
        setting = ":_gcc_version_default" if is_default else ":_gcc_version_" + _sanitize_version(gcc_version)
        content.append(_HUB_TOOLCHAIN_TEMPLATE.format(
            suffix = suffix,
            target_compatible_with = target_compatible_with,
            target_settings = [setting] + extra_target_settings,
            toolchain_repo = toolchain_repos[gcc_version],
        ))

    # Aliases under the hub name that follow the selected version.
    for target in _HUB_ALIASED_TARGETS:
        actual = {
            ":_gcc_version_" + _sanitize_version(gcc_version): "@{}//:{}".format(toolchain_repo, target)
            for gcc_version, toolchain_repo in toolchain_repos.items()
            if gcc_version != default_gcc_version
        }
        actual["//conditions:default"] = "@{}//:{}".format(toolchain_repos[default_gcc_version], target)
        content.append(_HUB_ALIAS_TEMPLATE.format(target = target, actual = actual))

    rctx.file("BUILD.bazel", "\n\n".join(content) + "\n")
    return rctx.repo_metadata(reproducible = True)

_gcc_toolchains_hub = repository_rule(
    _gcc_toolchains_hub_impl,
    attrs = dicts.add(
        _TOOLCHAIN_DECLARATION_ATTRS,
        {
            "default_gcc_version": attr.string(
                doc = "The GCC version used when the gcc_version flag is not set.",
                mandatory = True,
            ),
            "target_arch": attr.string(
                doc = "The target architecture the toolchains produce. E.g. x86_64.",
                mandatory = True,
            ),
            "toolchain_repos": attr.string_dict(
                doc = "Maps each available GCC version to the repository holding its cc_toolchain.",
                mandatory = True,
            ),
            "_gcc_version_flag": attr.label(
                default = Label("@gcc_toolchain//toolchain:gcc_version"),
            ),
        },
    ),
)

def _render_tool_paths(rctx, toolchain_root, binary_prefix):
    tool_paths = {}
    for name in ["ar", "as", "cpp", "g++", "gcc", "gcov", "ld", "nm", "objcopy", "objdump", "strip"]:
        link = paths.join("xbin", name)
        rctx.symlink("bin/{}{}".format(binary_prefix, name), link)
        tool_paths[name] = link
    path_env = "{root}/xbin:{root}/bin".format(root = toolchain_root)
    return tool_paths, path_env

def gcc_declare_toolchain(
        name,
        target_arch,
        **kwargs):
    """Declares a `gcc_toolchain` for every available GCC version.

    `name` is the hub repository holding the `toolchain` declarations. Each GCC version gets its
    own repository holding the `cc_toolchain`, fetched only when that version is selected through
    the `@gcc_toolchain//toolchain:gcc_version` flag.

    Args:
        name: The name of the hub repository holding the toolchain declarations.
        target_arch: The target architecture of the toolchain.
        **kwargs: The extra arguments passed to `gcc_toolchain`. See `gcc_toolchain` for more info.
            The attributes of the `toolchain` declarations themselves are also accepted here,
            since they apply to the hub rather than to `gcc_toolchain`:

            `target_compatible_with`: constraint_values passed to `target_compatible_with` of the
            toolchain. `{target_arch}` is rendered to the `target_arch` argument value. Defaults to
            `["@platforms//os:linux", "@platforms//cpu:{target_arch}"]`.

            `extra_target_compatible_with`: Additional constraint_values appended to
            `target_compatible_with` of the toolchain, on top of the values from the
            `target_compatible_with` argument (including its defaults). Unlike
            `target_compatible_with`, `{target_arch}` is not rendered.

            `target_settings`: Additional config_settings passed to `target_settings` of the
            toolchain, on top of the GCC version selection. `{target_arch}` is rendered to the
            `target_arch` argument value.

            `extra_target_settings`: Additional config_settings appended to `target_settings` of
            the toolchain, on top of the GCC version selection and the `target_settings` argument.
            Unlike `target_settings`, `{target_arch}` is not rendered.
    """
    binary_prefix = kwargs.pop("binary_prefix", None)
    if binary_prefix == None:
        if target_arch not in _TRIPLE_BINARY_PREFIXES:
            fail("Unsupported target architecture: {}".format(target_arch))
        binary_prefix = _AUTO_BINARY_PREFIX

    default_gcc_version = kwargs.pop("gcc_version")
    gcc_versions = kwargs.pop("gcc_versions")
    extra_target_compatible_with = kwargs.pop("extra_target_compatible_with", [])
    target_compatible_with = kwargs.pop("target_compatible_with", None)
    target_settings = kwargs.pop("target_settings", [])
    extra_target_settings = kwargs.pop("extra_target_settings", [])

    # Left in kwargs so that the per-version repositories keep receiving it.
    repo_mapping = kwargs.get("repo_mapping", None)

    extra_cflags = kwargs.pop("extra_cflags", [])
    extra_cxxflags = kwargs.pop("extra_cxxflags", [])
    extra_ldflags = kwargs.pop("extra_ldflags", [])
    extra_asmflags = kwargs.pop("extra_asmflags", [])
    extra_enabled_features = kwargs.pop("extra_enabled_features", [])
    extra_known_features = kwargs.pop("extra_known_features", [])
    includes = kwargs.pop("includes", [])

    toolchain_repos = {}
    for gcc_version in json.decode(gcc_versions):
        toolchain_repo = "{}_{}".format(name, _sanitize_version(gcc_version))
        toolchain_repos[gcc_version] = toolchain_repo
        gcc_toolchain(
            name = toolchain_repo,
            binary_prefix = binary_prefix,
            extra_cflags = extra_cflags,
            extra_cxxflags = extra_cxxflags,
            extra_ldflags = extra_ldflags,
            extra_asmflags = extra_asmflags,
            extra_enabled_features = extra_enabled_features,
            extra_known_features = extra_known_features,
            includes = includes,
            gcc_version = gcc_version,
            gcc_versions = gcc_versions,
            target_arch = target_arch,
            **kwargs
        )

    hub_kwargs = {}

    # An explicitly empty target_compatible_with drops the default constraints.
    if target_compatible_with != None:
        hub_kwargs["target_compatible_with"] = target_compatible_with

    # The hub BUILD file needs the same repository mapping as the per-version repositories.
    if repo_mapping != None:
        hub_kwargs["repo_mapping"] = repo_mapping

    _gcc_toolchains_hub(
        name = name,
        default_gcc_version = default_gcc_version,
        extra_target_compatible_with = extra_target_compatible_with,
        extra_target_settings = extra_target_settings,
        target_arch = target_arch,
        target_settings = target_settings,
        toolchain_repos = toolchain_repos,
        **hub_kwargs
    )

ARCHS = struct(
    aarch64 = "aarch64",
    x86_64 = "x86_64",
)

# Public targets of a toolchain repository, aliased by the hub to the selected version.
_HUB_ALIASED_TARGETS = [
    "_cc_toolchain",
    "all_files",
    "ar",
    "ar_files",
    "as",
    "as_files",
    "cc_toolchain_config",
    "compiler_files",
    "coverage_files",
    "dwp_files",
    "gcc",
    "gcov",
    "include",
    "ld",
    "ld.bfd",
    "ld_files",
    "lib",
    "libasan",
    "liblsan",
    "libstdcxx",
    "libstdcxx_static",
    "libtsan",
    "libubsan",
    "linker_files",
    "lld_files",
    "nm",
    "objcopy",
    "objcopy_files",
    "objdump",
    "ranlib",
    "readelf",
    "strip",
    "strip_files",
]

_HUB_BUILD_FILE_HEADER = '''\
load("@bazel_skylib//lib:selects.bzl", "selects")

package(default_visibility = ["//visibility:public"])'''

_HUB_TOOLCHAIN_TEMPLATE = '''\
toolchain(
    name = "cc_toolchain{suffix}",
    exec_compatible_with = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
    target_compatible_with = {target_compatible_with},
    target_settings = {target_settings},
    toolchain = "@{toolchain_repo}//:_cc_toolchain",
    toolchain_type = "@rules_cc//cc:toolchain_type",
)'''

_TOOLCHAIN_BUILD_FILE_CONTENT = """\
load("@rules_cc//cc:defs.bzl", "cc_toolchain", "cc_library")
load("{cc_toolchain_config_bzl}", "cc_toolchain_config")
load("//:tool_paths.bzl", "tool_paths")

package(default_visibility = ["//visibility:public"])

cc_toolchain(
    name = "_cc_toolchain",
    all_files = ":all_files",
    ar_files = ":ar_files",
    as_files = ":as_files",
    compiler_files = ":compiler_files",
    dwp_files = ":dwp_files",
    linker_files = ":linker_files",
    objcopy_files = ":objcopy_files",
    strip_files = ":strip_files",
    coverage_files = ":coverage_files",
    supports_param_files = {supports_param_files},
    dynamic_runtime_lib = ":dx_dynamic_runtime_lib",
    static_runtime_lib = ":dx_static_runtime_lib",
    toolchain_config = ":cc_toolchain_config",
    toolchain_identifier = "gcc-toolchain",
)

cc_toolchain_config(
    name = "cc_toolchain_config",
    cxx_builtin_include_directories = {cxx_builtin_include_directories},
    extra_cflags = {extra_cflags},
    extra_cxxflags = {extra_cxxflags},
    extra_ldflags = {extra_ldflags},
    extra_asmflags = {extra_asmflags},
    target_libc = "{target_libc}",
    target_system_name = "{target_system_name}",
    extra_enabled_features = {extra_enabled_features},
    extra_known_features = {extra_known_features},
    tool_paths = tool_paths,
    path_env = "{path_env}",
)

filegroup(
    name = "dx_dynamic_runtime_lib",
    srcs = glob(
        include = [
            "lib64/libstdc++.so.6",
            "lib64/libgcc_s.so.1",
            "*/lib64/libstdc++.so.6",
            "*/lib64/libgcc_s.so.1",
        ],
        allow_empty = True,
    ),
)

filegroup(
    name = "dx_static_runtime_lib",
    srcs = [],
)

filegroup(
    name = "all_files",
    srcs = [
        ":ar_files",
        ":as_files",
        ":compiler_files",
        ":coverage_files",
        ":dwp_files",
        ":linker_files",
        ":objcopy_files",
        ":strip_files",
    ],
)

filegroup(
    name = "compiler_files",
    srcs = [
        ":as_files",
        ":gcc",
        ":include",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "linker_files",
    srcs = [
        ":ar",
        ":gcc",
        ":lib",
        ":ld_files",
        ":lld_files",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "ld_files",
    srcs = [
        ":ld",
        ":ld.bfd",
        "xbin/ld",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "lld_files",
    srcs = glob(["**/lld", "**/ld.lld"], allow_empty = True),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "include",
    srcs = glob([
        # C includes
        "lib/gcc/{include_prefix}*/include/**",
        "lib/gcc/{include_prefix}*/include-fixed/**",
        "{include_prefix}include/**",
        "sysroot/usr/include/**",

        # C++ includes
        "{include_prefix}include/c++/*/**",
        "include/c++/*/**",
    ], allow_empty=True),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "lib",
    srcs = glob(
        include = [
            "**/*.so",
            "**/*.so.*",
            "**/*.a",
            "**/*.la",
            "**/*.o",
            "**/*.lo",
            "**/lib*/libsanitizer.spec",
        ],
        exclude = ["lib*/**/*python*/**"],
        allow_empty = True,
    ),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "gcc",
    srcs = [
        "bin/{binary_prefix}cpp",
        "bin/{binary_prefix}g++",
        "bin/{binary_prefix}gcc",
        "xbin/cpp",
        "xbin/g++",
        "xbin/gcc",
    ] + glob([
        "**/libexec/gcc/**/cc1plus",
        "**/libexec/gcc/**/cc1",
        "**/libexec/gcc/**/collect2",
        "**/libexec/gcc/**/lto-wrapper",
        "**/libexec/gcc/**/lto1",
        # These shared objects are needed at runtime by GCC when linked dynamically to them.
        "lib/libgmp.so*",
        "lib/libmpc.so*",
        "lib/libmpfr.so*",
    ], allow_empty=True),
    visibility = ["//visibility:public"],
)

# Binutils

filegroup(
    name = "ar_files",
    srcs = [
        ":ar",
        "xbin/ar",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "as_files",
    srcs = [
        ":as",
        "xbin/as",
        ":gcc",
        "xbin/gcc",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "dwp_files",
    srcs = [],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "objcopy_files",
    srcs = [
        ":objcopy",
        "xbin/objcopy",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "strip_files",
    srcs = [
        ":strip",
        "xbin/strip",
    ],
    visibility = ["//visibility:public"],
)

filegroup(
    name = "coverage_files",
    srcs = [
        ":gcov",
        "xbin/gcov",
    ],
    visibility = ["//visibility:public"],
)

[
    filegroup(
        name = bin,
        srcs = [
            "bin/{binary_prefix}" + bin,
        ],
        visibility = ["//visibility:public"],
    )
    for bin in [
        "ar",
        "as",
        "gcov",
        "ld",
        "ld.bfd",
        "nm",
        "objcopy",
        "objdump",
        "ranlib",
        "readelf",
        "strip",
    ]
]

cc_library(
    name = "libstdcxx",
    srcs = glob(
        include = ["**/libstdc++.so*"],
        exclude = ["**/*.py"],
    ),
    visibility = ["//visibility:public"],
)

cc_library(
    name = "libstdcxx_static",
    srcs = glob(["**/libstdc++.a"]),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "libasan",
    srcs = glob([
        "**/lib*/libasan.so",
    ], allow_empty=True),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "liblsan",
    srcs = glob([
        "**/lib*/liblsan.so",
    ], allow_empty=True),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "libtsan",
    srcs = glob([
        "**/lib*/libtsan.so",
    ], allow_empty=True),
    visibility = ["//visibility:public"],
)

filegroup(
    name = "libubsan",
    srcs = glob([
        "**/lib*/libubsan.so",
    ], allow_empty=True),
    visibility = ["//visibility:public"],
)
"""
