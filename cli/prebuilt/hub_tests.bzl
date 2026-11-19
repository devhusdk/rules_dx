"""Tests for the @dx_prebuilt plan, pins and hub BUILD text."""

load("//core/private/testing:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_true", "starlark_test")
load(":hub.bzl", "GO_TOOLS", "SOURCE_LABEL", "SOURCE_TOOLS", "alias_build", "consumer_build", "plan_assets", "plan_jars", "plan_native_tools", "plan_root", "valid_sha256", "with_jars", "with_native_tools")
load(":pins.bzl", "DX_VERSION", "PINS", "SCHEMA", "URL_TEMPLATE")
load(":tools.bzl", "DEV_SOURCE_FALLBACK", "JAR_TOOLS", "NATIVE_TOOLS", "TOOLS", "asset_name", "registry_lines")

visibility("private")

_HASH = "0123456789abcdef" * 4

_PINNED = {"dx": {
    "linux_x86_64": _HASH,
    "windows_x86_64": _HASH,
}}

_ALL_PINNED = {"dx": {key: _HASH for key in TOOLS["dx"].platforms}}

def _override(url, os = "", cpu = ""):
    return struct(cpu = cpu, os = os, sha256 = _HASH, url = url)

def _mod(name, is_root, assets = [], from_source = False):
    return struct(name = name, is_root = is_root, tags = struct(asset = assets, from_source = [struct()] if from_source else []))

def _tag(sha256 = _HASH, name = "dx", os = "linux", cpu = "x86_64"):
    return struct(cpu = cpu, name = name, os = os, sha256 = sha256, url = "https://mirror/dx")

