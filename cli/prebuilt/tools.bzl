"""The rules_dx tools a release ships prebuilt, and how a consumer finds each one.

Hand-written. fill_pins reads the asset list rendered from this file
(//cli/prebuilt:registry) and writes one sha256 per tool and platform into
pins.bzl, so adding a tool here and to release.yml is all a new prebuilt needs.

A platform key is "<os>_<cpu>" in release asset spelling: os is linux, macos or
windows, cpu is x86_64 or arm64, and a jar that runs anywhere uses "any".
"""

visibility(["//cli/...", "//core/quality", "//tools/ci"])

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
        library = "",
    )

# Haskell tools: windows_arm64 runs the x86_64 build under emulation.
_HASKELL = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]

def _haskell(tool, role, source):
    return struct(
        asset = "dx-" + tool.replace("_", "-") + "-{os}-{cpu}",
        role = role,
        platforms = _HASKELL,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = source,
        member = "",
        main_class = "",
        library = "",
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
        library = "",
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
        library = "",
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
        library = "",
    )

def _library(tool, file, source, ext = ".dll", role = "exec", platforms = ["windows_arm64"]):
    return struct(
        asset = "dx-" + tool.replace("_", "-") + "-{os}-{cpu}" + ext,
        role = role,
        platforms = platforms,
        fallbacks = {},
        source = source,
        member = "",
        main_class = "",
        library = file,
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
#   library: the file name a shared library asset is loaded as; "" for an executable.
TOOLS = {
    "dx": struct(
        asset = "dx-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@rules_dx//cli/cli:dx",
        member = "",
        main_class = "",
        library = "",
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
        library = "",
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
        library = "",
    ),
    # Android and iOS targets have no asset and build the runtime from source.
    "runtime": struct(
        asset = "dx-runtime-{os}-{cpu}",
        role = "target",
        platforms = _NATIVE,
        fallbacks = {},
        source = "@rules_dx//cli/runtime:dx_runtime",
        member = "",
        main_class = "",
        library = "",
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
        library = "",
    ),
    "deadnix": _exec_native("deadnix"),
    "progenitor": _exec_native("progenitor"),
    "selene": _exec_native("selene"),
    "topiary": _exec_native("topiary"),
    "avro_union": _jar("avro_union", "@rules_dx//avro/private/tools/avro_union", "dx.avro.tools.AvroUnion"),
    "dokka_extract": _jar("dokka_extract", "@rules_dx//tools/docs/jvm/dokka:dokka_extract", "dx.docs.dokka.Main"),
    "javac_check": _jar("javac_check", "@rules_dx//java/private/quality/tools/javac_check", "dx.quality.javac.Main"),
    "javadoc_extract": _jar("javadoc_extract", "@rules_dx//tools/docs/jvm/javadoc:javadoc_extract", "dx.docs.javadoc.Main"),
    "jmh_generate": _jar("jmh_generate", "@rules_dx//java/private/jmh:jmh_generate", "dx.bench.JmhGenerate"),
    "maven_resolver": _jar("maven_resolver", "@rules_dx//java/private/tools/lock:resolver", "com.github.bazelbuild.rules_jvm_external.resolver.maven.MavenMain"),
    "kotlinc_check": _jar("kotlinc_check", "@rules_dx//java/private/quality/tools/kotlinc_check", "dx.quality.kotlinc.Main"),
    "scalac_check": _jar("scalac_check", "@rules_dx//java/private/quality/tools/scalac_check", "dx.quality.scalac.Main"),
    "scaladoc_extract": _jar("scaladoc_extract", "@rules_dx//tools/docs/jvm/scaladoc:scaladoc_extract", "dx.docs.scaladoc.Main"),
    "hpc_lcov": _haskell("hpc_lcov", "target", "@rules_dx//haskell/private/coverage:hpc_lcov_source"),
    "weeder": _haskell("weeder", "exec", "@rules_dx//haskell/private/tools/weeder/bin:source"),
    "tree_sitter": _library("tree_sitter", "tree-sitter.dll", "@jvm_tree_sitter//:tree-sitter.dll"),
    "tree_sitter_pkl": _library("tree_sitter_pkl", "tree-sitter-pkl.dll", "@jvm_tree_sitter_pkl//:tree-sitter-pkl.dll"),
    "pcov": _library("pcov", "pcov.so", "@dx_php_pcov//:pcov.so", ext = ".so", role = "target", platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"]),
    "hlint": _hackage("hlint", ["linux_arm64", "macos_arm64"]),
    "ormolu": _hackage("ormolu", ["linux_arm64"]),
    "nixfmt": struct(
        asset = "dx-nixfmt-{os}-{cpu}",
        role = "exec",
        platforms = ["linux_arm64", "macos_arm64"],
        fallbacks = {},
        source = "@rules_dx//core/quality/tools/nixfmt/bin:nixfmt_source",
        member = "",
        main_class = "",
        library = "",
    ),
    "kcov": struct(
        asset = "dx-kcov-{os}-{cpu}",
        role = "target",
        platforms = ["linux_arm64", "linux_x86_64"],
        fallbacks = {},
        source = "@rules_dx//tools/coverage/kcov:kcov",
        member = "",
        main_class = "",
        library = "",
    ),
    # One archive of Python and the PYTHON_BUNDLE_TOOLS; the hub serves its launchers.
    "python_tools": struct(
        asset = "dx-python-tools-{os}-{cpu}.tar.gz",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {},
        source = "",
        member = "",
        main_class = "",
        library = "",
    ),
}

_PYTHON_SOURCE = "@rules_dx//core/quality/tools/python:"

# The tools inside the python_tools bundle, by hub name: the launcher name in the
# bundle's bin/, the source build, and the platforms whose bundle leaves it out.
PYTHON_BUNDLE_TOOLS = {
    "ansible_lint_tool": struct(launcher = "ansible_lint", source = _PYTHON_SOURCE + "ansible_lint_tool", missing = ["windows_arm64"]),
    "cfn_lint": struct(launcher = "cfn_lint", source = _PYTHON_SOURCE + "cfn_lint", missing = []),
    "cmakelint": struct(launcher = "cmakelint", source = _PYTHON_SOURCE + "cmakelint", missing = []),
    "djlint": struct(launcher = "djlint", source = _PYTHON_SOURCE + "djlint", missing = []),
    "docstrfmt": struct(launcher = "docstrfmt", source = _PYTHON_SOURCE + "docstrfmt", missing = []),
    "mbake": struct(launcher = "mbake", source = _PYTHON_SOURCE + "mbake", missing = []),
    "sqlfluff": struct(launcher = "sqlfluff", source = _PYTHON_SOURCE + "sqlfluff", missing = []),
    "yamllint": struct(launcher = "yamllint", source = _PYTHON_SOURCE + "yamllint", missing = []),
}

# The tools whose source builds need rules_dx dev-only modules; a consumer hub serves
# this label on platforms with no pin instead.
DEV_SOURCE_FALLBACK = {"kcov": "@rules_dx//tools/coverage/kcov:none"}

# The tools released as one native binary or shared library per platform.
NATIVE_TOOLS = ["capnp", "deadnix", "flatc", "hlint", "hpc_lcov", "kcov", "launcher", "nixfmt", "oapi_codegen", "ormolu", "pcov", "progenitor", "runtime", "selene", "thrift", "topiary", "tree_sitter", "tree_sitter_pkl", "weeder"]

# The tools released as one platform-independent jar each.
JAR_TOOLS = sorted([tool for tool, entry in TOOLS.items() if entry.main_class])

def deploy_jar(tool):
    """Returns the deploy jar label of a jar tool, as the release jars job builds it."""
    source = Label(TOOLS[tool].source)
    return "//{}:{}_deploy.jar".format(source.package, source.name)

def split_key(key):
    """Returns (os, cpu) in release spelling for a platform key such as "linux_x86_64"."""
    os, _, cpu = key.partition("_")
    return os, cpu

def asset_name(tool, key):
    """Returns the release asset name of tool on the platform key."""
    os, cpu = split_key(key)
    return TOOLS[tool].asset.format(tool = tool, os = os, cpu = cpu)

def python_hub_lines():
    """Returns one "<name> <launcher> [<missing key>...]" line per PYTHON_BUNDLE_TOOLS entry, sorted."""
    return sorted([
        " ".join([name, tool.launcher] + sorted(tool.missing))
        for name, tool in PYTHON_BUNDLE_TOOLS.items()
    ])

def target_tools():
    """Returns the sorted names of the tools that run on the target platform."""
    return sorted([tool for tool, entry in TOOLS.items() if entry.role == "target"])

def registry_lines():
    """Returns one "<tool> <key> <asset>" line per release asset, sorted, for fill_pins."""
    return sorted([
        "{} {} {}".format(tool, key, asset_name(tool, key))
        for tool, entry in TOOLS.items()
        for key in entry.platforms
    ])
