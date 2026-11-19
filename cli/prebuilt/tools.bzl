"""The rules_dx tools a release ships prebuilt, and how a consumer finds each one.

Hand-written. fill_pins reads the asset list rendered from this file
(//cli/prebuilt:registry) and writes one sha256 per tool and platform into
pins.bzl, so adding a tool here and to release.yml is all a new prebuilt needs.

A platform key is "<os>_<cpu>" in release asset spelling: os is linux, macos or
windows, cpu is x86_64 or arm64, and a jar that runs anywhere uses "any".
"""

visibility(["//cli/...", "//core/quality"])

# Release asset spelling to @platforms constraint values.
RELEASE_OS = {
    "linux": "linux",
    "macos": "macos",
    "windows": "windows",
}

RELEASE_CPU = {
    "arm64": "aarch64",
    "x86_64": "x86_64",
}

_NATIVE = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]

def _exec_native(tool):
    return struct(
        asset = "dx-" + tool + "-{os}-{cpu}",
        role = "exec",
        platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@rules_dx//rust/private/quality/tools:" + tool + "_source",
        member = "",
        main_class = "",
    )

# Haskell tools: windows_arm64 runs the x86_64 build under emulation.
_HASKELL = ["linux_arm64", "macos_arm64", "windows_x86_64"]

def _haskell(tool, role, source):
    return struct(
        asset = "dx-" + tool.replace("_", "-") + "-{os}-{cpu}",
        role = role,
        platforms = _HASKELL,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = source,
        member = "",
        main_class = "",
    )

# Hackage tools released only on the platforms with no upstream binary.
def _hackage(tool, platforms):
    return struct(
        asset = "dx-" + tool + "-{os}-{cpu}",
        role = "exec",
        platforms = platforms,
        fallbacks = {},
        source = "@rules_dx//haskell/private/tools/" + tool + "/bin:source",
        member = "",
        main_class = "",
    )

def _jar(tool, source, main_class):
    return struct(
        asset = "dx-" + tool.replace("_", "-") + "-any.jar",
        role = "exec",
        platforms = ["any"],
        fallbacks = {},
        source = source,
        member = "",
        main_class = main_class,
    )

def _native_exec(source):
    return struct(
        asset = "dx-{tool}-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = source,
        member = "",
        main_class = "",
    )