def _checks():
    middle = plan_root([_mod("app", True), _mod("middle", False, from_source = True)])
    badsha = plan_root([_mod("app", True, assets = [_tag("ABC")])])
    badname = plan_root([_mod("app", True, assets = [_tag(name = "dx-linux-x86_64")])])
    badcpu = plan_root([_mod("app", True, assets = [_tag(cpu = "arm64")])])
    twice = plan_root([_mod("app", True, assets = [_tag(), _tag()])])
    rooted = plan_root([_mod("rules_dx", True)])
    sourced = plan_root([_mod("app", True, from_source = True)])
    pinned_root = plan_root([_mod("app", True, assets = [_tag()]), _mod("rules_dx", False)])
    unpinned = plan_assets("dx", "1.2.3", {}, URL_TEMPLATE, {})
    pinned = plan_assets("dx", "1.2.3", _PINNED, URL_TEMPLATE, {})
    pinned_build = consumer_build(pinned)
    all_pinned_build = consumer_build(plan_assets("dx", "1.2.3", _ALL_PINNED, URL_TEMPLATE, {}))
    linux = [a for a in pinned if a.key == "linux_x86_64"][0]
    windows = [a for a in pinned if a.key == "windows_x86_64"][0]
    windows_arm = [a for a in pinned if a.key == "windows_aarch64"][0]
    mirrored = plan_assets("dx", "1.2.3", PINS, URL_TEMPLATE, {
        "linux_riscv64": _override("file:///dx", os = "linux", cpu = "riscv64"),
        "macos_aarch64": _override("https://mirror/dx", os = "macos", cpu = "aarch64"),
    })
    mirror = [a for a in mirrored if a.key == "macos_aarch64"][0]
    riscv = [a for a in mirrored if a.key == "linux_riscv64"][0]
    unpinned_build = consumer_build(unpinned)
    launchers = plan_native_tools("1.2.3", {"launcher": {"linux_x86_64": _HASH, "windows_x86_64": _HASH}}, URL_TEMPLATE, {})
    launcher_build = with_native_tools(unpinned_build, launchers, consumer = True)
    both_build = with_native_tools(pinned_build, launchers, consumer = True)
    launcher_root = plan_root([_mod("app", True, assets = [_tag(), _tag(name = "launcher")])], tools = ["dx"] + NATIVE_TOOLS)
    go_root = plan_root([_mod("app", True, assets = [_tag(name = "go")])], tools = ["dx", "go"])
    go_assets = plan_assets("go", "1.2.3", {}, URL_TEMPLATE, go_root.overrides["go"], exe = "dx-go", repo_prefix = "dx_go_prebuilt_")
    go_build = consumer_build(unpinned, go_assets)
    go_linux = [a for a in go_assets if a.key == "linux_x86_64"][0]
    go_all_pinned = plan_assets("go", "1.2.3", {"go": {key: _HASH for key in TOOLS["go"].platforms}}, URL_TEMPLATE, {}, exe = "dx-go", repo_prefix = "dx_go_prebuilt_")
    go_windows_arm = [a for a in go_all_pinned if a.key == "windows_aarch64"][0]
    source_jars = with_jars(alias_build(SOURCE_LABEL), plan_jars("1.2.3", {}, URL_TEMPLATE))
    pinned_jars = plan_jars("1.2.3", {"javac_check": {"any": _HASH}}, URL_TEMPLATE)
    pinned_jars_build = with_jars(pinned_build, pinned_jars)
    javac = [j for j in pinned_jars if j.tool == "javac_check"][0]
    jar_root = plan_root([_mod("app", True, assets = [_tag(name = "javac_check", os = "any", cpu = "any")])], tools = ["dx"] + JAR_TOOLS)
    jar_linux = plan_root([_mod("app", True, assets = [_tag(name = "javac_check")])], tools = ["dx"] + JAR_TOOLS)
    dx_any = plan_root([_mod("app", True, assets = [_tag(os = "any", cpu = "any")])], tools = ["dx"] + JAR_TOOLS)
    overridden_javac = [j for j in plan_jars("1.2.3", {}, URL_TEMPLATE, jar_root.overrides) if j.tool == "javac_check"][0]
    source_native = with_native_tools(alias_build(SOURCE_LABEL), plan_native_tools("1.2.3", {}, URL_TEMPLATE, {}))
    native = plan_native_tools("1.2.3", {"topiary": {"linux_x86_64": _HASH, "windows_x86_64": _HASH}}, URL_TEMPLATE, {})
    native_build = with_native_tools(pinned_build, native)
    flatc_build = with_native_tools(pinned_build, plan_native_tools("1.2.3", {"flatc": {"linux_x86_64": _HASH, "windows_x86_64": _HASH}}, URL_TEMPLATE, {}))
    flatc_root = plan_root([_mod("app", True, assets = [_tag(name = "flatc")])], tools = ["dx", "go"] + NATIVE_TOOLS)
    release_tools = plan_native_tools("1.2.3", {"deadnix": {"windows_x86_64": _HASH}, "kcov": {"linux_arm64": _HASH, "linux_x86_64": _HASH}}, URL_TEMPLATE, {})
    release_tools_build = with_native_tools(alias_build(SOURCE_LABEL), release_tools)
    consumer_source = with_native_tools(alias_build(SOURCE_LABEL), plan_native_tools("1.2.3", {}, URL_TEMPLATE, {}), consumer = True)
    consumer_pinned = with_native_tools(pinned_build, release_tools, consumer = True)
    topiary_windows_arm = [a for a in native["topiary"] if a.key == "windows_aarch64"][0]
    source_haskell = with_native_tools(alias_build(SOURCE_LABEL), plan_native_tools("1.2.3", {}, URL_TEMPLATE, {}))
    weeder_pins = {"weeder": {"linux_arm64": _HASH, "windows_x86_64": _HASH}}
    pinned_haskell = plan_native_tools("1.2.3", weeder_pins, URL_TEMPLATE, {})
    pinned_haskell_build = with_native_tools(alias_build(SOURCE_LABEL), pinned_haskell)
    weeder_windows_arm = [a for a in pinned_haskell["weeder"] if a.key == "windows_aarch64"][0]
    runtime = plan_native_tools("1.2.3", {"runtime": {"linux_x86_64": _HASH, "windows_x86_64": _HASH}}, URL_TEMPLATE, {})
    runtime_build = with_native_tools(pinned_build, runtime, consumer = True)
    runtime_root = plan_root([_mod("app", True, assets = [_tag(name = "runtime")])], tools = ["dx", "go"] + NATIVE_TOOLS)
    tree_sitter = plan_native_tools("1.2.3", {"tree_sitter": {"windows_arm64": _HASH}}, URL_TEMPLATE, {})
    tree_sitter_build = with_native_tools(alias_build(SOURCE_LABEL), tree_sitter, consumer = True)
    tree_sitter_windows_arm = tree_sitter["tree_sitter"][0]
    tree_sitter_root = plan_root([_mod("app", True, assets = [_tag(name = "tree_sitter", os = "windows", cpu = "aarch64")])], tools = ["dx", "go"] + NATIVE_TOOLS)
    return [
        expect_equal("tree-sitter assets", [asset_name(t, k) for t in ["tree_sitter", "tree_sitter_pkl"] for k in TOOLS[t].platforms], ["dx-tree-sitter-windows-arm64.dll", "dx-tree-sitter-pkl-windows-arm64.dll"]),
        expect_contains("the registry lists tree-sitter for fill_pins", registry_lines(), "tree_sitter windows_arm64 dx-tree-sitter-windows-arm64.dll"),
        expect_contains("the registry lists tree-sitter-pkl for fill_pins", registry_lines(), "tree_sitter_pkl windows_arm64 dx-tree-sitter-pkl-windows-arm64.dll"),
        expect_contains("a pinned tree-sitter selects the downloaded file", tree_sitter_build, '":native_windows_aarch64": "@dx_tree_sitter_prebuilt_windows_aarch64//file:tree-sitter.dll",'),
        expect_false("a library gets no native_binary", "tree_sitter_windows_aarch64" in tree_sitter_build),
        expect_false("a library-only hub loads no native_binary", "native_binary" in tree_sitter_build),
        expect_contains("a pinned tree-sitter builds from source elsewhere", tree_sitter_build, '"//conditions:default": "@jvm_tree_sitter//:tree-sitter.dll",'),
        expect_contains("an unpinned tree-sitter-pkl aliases its source", tree_sitter_build, 'name = "tree_sitter_pkl",\n    actual = "@jvm_tree_sitter_pkl//:tree-sitter-pkl.dll",'),
        expect_equal("a tree-sitter download keeps the loaded file name", tree_sitter_windows_arm.exe, "tree-sitter.dll"),
        expect_false("a tree-sitter download is not executable", tree_sitter_windows_arm.executable),
        expect_true("a native tool download is executable", topiary_windows_arm.executable),
        expect_equal("a root asset may name tree-sitter", tree_sitter_root.overrides.keys(), ["tree_sitter"]),
        expect_equal("the native tools", NATIVE_TOOLS, ["capnp", "deadnix", "flatc", "hlint", "hpc_lcov", "kcov", "launcher", "nixfmt", "oapi_codegen", "ormolu", "progenitor", "runtime", "selene", "thrift", "topiary", "tree_sitter", "tree_sitter_pkl", "weeder"]),
        expect_equal("runtime assets", [asset_name("runtime", k) for k in TOOLS["runtime"].platforms], ["dx-runtime-linux-arm64", "dx-runtime-linux-x86_64", "dx-runtime-macos-arm64", "dx-runtime-windows-arm64", "dx-runtime-windows-x86_64"]),
        expect_equal("the runtime runs on the target platform", TOOLS["runtime"].role, "target"),
        expect_contains("the registry lists the runtime for fill_pins", registry_lines(), "runtime windows_arm64 dx-runtime-windows-arm64"),
        expect_contains("an unpinned runtime builds from source", source_native, 'name = "runtime",\n    actual = "@rules_dx//cli/runtime:dx_runtime",'),
        expect_contains("a pinned runtime selects its target platform", runtime_build, '":native_linux_x86_64": ":runtime_linux_x86_64",'),
        expect_contains("a pinned windows runtime runs its download", runtime_build, 'name = "runtime_windows_x86_64",\n    src = "@dx_runtime_prebuilt_windows_x86_64//file:runtime.exe",'),
        expect_contains("an unpinned runtime platform builds from source", runtime_build, '"//conditions:default": "@rules_dx//cli/runtime:dx_runtime",'),
        expect_equal("an unpinned windows arm64 runtime takes no x86_64 download", [a.repo for a in runtime["runtime"] if a.key == "windows_aarch64"], [""]),
        expect_false("an unpinned windows arm64 runtime builds from source", "runtime_windows_aarch64" in runtime_build),
        expect_equal("a root asset may name the runtime", runtime_root.overrides.keys(), ["runtime"]),
        expect_equal("oapi-codegen assets", [asset_name("oapi_codegen", k) for k in TOOLS["oapi_codegen"].platforms], ["dx-oapi-codegen-linux-arm64", "dx-oapi-codegen-linux-x86_64", "dx-oapi-codegen-macos-arm64", "dx-oapi-codegen-windows-arm64", "dx-oapi-codegen-windows-x86_64"]),
        expect_contains("the registry lists oapi-codegen for fill_pins", registry_lines(), "oapi_codegen linux_arm64 dx-oapi-codegen-linux-arm64"),
        expect_equal("capnp assets", [asset_name("capnp", k) for k in TOOLS["capnp"].platforms], ["dx-capnp-linux-arm64", "dx-capnp-linux-x86_64", "dx-capnp-macos-arm64", "dx-capnp-windows-arm64", "dx-capnp-windows-x86_64"]),
        expect_contains("the registry lists the code generators for fill_pins", registry_lines(), "thrift macos_arm64 dx-thrift-macos-arm64"),
        expect_equal("native tools run on the exec platform except hpc_lcov, kcov, launcher and runtime", [t for t in NATIVE_TOOLS if TOOLS[t].role != "exec"], ["hpc_lcov", "kcov", "launcher", "runtime"]),
        expect_equal("release tool assets", [asset_name(t, k) for t in ["deadnix", "selene", "nixfmt", "kcov"] for k in TOOLS[t].platforms], ["dx-deadnix-linux-arm64", "dx-deadnix-linux-x86_64", "dx-deadnix-macos-arm64", "dx-deadnix-windows-x86_64", "dx-selene-linux-arm64", "dx-selene-linux-x86_64", "dx-selene-macos-arm64", "dx-selene-windows-x86_64", "dx-nixfmt-linux-arm64", "dx-nixfmt-macos-arm64", "dx-kcov-linux-arm64", "dx-kcov-linux-x86_64"]),
        expect_equal("python bundle assets", [asset_name("python_tools", k) for k in TOOLS["python_tools"].platforms], ["dx-python-tools-linux-arm64.tar.gz", "dx-python-tools-linux-x86_64.tar.gz", "dx-python-tools-macos-arm64.tar.gz", "dx-python-tools-windows-arm64.tar.gz", "dx-python-tools-windows-x86_64.tar.gz"]),
        expect_contains("the registry lists the python bundle for fill_pins", registry_lines(), "python_tools windows_arm64 dx-python-tools-windows-arm64.tar.gz"),
        expect_false("the hub has no python bundle target", 'name = "python_tools"' in with_native_tools(pinned_build, plan_native_tools("1.2.3", {"python_tools": {"linux_x86_64": _HASH}}, URL_TEMPLATE, {}))),
        expect_equal("windows arm64 runs the x86_64 deadnix", [a.repo for a in release_tools["deadnix"] if a.key == "windows_aarch64"], ["dx_deadnix_prebuilt_windows_x86_64"]),
        expect_contains("a pinned kcov selects its linux download", release_tools_build, '":native_linux_aarch64": ":kcov_linux_aarch64",'),
        expect_equal("kcov plans only linux", [a.key for a in release_tools["kcov"]], ["linux_aarch64", "linux_x86_64"]),
        expect_contains("an unpinned deadnix builds from source", source_native, 'name = "deadnix",\n    actual = "@rules_dx//rust/private/quality/tools:deadnix_source",'),
        expect_contains("an unpinned selene builds from source", source_native, 'name = "selene",\n    actual = "@rules_dx//rust/private/quality/tools:selene_source",'),
        expect_contains("an unpinned nixfmt builds from source", source_native, 'name = "nixfmt",\n    actual = "@rules_dx//core/quality/tools/nixfmt/bin:nixfmt_source",'),
        expect_contains("an unpinned kcov builds from source", source_native, 'name = "kcov",\n    actual = "@rules_dx//tools/coverage/kcov:kcov",'),
        expect_contains("a consumer hub with no kcov pin serves no kcov", consumer_source, 'name = "kcov",\n    actual = "@rules_dx//tools/coverage/kcov:none",'),
        expect_false("a consumer hub never names the kcov source build", "tools/coverage/kcov:kcov" in consumer_source + consumer_pinned),
        expect_contains("a pinned consumer kcov serves no kcov on other platforms", consumer_pinned, '"//conditions:default": "@rules_dx//tools/coverage/kcov:none",'),
        expect_contains("a consumer hub builds deadnix from source", consumer_pinned, '"//conditions:default": "@rules_dx//rust/private/quality/tools:deadnix_source",'),
        expect_equal("every dev source fallback is a native tool", [t for t in DEV_SOURCE_FALLBACK if t not in NATIVE_TOOLS], []),
        expect_contains("a pinned code generator runs its download", flatc_build, 'name = "flatc_linux_x86_64",\n    src = "@dx_flatc_prebuilt_linux_x86_64//file:flatc",\n    out = "linux_x86_64/flatc",'),
        expect_contains("an unpinned code generator platform builds from source", flatc_build, '"//conditions:default": "@dx_flatbuffers//:flatc",'),
        expect_contains("a code generator with no pins builds from source", flatc_build, 'name = "capnp",\n    actual = "@dx_capnproto//src/capnp:capnp_tool",'),
        expect_contains("the root hub aliases the source code generators", source_native, 'name = "thrift",\n    actual = "@dx_thrift//:thrift_compiler",'),
        expect_equal("a root asset may name a code generator", flatc_root.overrides.keys(), ["flatc"]),
        expect_equal("topiary assets", [asset_name("topiary", k) for k in TOOLS["topiary"].platforms], ["dx-topiary-linux-arm64", "dx-topiary-linux-x86_64", "dx-topiary-macos-arm64", "dx-topiary-windows-x86_64"]),
        expect_contains("the registry lists progenitor for fill_pins", registry_lines(), "progenitor macos_arm64 dx-progenitor-macos-arm64"),
        expect_equal("an unpinned native tool plans no windows arm64", [a.key for a in plan_native_tools("1.2.3", {}, URL_TEMPLATE, {})["topiary"]], ["linux_aarch64", "linux_x86_64", "macos_aarch64", "windows_x86_64"]),
        expect_equal("windows arm64 runs the x86_64 topiary", topiary_windows_arm.repo, "dx_topiary_prebuilt_windows_x86_64"),
        expect_contains("windows arm64 selects the shared download", native_build, 'name = "topiary_windows_aarch64",\n    src = "@dx_topiary_prebuilt_windows_x86_64//file:topiary.exe",'),
        expect_contains("a pinned native tool selects on the exec platform", native_build, '":native_linux_x86_64": ":topiary_linux_x86_64",'),
        expect_contains("an unpinned native platform builds from source", native_build, '"//conditions:default": "@rules_dx//rust/private/quality/tools:topiary_source",'),
        expect_contains("an unpinned native tool builds from source", native_build, 'name = "progenitor",\n    actual = "@rules_dx//rust/private/quality/tools:progenitor_source",'),
        expect_equal("native tools share one config_setting per platform", native_build.count('name = "native_linux_x86_64",'), 1),
        expect_equal("the native hub loads native_binary once", native_build.count("native_binary.bzl"), 1),
        expect_false("a source hub selects no native download", "dx_topiary_prebuilt" in source_native),
        expect_true("a pinned native tool loads native_binary", with_native_tools(alias_build(SOURCE_LABEL), native).startswith('load("@bazel_skylib//rules:native_binary.bzl", "native_binary")\n')),
        expect_equal("haskell assets", [asset_name(t, k) for t in ["hpc_lcov", "weeder"] for k in TOOLS[t].platforms], ["dx-hpc-lcov-linux-arm64", "dx-hpc-lcov-linux-x86_64", "dx-hpc-lcov-macos-arm64", "dx-hpc-lcov-windows-x86_64", "dx-weeder-linux-arm64", "dx-weeder-linux-x86_64", "dx-weeder-macos-arm64", "dx-weeder-windows-x86_64"]),
        expect_contains("the registry lists the haskell tools for fill_pins", registry_lines(), "weeder macos_arm64 dx-weeder-macos-arm64"),
        expect_equal("hpc_lcov runs on the target platform", TOOLS["hpc_lcov"].role, "target"),
        expect_equal("weeder runs on the exec platform", TOOLS["weeder"].role, "exec"),
        expect_equal("hackage tool assets", [asset_name(t, k) for t in ["hlint", "ormolu"] for k in TOOLS[t].platforms], ["dx-hlint-linux-arm64", "dx-hlint-macos-arm64", "dx-ormolu-linux-arm64"]),
        expect_equal("hackage tools run on the exec platform", [TOOLS[t].role for t in ["hlint", "ormolu"]], ["exec", "exec"]),
        expect_contains("an unpinned hackage tool builds from source", source_haskell, 'name = "hlint",\n    actual = "@rules_dx//haskell/private/tools/hlint/bin:source",'),
        expect_contains("ormolu builds from its source alias", source_haskell, 'name = "ormolu",\n    actual = "@rules_dx//haskell/private/tools/ormolu/bin:source",'),
        expect_equal("hackage tools plan no windows arm64", [a.key for t in ["hlint", "ormolu"] for a in plan_native_tools("1.2.3", {t: {k: _HASH for k in TOOLS[t].platforms}}, URL_TEMPLATE, {})[t] if a.key == "windows_aarch64"], []),
        expect_contains("an unpinned haskell tool builds from source", source_haskell, 'name = "weeder",\n    actual = "@rules_dx//haskell/private/tools/weeder/bin:source",'),
        expect_false("a source hub downloads no haskell tool", "dx_weeder_prebuilt_" in source_haskell),
        expect_equal("windows arm64 takes the x86_64 haskell download", weeder_windows_arm.repo, "dx_weeder_prebuilt_windows_x86_64"),
        expect_equal("windows arm64 runs the x86_64 exe", weeder_windows_arm.exe, "weeder.exe"),
        expect_contains("a pinned haskell tool selects its platform", pinned_haskell_build, '":native_linux_aarch64": ":weeder_linux_aarch64",'),
        expect_contains("windows arm64 runs the shared haskell download", pinned_haskell_build, 'name = "weeder_windows_aarch64",\n    src = "@dx_weeder_prebuilt_windows_x86_64//file:weeder.exe",'),
        expect_contains("an unpinned haskell platform builds from source", pinned_haskell_build, '"//conditions:default": "@rules_dx//haskell/private/tools/weeder/bin:source",'),
        expect_true("a pinned haskell hub loads native_binary", pinned_haskell_build.startswith('load("@bazel_skylib//rules:native_binary.bzl", "native_binary")\n')),
        expect_equal("native_binary is loaded once", with_native_tools(pinned_build, pinned_haskell).count("native_binary.bzl"), 1),
        expect_false("an unpinned haskell tool downloads nothing", "dx_hpc_lcov_prebuilt_" in pinned_haskell_build),
        expect_equal("the jar tools", JAR_TOOLS, ["avro_union", "dokka_extract", "javac_check", "javadoc_extract", "jmh_generate", "kotlinc_check", "scalac_check", "scaladoc_extract"]),
        expect_equal("a jar is one any asset", [asset_name(t, k) for t in JAR_TOOLS for k in TOOLS[t].platforms], ["dx-avro-union-any.jar", "dx-dokka-extract-any.jar", "dx-javac-check-any.jar", "dx-javadoc-extract-any.jar", "dx-jmh-generate-any.jar", "dx-kotlinc-check-any.jar", "dx-scalac-check-any.jar", "dx-scaladoc-extract-any.jar"]),
        expect_contains("the registry lists the jars for fill_pins", registry_lines(), "javac_check any dx-javac-check-any.jar"),
        expect_equal("a pinned jar downloads its release", javac.url, "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-javac-check-any.jar"),
        expect_equal("unpinned jars download nothing", [j.repo for j in pinned_jars if j.tool != "javac_check"], [""] * 7),
        expect_contains("an unpinned jar tool builds from source", source_jars, 'name = "scalac_check",\n    actual = "@rules_dx//java/private/quality/tools/scalac_check",'),
        expect_false("a source hub loads no java rules", "rules_java" in source_jars),
        expect_true("a pinned jar hub loads the java rules first", pinned_jars_build.startswith('load("@rules_java//java:defs.bzl", "java_binary", "java_import")\n')),
        expect_contains("a pinned jar is imported", pinned_jars_build, 'name = "javac_check_jar",\n    jars = ["@dx_prebuilt_javac_check//file:dx-javac-check-any.jar"],'),
        expect_contains("a pinned jar runs its main class", pinned_jars_build, 'name = "javac_check",\n    main_class = "dx.quality.javac.Main",'),
        expect_false("a pinned jar never names its source", TOOLS["javac_check"].source in pinned_jars_build),
        expect_equal("pins.bzl is schema 1", SCHEMA, 1),
        expect_equal("pins name only registry tools and platforms", [(t, k) for t, by_key in PINS.items() for k in by_key if t not in TOOLS or k not in TOOLS[t].platforms], []),
        expect_equal("pins are empty or a sha256", [(t, k) for t, by_key in PINS.items() for k, v in by_key.items() if not valid_sha256(v)], []),
        expect_equal("dx assets keep their release names", [asset_name("dx", k) for k in TOOLS["dx"].platforms], ["dx-linux-arm64", "dx-linux-x86_64", "dx-macos-arm64", "dx-windows-arm64", "dx-windows-x86_64"]),
        expect_contains("the registry lists gazelle for fill_pins", registry_lines(), "gazelle windows_x86_64 dx-gazelle-windows-x86_64"),
        expect_equal("a fallback takes the x86_64 download", windows_arm.repo, "dx_prebuilt_windows_x86_64"),
        expect_equal("a fallback keeps its own platform", windows_arm.key, "windows_aarch64"),
        expect_contains("a fallback platform runs the shared download", pinned_build, 'name = "dx_windows_aarch64",\n    src = "@dx_prebuilt_windows_x86_64//file:dx.exe",'),
        expect_true("the version is semver", len(DX_VERSION.split(".")) == 3),
        expect_contains("the url names the version tag", URL_TEMPLATE, "/download/v{version}/{asset}"),
        expect_false("an uppercase hash is invalid", valid_sha256(_HASH.upper())),
        expect_false("a short hash is invalid", valid_sha256("abc")),
        expect_equal("repo names", windows.repo, "dx_prebuilt_windows_x86_64"),
        expect_equal("unpinned assets get no repo", [a.repo for a in unpinned], [""] * 5),
        expect_equal("unpinned assets get no url", [a.url for a in unpinned], [""] * 5),
        expect_equal("a pinned asset downloads its release", linux.url, "https://github.com/devhusdk/rules_dx/releases/download/v1.2.3/dx-linux-x86_64"),
        expect_equal("a pinned asset keeps its hash", linux.sha256, _HASH),
        expect_equal("linux runs dx", linux.exe, "dx"),
        expect_equal("windows runs dx.exe", windows.exe, "dx.exe"),
        expect_equal("an override replaces the url", mirror.url, "https://mirror/dx"),
        expect_equal("an override pins its hash", mirror.repo, "dx_prebuilt_macos_aarch64"),
        expect_equal("a new asset takes its platform", riscv.key, "linux_riscv64"),
        expect_contains("the hub selects a platform", pinned_build, '":linux_x86_64": ":dx_linux_x86_64",'),
        expect_contains("the hub runs the download", pinned_build, 'src = "@dx_prebuilt_linux_x86_64//file:dx",'),
        expect_contains("windows keeps .exe", pinned_build, 'out = "windows_x86_64/dx.exe",'),
        expect_contains("other platforms fail", pinned_build, '"//conditions:default": ":unsupported",'),
        expect_contains("the failure names the remedies", pinned_build, "dx_prebuilt.from_source(), which requires Rust and C toolchains"),
        expect_contains("the failure lists supported platforms", pinned_build, "linux x86_64, macos aarch64"),
        expect_equal("a release with no pins builds dx from source", unpinned_build, alias_build(SOURCE_LABEL)),
        expect_contains("an unpinned asset builds dx from source", pinned_build, '":linux_aarch64": "@rules_dx//cli/cli:dx",'),
        expect_false("an unpinned asset downloads nothing", "dx_prebuilt_linux_aarch64" in pinned_build),
        expect_false("a fully pinned hub never names the source dx", SOURCE_LABEL in all_pinned_build),
        expect_equal("a fully pinned hub never names a source tool", [t for t in SOURCE_TOOLS.values() if t in all_pinned_build], []),
        expect_contains("a pinned platform runs dx as the runner", pinned_build, '":linux_x86_64": ":quality_runner_linux_x86_64",'),
        expect_contains("the runner is the downloaded dx", pinned_build, 'name = "quality_runner_linux_x86_64",\n    src = "@dx_prebuilt_linux_x86_64//file:dx",\n    out = "linux_x86_64/quality_runner",'),
        expect_contains("a windows tool keeps .exe", pinned_build, 'out = "windows_x86_64/quality_notebook.exe",'),
        expect_contains("the rule helpers are the downloaded dx", pinned_build, 'name = "env_shard_writer_linux_x86_64",\n    src = "@dx_prebuilt_linux_x86_64//file:dx",\n    out = "linux_x86_64/env_shard_writer",'),
        expect_contains("a windows rule helper keeps .exe", pinned_build, 'out = "windows_x86_64/codegen_shard_writer.exe",'),
        expect_contains("the coverage collector is the downloaded dx", pinned_build, 'out = "linux_x86_64/collect_native",'),
        expect_contains("the lsp driver is the downloaded dx", pinned_build, 'out = "linux_x86_64/lsp_check",'),
        expect_contains("the deploy tool is the downloaded dx", pinned_build, 'out = "linux_x86_64/deploy_tool",'),
        expect_contains("an unpinned hub aliases the source deploy tool", unpinned_build, 'actual = "@rules_dx//deploy/private:deploy_tool",'),
        expect_contains("an unpinned hub aliases the source shard writers", unpinned_build, 'actual = "@rules_dx//cli/codegen_shard:codegen_shard_writer",'),
        expect_contains("an unpinned platform builds the tool from source", pinned_build, '":linux_aarch64": "@rules_dx//core/quality/markdown:quality_markdown",'),
        expect_contains("the root hub aliases the source runner", alias_build(SOURCE_LABEL), 'name = "quality_runner",\n    actual = "@rules_dx//core/quality/runner:quality_runner",'),
        expect_contains("an unpinned hub aliases the source notebook tool", unpinned_build, 'actual = "@rules_dx//core/quality/notebook:quality_notebook",'),
        expect_contains("an unpinned hub aliases the python quality tools", unpinned_build, 'actual = "@rules_dx//core/quality/tools/python:yamllint",'),
        expect_contains("a pinned hub aliases the python quality tools", pinned_build, 'name = "python_tool_yamllint",'),
        expect_false("the consumer hub loads nothing from rules_rust", "rules_rust" in all_pinned_build),
        expect_contains("the root hub aliases the source dx", alias_build(SOURCE_LABEL), 'actual = "@rules_dx//cli/cli:dx",'),
        expect_contains("a non-root module cannot set tags", middle.error, "only the root module may set asset or from_source tags; middle sets them"),
        expect_contains("a bad sha names its asset", badsha.error, 'dx_prebuilt.asset(name = "dx", os = "linux", cpu = "x86_64"): sha256 must be 64 lowercase hex digits'),
        expect_contains("an asset name is the tool", badname.error, 'name is one of the tools ["dx"]'),
        expect_contains("cpu uses @platforms spelling", badcpu.error, "cpu one of"),
        expect_contains("one asset per os and cpu", twice.error, "a second asset for the same os and cpu"),
        expect_equal("a jar asset takes any os and cpu", (jar_root.error, jar_root.overrides["javac_check"].keys()), ("", ["any"])),
        expect_contains("a jar asset rejects a native os", jar_linux.error, 'dx_prebuilt.asset(name = "javac_check", os = "linux", cpu = "x86_64"): a jar tool takes os = "any" and cpu = "any"'),
        expect_contains("a native asset rejects any", dx_any.error, "os must be one of"),
        expect_equal("an overridden jar keeps its asset and downloads the override", (overridden_javac.asset, overridden_javac.repo, overridden_javac.url), ("dx-javac-check-any.jar", "dx_prebuilt_javac_check", "https://mirror/dx")),
        expect_true("from_source builds from source", sourced.source),
        expect_true("rules_dx as root builds from source", rooted.source),
        expect_false("a consumer root does not build from source", pinned_root.source),
        expect_equal("a root asset is an override", pinned_root.overrides["dx"].keys(), ["linux_x86_64"]),
        expect_equal("a root launcher asset overrides only the launcher", sorted(launcher_root.overrides.keys()), ["dx", "launcher"]),
        expect_equal("the launcher is a target-platform asset", TOOLS["launcher"].role, "target"),
        expect_equal("launcher assets", [asset_name("launcher", k) for k in TOOLS["launcher"].platforms], ["dx-launcher-linux-arm64", "dx-launcher-linux-x86_64", "dx-launcher-macos-arm64", "dx-launcher-windows-arm64", "dx-launcher-windows-x86_64"]),
        expect_contains("the root hub builds the launcher from source", source_native, 'name = "launcher",\n    actual = "@rules_dx//cli/launcher:exec_launcher",'),
        expect_contains("an unpinned consumer hub builds the launcher from source", consumer_source, 'name = "launcher",\n    actual = "@rules_dx//cli/launcher:exec_launcher",'),
        expect_equal("the hub serves the launcher once", both_build.count('name = "launcher",'), 1),
        expect_contains("a pinned launcher runs its download", launcher_build, 'name = "launcher_linux_x86_64",\n    src = "@dx_launcher_prebuilt_linux_x86_64//file:launcher",\n    out = "linux_x86_64/launcher",'),
        expect_contains("a windows launcher keeps .exe", launcher_build, 'out = "windows_x86_64/launcher.exe",'),
        expect_contains("the launcher selects its platform", launcher_build, '":native_linux_x86_64": ":launcher_linux_x86_64",'),
        expect_contains("other targets build the launcher from source", launcher_build, '"//conditions:default": "@rules_dx//cli/launcher:exec_launcher",'),
        expect_equal("android and ios have no launcher download", [a.key for a in launchers["launcher"] if a.os in ["android", "ios"]], []),
        expect_false("an unpinned launcher platform downloads nothing", "dx_launcher_prebuilt_linux_aarch64" in launcher_build),
        expect_equal("a valid root has no error", pinned_root.error, ""),
        expect_equal("a release builds dx-go for every platform", [line for line in registry_lines() if line.startswith("go ")], ["go linux_arm64 dx-go-linux-arm64", "go linux_x86_64 dx-go-linux-x86_64", "go macos_arm64 dx-go-macos-arm64", "go windows_arm64 dx-go-windows-arm64", "go windows_x86_64 dx-go-windows-x86_64"]),
        expect_equal("an unpinned go downloads only its overrides", [a.key for a in go_assets if a.repo], ["linux_x86_64"]),
        expect_equal("a go override downloads dx-go", go_linux.repo, "dx_go_prebuilt_linux_x86_64"),
        expect_equal("a pinned windows arm64 downloads its own dx-go", go_windows_arm.repo, "dx_go_prebuilt_windows_aarch64"),
        expect_contains("a go override declares its download", go_build, 'name = "dx_govet_linux_x86_64",\n    src = "@dx_go_prebuilt_linux_x86_64//file:dx-go",\n    out = "linux_x86_64/dx_govet",'),
        expect_contains("a go tool selects its download", go_build, '":linux_x86_64": ":dx_staticcheck_linux_x86_64",'),
        expect_contains("other platforms build the go tool from source", go_build, '"//conditions:default": "{}",'.format(GO_TOOLS["dx_staticcheck"])),
        expect_contains("an unpinned hub aliases the source go tools", unpinned_build, 'name = "dx_govet",\n    actual = "@rules_dx//go/private/tools/analysis/dx_go:dx_govet",'),
        expect_contains("the root hub aliases the source go tools", alias_build(SOURCE_LABEL), 'name = "dx_modfmt",\n    actual = "@rules_dx//go/private/tools/analysis/dx_go:dx_modfmt",'),
        expect_equal("the go multicall answers to seven names", sorted(GO_TOOLS.keys()), ["dx_errcheck", "dx_godoc_json", "dx_gofumpt", "dx_gotype", "dx_govet", "dx_modfmt", "dx_staticcheck"]),
    ]

def dx_prebuilt_hub_tests(name):
    """Declares the @dx_prebuilt hub test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
