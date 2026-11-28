"""The rules_dx tools a release ships prebuilt, and how a consumer finds each one."""

visibility(["//cli/...", "//core/quality", "//tools/ci"])

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

def _hackage(tool, platforms):
    return struct(
        asset = "dx-" + tool + "-{os}-{cpu}",
        role = "exec",
        platforms = platforms,
        fallbacks = {},
        source = "@rules_dx//haskell/private/tools/{}/bin:source".format(tool),
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

DX_SUBCOMMANDS = {
    "android_ops": "@rules_dx//android/private/packaging/ops:android_ops",
    "ansible_lock": "@rules_dx//ansible/private/tools/lock:ansible_lock",
    "api_spec_preflight": "@rules_dx//api_spec/private/codegen:api_spec_preflight",
    "appimage_packer": "@rules_dx//appimage/private/tools/packer:appimage_packer",
    "apple_ops": "@rules_dx//apple/private/tools/ops:apple_ops",
    "apple_post_action": "@rules_dx//tools/coverage/collect:apple_post_action_subcommand",
    "archiver": "@rules_dx//deploy/private:archiver",
    "bicep_lock": "@rules_dx//bicep/private/tools/lock:bicep_lock",
    "buf_diff": "@rules_dx//core/quality/tools/diff:buf_diff",
    "capnpc_rust_dx": "@rules_dx//capnproto/private/tools/capnpc_rust_dx:capnpc_rust_dx_subcommand",
    "capture": "@rules_dx//core/quality/capture:capture",
    "cloudflare_verify": "@rules_dx//deploy/private:cloudflare_verify",
    "compile_check": "@rules_dx//cli/compile_check:compile_check",
    "csharp_lock": "@rules_dx//csharp/private/tools/lock:csharp_lock",
    "d_lock": "@rules_dx//d/private/tools/lock:d_lock",
    "dart_lock": "@rules_dx//dart/private/tools/lock:dart_lock",
    "elixir_lock": "@rules_dx//elixir/private/tools/lock:elixir_lock",
    "fortran_lock": "@rules_dx//fortran/private/tools/lock:fortran_lock",
    "hasher": "@rules_dx//deploy/private:hasher",
    "haskell_lock": "@rules_dx//haskell/private/tools/lock:haskell_lock",
    "java_lock": "@rules_dx//java/private/tools/lock:java_lock",
    "lua_lock": "@rules_dx//lua/private/tools/lock:lua_lock",
    "npm_packer": "@rules_dx//deploy/private:npm_packer",
    "oapi_codegen": "@rules_dx//api_spec/private/codegen:oapi_codegen",
    "ocaml_dune": "@rules_dx//cli/ocaml_cc_driver:ocaml_dune",
    "ocaml_lock": "@rules_dx//ocaml/private/tools/lock:ocaml_lock",
    "perl_lock": "@rules_dx//perl/private/tools/lock:perl_lock",
    "php_lock": "@rules_dx//php/private/tools/lock:php_lock",
    "powershell_lock": "@rules_dx//powershell/private/tools/lock:powershell_lock",
    "qt_rust_bridge": "@rules_dx//qt/private/tools/rust:qt_rust_bridge_subcommand",
    "release_tool": "@rules_dx//deploy/private/release:release_tool_subcommand",
    "release_tree": "@rules_dx//cli/release_tree:release_tree_subcommand",
    "ruby_lock": "@rules_dx//ruby/private/tools/lock:ruby_lock",
    "size_checker": "@rules_dx//deploy/private:size_checker",
    "swift_lock": "@rules_dx//swift/private/tools/lock:swift_lock",
    "symbols_dsym": "@rules_dx//deploy/private/symbols:symbols_dsym",
    "symbols_pdb": "@rules_dx//deploy/private/symbols:symbols_pdb",
    "symbols_split": "@rules_dx//deploy/private/symbols:symbols_split",
    "terraform_lock": "@rules_dx//terraform/private/tools/lock:terraform_lock",
    "zig_lock": "@rules_dx//zig/private/tools/lock:zig_lock",
}

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
        source = "@dx_gazelle//:gazelle_source",
        member = "",
        main_class = "",
        library = "",
    ),
    "go": _native_exec("@rules_dx//go/private/tools/analysis/dx_go:dx_go"),
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
    "ldoc_main": struct(
        asset = "dx-ldoc-main-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@rules_dx//tools/docs/lua:ldoc_main",
        member = "",
        main_class = "",
        library = "",
    ),
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
    "gopackagesdriver": struct(
        asset = "dx-gopackagesdriver-{os}-{cpu}",
        role = "exec",
        platforms = _NATIVE,
        fallbacks = {"windows_arm64": "windows_x86_64"},
        source = "@rules_go//go/tools/gopackagesdriver",
        member = "",
        main_class = "",
        library = "",
    ),
    "msvc_link": struct(
        asset = "dx-msvc-link-{os}-{cpu}",
        role = "exec",
        platforms = ["windows_arm64", "windows_x86_64"],
        fallbacks = {},
        source = "@rules_dx//rust/private/tools/msvc_paths:msvc_link",
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
    "julia_internal": _library("julia_internal", "libjulia-internal.so", "@rules_dx//julia/private/runtime:libjulia_internal", ext = ".so", role = "target", platforms = ["linux_x86_64"]),
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

DEV_SOURCE_FALLBACK = {"kcov": "@rules_dx//tools/coverage/kcov:none"}

NATIVE_TOOLS = ["capnp", "deadnix", "flatc", "gopackagesdriver", "hlint", "hpc_lcov", "julia_internal", "kcov", "launcher", "ldoc_main", "msvc_link", "nixfmt", "oapi_codegen", "ormolu", "pcov", "progenitor", "runtime", "selene", "thrift", "topiary", "tree_sitter", "tree_sitter_pkl", "weeder"]

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