# Each tool:
#   asset: the release asset name; {os} and {cpu} are filled from the platform key
#     and {tool} from the tool name.
#   role: "exec" when the tool runs during the build (the hub selects on the exec
#     platform), "target" when it ships inside the built output (target platform).
#   platforms: the platform keys a release builds.
#   fallbacks: a platform key with no pin of its own that may run another key's
#     download, such as windows_arm64 running x86_64 under emulation.
#   source: the label that builds the tool when there is no usable pin.
#   member: the file to run inside an archive asset; "" for a bare binary.
#   main_class: the entry point of a jar asset; "" for a native binary.
TOOLS = {
    "dx": struct(
        asset = "dx-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@rules_dx//cli/cli:dx",
        member = "",
        main_class = "",
    ),
    "gazelle": struct(
        asset = "dx-gazelle-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        # The gazelle hub builds its own gazelle_binary from the registered languages.
        source = "@dx_gazelle//:gazelle_source",
        member = "",
        main_class = "",
    ),
    "go": _native_exec("@rules_dx//go/private/tools/analysis/dx_go:dx_go"),
    # Android and iOS targets have no asset and build the launcher from source.
    "launcher": struct(
        asset = "dx-launcher-{os}-{cpu}",
        role = "target",
        platforms = _NATIVE,
        fallbacks = {},
        source = "@rules_dx//cli/launcher:exec_launcher",
        member = "",
        main_class = "",
    ),
    "capnp": _native_exec("@dx_capnproto//src/capnp:capnp_tool"),
    "flatc": _native_exec("@dx_flatbuffers//:flatc"),
    "thrift": _native_exec("@dx_thrift//:thrift_compiler"),
    # Upstream oapi-codegen, cross-built unmodified by the release-tools go job.
    "oapi_codegen": struct(
        asset = "dx-oapi-codegen-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@com_github_oapi_codegen_oapi_codegen_v2//cmd/oapi-codegen",
        member = "",
        main_class = "",
    ),
    "deadnix": _exec_native("deadnix"),
    "progenitor": _exec_native("progenitor"),
    "selene": _exec_native("selene"),
    "topiary": _exec_native("topiary"),
    "avro_union": _jar("avro_union", "@rules_dx//avro/private/tools/avro_union", "dx.avro.tools.AvroUnion"),
    "javac_check": _jar("javac_check", "@rules_dx//java/private/quality/tools/javac_check", "dx.quality.javac.Main"),
    "javadoc_extract": _jar("javadoc_extract", "@rules_dx//tools/docs/jvm/javadoc:javadoc_extract", "dx.docs.javadoc.Main"),
    "jmh_generate": _jar("jmh_generate", "@rules_dx//java/private/jmh:jmh_generate", "dx.bench.JmhGenerate"),
    "kotlinc_check": _jar("kotlinc_check", "@rules_dx//java/private/quality/tools/kotlinc_check", "dx.quality.kotlinc.Main"),
    "scalac_check": _jar("scalac_check", "@rules_dx//java/private/quality/tools/scalac_check", "dx.quality.scalac.Main"),
    "hpc_lcov": _haskell("hpc_lcov", "target", "@rules_dx//haskell/private/coverage:hpc_lcov_source"),
    "weeder": _haskell("weeder", "exec", "@rules_dx//haskell/private/tools/weeder/bin:source"),
    "hlint": _hackage("hlint", ["linux_arm64", "macos_arm64"]),
    "ormolu": _hackage("ormolu", ["linux_arm64"]),
    "nixfmt": struct(
        asset = "dx-nixfmt-{os}-{cpu}",
        role = "exec",
        platforms = ["linux_arm64"],
        fallbacks = {},
        source = "@rules_dx//core/quality/tools/nixfmt/bin:nixfmt_source",
        member = "",
        main_class = "",
    ),
    "kcov": struct(
        asset = "dx-kcov-{os}-{cpu}",
        role = "target",
        platforms = ["linux_arm64", "linux_x86_64"],
        fallbacks = {},
        source = "@rules_dx//tools/coverage/kcov:kcov",
        member = "",
        main_class = "",
    ),
    # Pinned only: the hub does not serve this bundle.
    "python_tools": struct(
        asset = "dx-python-tools-{os}-{cpu}.tar.gz",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {},
        source = "",
        member = "",
        main_class = "",
    ),
}

# The tools whose source builds need rules_dx dev-only modules; a consumer hub serves
# this label on platforms with no pin instead.
DEV_SOURCE_FALLBACK = {"kcov": "@rules_dx//tools/coverage/kcov:none"}

# The tools released as one native binary per platform.
NATIVE_TOOLS = ["capnp", "deadnix", "flatc", "hlint", "hpc_lcov", "kcov", "nixfmt", "oapi_codegen", "ormolu", "progenitor", "selene", "thrift", "topiary", "weeder"]

# The tools released as one platform-independent jar each.
JAR_TOOLS = sorted([tool for tool, entry in TOOLS.items() if entry.main_class])

def split_key(key):
    """Returns (os, cpu) in release spelling for a platform key such as "linux_x86_64"."""
    os, _, cpu = key.partition("_")
    return os, cpu

def asset_name(tool, key):
    """Returns the release asset name of tool on the platform key."""
    os, cpu = split_key(key)
    return TOOLS[tool].asset.format(tool = tool, os = os, cpu = cpu)

def registry_lines():
    """Returns one "<tool> <key> <asset>" line per release asset, sorted, for fill_pins."""
    return sorted([
        "{} {} {}".format(tool, key, asset_name(tool, key))
        for tool, entry in TOOLS.items()
        for key in entry.platforms
    ])
