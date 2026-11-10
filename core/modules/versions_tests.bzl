"""Canonical version pins stay equal to MODULE.bazel literals."""

load("@dx_pins_data//:pins.bzl", _PINS = "PINS")
load("//dart/private:pins.bzl", _DART_STRIP_PREFIX = "RULES_DART_STRIP_PREFIX", _DART_URL = "RULES_DART_URL")
load("//dotnet:pins.bzl", _DOTNET_STRIP_PREFIX = "RULES_DOTNET_STRIP_PREFIX", _DOTNET_URL = "RULES_DOTNET_URL")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":cuda.bzl", _CUDA_CELLS = "CUDA_CELLS", _CUDA_COMPONENTS = "CUDA_COMPONENTS", _CUDA_MAX_HOST_CLANG_MAJOR = "CUDA_MAX_HOST_CLANG_MAJOR", _CUDA_REDIST_SHA256 = "CUDA_REDIST_SHA256", _CUDA_REDIST_URL = "CUDA_REDIST_URL", _CUDA_REDIST_VERSION = "CUDA_REDIST_VERSION", _MOD_CUDA_VERSION = "CUDA_VERSION", _MOD_RULES_CUDA_VERSION = "RULES_CUDA_VERSION", _cuda_qualified_cells = "cuda_qualified_cells")
load(":python.bzl", _MOD_PYTHON_VERSION = "PYTHON_VERSION")
load(":ruby.bzl", _GEM_CHECKSUMS = "GEM_CHECKSUMS", _MOD_RUBY_VERSION = "RUBY_VERSION", _PORTABLE_RUBY_CHECKSUMS = "PORTABLE_RUBY_CHECKSUMS")
load(":rust.bzl", _CRATE_BAZEL_LOCK = "CRATE_BAZEL_LOCK", _CRATE_CARGO_LOCK = "CRATE_CARGO_LOCK", _MOD_RUST_VERSION = "RUST_VERSION", _RUST_CRATE_MANIFESTS = "RUST_CRATE_MANIFESTS")
load(":versions.bzl", _APPLE_SUPPORT_VERSION = "TESTED_APPLE_SUPPORT", _ASPECT_RULES_JEST_VERSION = "ASPECT_RULES_JEST_VERSION", _ASPECT_RULES_JS_VERSION = "ASPECT_RULES_JS_VERSION", _ASPECT_RULES_TS_VERSION = "ASPECT_RULES_TS_VERSION", _BAZEL_LIB_VERSION = "BAZEL_LIB_VERSION", _BAZEL_SKYLIB_VERSION = "BAZEL_SKYLIB_VERSION", _CUDA_VERSION = "CUDA_VERSION", _DART_SDK_VERSION = "DART_SDK_VERSION", _DOTNET_VERSION = "DOTNET_VERSION", _GAZELLE_VERSION = "GAZELLE_VERSION", _GOOGLETEST_VERSION = "GOOGLETEST_VERSION", _GO_LANGUAGE_FLOOR = "GO_LANGUAGE_FLOOR", _GO_SDK_SDKS = "GO_SDK_SDKS", _GO_SDK_VERSION = "GO_SDK_VERSION", _JAVA_RUNTIME_VERSION = "JAVA_RUNTIME_VERSION", _JULIA_VERSION = "JULIA_VERSION", _JVM_TOOL_VERSIONS = "JVM_TOOL_VERSIONS", _LLVM_VERSION = "LLVM_VERSION", _LUA_VERSION = "LUA_VERSION", _NPM_DEV_HUBS = "NPM_DEV_HUBS", _NPM_HUBS = "NPM_HUBS", _NPM_LOCKS = "NPM_LOCKS", _PLATFORMS_VERSION = "PLATFORMS_VERSION", _PNPM_VERSION = "PNPM_VERSION", _PROTOBUF_VERSION = "TESTED_PROTOBUF", _PYTHON_VERSION = "PYTHON_VERSION", _RUBY_VERSION = "RUBY_VERSION", _RULES_ANDROID_VERSION = "RULES_ANDROID_VERSION", _RULES_APPLE_VERSION = "TESTED_RULES_APPLE", _RULES_CC_VERSION = "RULES_CC_VERSION", _RULES_CUDA_VERSION = "RULES_CUDA_VERSION", _RULES_DART_VERSION = "RULES_DART_VERSION", _RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION", _RULES_GO_VERSION = "TESTED_RULES_GO", _RULES_JAVA_VERSION = "RULES_JAVA_VERSION", _RULES_JULIA_VERSION = "RULES_JULIA_VERSION", _RULES_JVM_EXTERNAL_VERSION = "TESTED_RULES_JVM_EXTERNAL", _RULES_KOTLIN_VERSION = "TESTED_RULES_KOTLIN", _RULES_NODEJS_VERSION = "RULES_NODEJS_VERSION", _RULES_PERL_VERSION = "RULES_PERL_VERSION", _RULES_PROTO_VERSION = "RULES_PROTO_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION", _RULES_PY_VERSION = "RULES_PY_VERSION", _RULES_RUST_VERSION = "RULES_RUST_VERSION", _RULES_RUST_WASM_BINDGEN_VERSION = "RULES_RUST_WASM_BINDGEN_VERSION", _RULES_SCALA_VERSION = "TESTED_RULES_SCALA", _RULES_SHELL_VERSION = "RULES_SHELL_VERSION", _RULES_SWIFT_VERSION = "RULES_SWIFT_VERSION", _RULES_ZIG_VERSION = "RULES_ZIG_VERSION", _RUSTFMT_VERSION = "RUSTFMT_VERSION", _RUST_EDITION = "RUST_EDITION", _RUST_VERSION = "RUST_VERSION", _SCALA_VERSION = "SCALA_VERSION", _SWIFT_VERSION = "SWIFT_VERSION", _TYPEDOC_VERSION = "TYPEDOC_VERSION", _TYPESCRIPT_INTEGRITY = "TYPESCRIPT_INTEGRITY", _TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION", _VUE_COMPONENT_META_VERSION = "VUE_COMPONENT_META_VERSION", _ZIG_VERSION = "ZIG_VERSION")

def _string_list(attr, items, multiline):
    if multiline:
        return attr + " = [\n" + "".join(["        \"" + item + "\",\n" for item in items]) + "    ],"
    return attr + " = [" + ", ".join(["\"" + item + "\"" for item in items]) + "],"

def _redist_pins(cells, multiline = False):
    return [
        "\n".join([
            _string_list("components", _CUDA_COMPONENTS, multiline),
            _string_list("platforms", [_CUDA_CELLS[cell].rules_cuda_platform for cell in cells], multiline),
            "sha256 = \"" + _CUDA_REDIST_SHA256 + "\",",
            "version = \"" + _CUDA_REDIST_VERSION + "\",",
        ]),
    ]

def versions_contract_tests(name):
    """Fails when a tracked copy drifts from its canonical pin."""
    go_archive_pins = []
    for platform in _GO_SDK_SDKS:
        for pin in _GO_SDK_SDKS[platform]:
            go_archive_pins.append(pin)
    cuda_pins = _redist_pins(_cuda_qualified_cells())
    root_cuda_pins = _redist_pins(_cuda_qualified_cells(), multiline = True)
    redist_prefix = "https://developer.download.nvidia.com/compute/cuda/redist/"
    cuda_manifest_blocks = [
        "            \"relative_path\": \"" + pin.url.removeprefix(redist_prefix) + "\",\n            \"sha256\": \"" + pin.sha256 + "\","
        for cell in _CUDA_CELLS.values()
        for pin in cell.components.values()
    ]
    cuda_pin_urls_match_platform = [
        ("/" + cell.rules_cuda_platform + "/") in pin.url
        for cell in _CUDA_CELLS.values()
        for pin in cell.components.values()
    ]
    gem_pins = ["        \"" + gem + "\": \"" + _GEM_CHECKSUMS[gem] + "\"," for gem in _GEM_CHECKSUMS]
    portable_ruby_pins = ["        \"" + name + "\": \"" + _PORTABLE_RUBY_CHECKSUMS[name] + "\"," for name in _PORTABLE_RUBY_CHECKSUMS]
    mirror_pins = "\n".join(
        go_archive_pins + [
            "rustfmt_version = \"" + _RUSTFMT_VERSION + "\"",
            "edition = \"" + _RUST_EDITION + "\"",
            "cargo_lockfile = \"" + _CRATE_CARGO_LOCK + "\"",
            "lockfile = \"" + _CRATE_BAZEL_LOCK + "\"",
        ],
    )
    manifests_block = "    manifests = [\n" + "".join(["        \"" + m + "\",\n" for m in _RUST_CRATE_MANIFESTS]) + "    ],\n"
    npm_pins = "\n".join(
        ["use_repo(npm, " + ", ".join(["\"" + hub + "\"" for hub in _NPM_HUBS]) + ")", "use_repo(npm_dev, " + ", ".join(["\"" + hub + "\"" for hub in _NPM_DEV_HUBS]) + ")"] +
        ["pnpm_lock = \"" + lock + "\"," for lock in _NPM_LOCKS],
    )
    cuda_stanza = "\n".join([
        "name = \"rules_cuda\", version = \"" + _RULES_CUDA_VERSION + "\"",
        "nvcc_version = \"" + _CUDA_VERSION + "\"",
    ] + cuda_pins)
    starlark_test(
        name = name,
        mode = "execution",
        checks = [
            expect_equal("ruby module follows canonical", _MOD_RUBY_VERSION, _RUBY_VERSION),
            expect_equal("python module follows canonical", _MOD_PYTHON_VERSION, _PYTHON_VERSION),
            expect_equal("dart archive url follows canonical", _DART_URL, "https://github.com/aran/rules_dart/releases/download/v" + _RULES_DART_VERSION + "/rules_dart-v" + _RULES_DART_VERSION + ".tar.gz"),
            expect_equal("dart archive strip prefix follows canonical", _DART_STRIP_PREFIX, "rules_dart-" + _RULES_DART_VERSION),
            expect_equal("dotnet archive url follows canonical", _DOTNET_URL, "https://github.com/bazel-contrib/rules_dotnet/releases/download/v" + _RULES_DOTNET_VERSION + "/rules_dotnet-v" + _RULES_DOTNET_VERSION + ".tar.gz"),
            expect_equal("dotnet archive strip prefix follows canonical", _DOTNET_STRIP_PREFIX, "rules_dotnet-" + _RULES_DOTNET_VERSION),
            expect_equal("rust module follows canonical", _MOD_RUST_VERSION, _RUST_VERSION),
            expect_equal("cuda module follows canonical", _MOD_CUDA_VERSION, _CUDA_VERSION),
            expect_equal("cuda rules module follows canonical", _MOD_RULES_CUDA_VERSION, _RULES_CUDA_VERSION),
            expect_equal("cuda max host clang major is a number", type(_CUDA_MAX_HOST_CLANG_MAJOR), "int"),
            expect_equal("cuda redist version extends the toolkit version", _CUDA_REDIST_VERSION.startswith(_CUDA_VERSION + "."), True),
            expect_equal("cuda pin urls sit under their cell platform", cuda_pin_urls_match_platform, [True] * len(cuda_pin_urls_match_platform)),
            expect_equal("cuda redist sha is sha256", len(_CUDA_REDIST_SHA256), 64),
        ],
        file_blocks = {
            "@cuda_redistrib_json//file": cuda_manifest_blocks,
            "//:MODULE.bazel": [manifests_block, "    version = \"" + _RUBY_VERSION + "\",\n"] + gem_pins + portable_ruby_pins + [
                "    sha256 = \"" + _CUDA_REDIST_SHA256 + "\",\n    urls = [\"" + _CUDA_REDIST_URL + "\"],",
            ],
            "//tools/bazelrc:src/lib.rs": [
                "\"build --java_runtime_version=" + _JAVA_RUNTIME_VERSION + "\",",
                "\"build --tool_java_runtime_version=" + _JAVA_RUNTIME_VERSION + "\",",
            ],
        },
        file_checks = {
            "//:.bazelrc": "common --check_direct_dependencies=error",
            "@rules_dx_cuda//:README.md": cuda_stanza,
            "//tools/testing/hermetic:consumers/cuda/module_fragment.bazel": cuda_stanza,
            "//:package.json": "\"packageManager\": \"pnpm@" + _PNPM_VERSION + "\"",
            "//javascript/private/quality:package.json": "\"typedoc\": \"" + _TYPEDOC_VERSION + "\",\n    \"typescript\": \"" + _TYPESCRIPT_VERSION + "\",",
            "//javascript/private/quality:pnpm-lock.yaml": "  vue-component-meta@" + _VUE_COMPONENT_META_VERSION + ":\n",
            "//rust:README.md": "rustfmt_version = \"" + _RUSTFMT_VERSION + "\",\n    versions = [\"" + _RUST_VERSION + "\"],",
            "//:rustfmt.toml": "edition = \"" + _RUST_EDITION + "\"",
            "//third_party/dotnet:paket.lock": "Google.Protobuf (3." + _PROTOBUF_VERSION + ")",
            "//third_party/go:go.mod": "go " + _GO_LANGUAGE_FLOOR,
            "//:MODULE.bazel": "name = \"rules_rust\", version = \"" + _RULES_RUST_VERSION + "\"\n" +
                               "name = \"rules_rust_wasm_bindgen\", version = \"" + _RULES_RUST_WASM_BINDGEN_VERSION + "\"\n" +
                               "name = \"protobuf\", version = \"" + _PROTOBUF_VERSION + "\"\n" +
                               "name = \"rules_android\", version = \"" + _RULES_ANDROID_VERSION + "\"\n" +
                               "name = \"rules_cc\", version = \"" + _RULES_CC_VERSION + "\"\n" +
                               "name = \"llvm\", version = \"" + _LLVM_VERSION + "\"\n" +
                               "name = \"rules_cuda\", version = \"" + _RULES_CUDA_VERSION + "\"\n" +
                               "nvcc_version = \"" + _CUDA_VERSION + "\"\n" +
                               "\n".join(root_cuda_pins) + "\n" +
                               "name = \"googletest\", version = \"" + _GOOGLETEST_VERSION + "\"\n" +
                               "name = \"rules_python\", version = \"" + _RULES_PYTHON_VERSION + "\"\n" +
                               "name = \"rules_go\", version = \"" + _RULES_GO_VERSION + "\"\n" +
                               "name = \"gazelle\", version = \"" + _GAZELLE_VERSION + "\"\n" +
                               "name = \"rules_shell\", version = \"" + _RULES_SHELL_VERSION + "\"\n" +
                               "name = \"apple_support\", version = \"" + _APPLE_SUPPORT_VERSION + "\"\n" +
                               "name = \"rules_apple\", version = \"" + _RULES_APPLE_VERSION + "\"\n" +
                               "name = \"platforms\", version = \"" + _PLATFORMS_VERSION + "\"\n" +
                               "name = \"bazel_skylib\", version = \"" + _BAZEL_SKYLIB_VERSION + "\"\n" +
                               "name = \"rules_proto\", version = \"" + _RULES_PROTO_VERSION + "\"\n" +
                               "name = \"rules_java\", version = \"" + _RULES_JAVA_VERSION + "\"\n" +
                               "name = \"rules_kotlin\", version = \"" + _RULES_KOTLIN_VERSION + "\"\n" +
                               "name = \"rules_scala\", version = \"" + _RULES_SCALA_VERSION + "\"\n" +
                               "name = \"bazel_lib\", version = \"" + _BAZEL_LIB_VERSION + "\"\n" +
                               "name = \"rules_jvm_external\", version = \"" + _RULES_JVM_EXTERNAL_VERSION + "\"\n" +
                               "name = \"rules_zig\", version = \"" + _RULES_ZIG_VERSION + "\"\n" +
                               "name = \"rules_swift\", version = \"" + _RULES_SWIFT_VERSION + "\"\n" +
                               "swift_version = \"" + _SWIFT_VERSION + "\"\n" +
                               "name = \"rules_dart\", version = \"" + _RULES_DART_VERSION + "\"\n" +
                               "dart.toolchain(dart_version = \"" + _DART_SDK_VERSION + "\")\n" +
                               "name = \"rules_julia\", version = \"" + _RULES_JULIA_VERSION + "\"\n" +
                               "julia.toolchain(version = \"" + _JULIA_VERSION + "\")\n" +
                               "zig.toolchain(zig_version = \"" + _ZIG_VERSION + "\")\n" +
                               "name = \"rules_perl\", version = \"" + _RULES_PERL_VERSION + "\"\n" +
                               "name = \"lua\", version = \"" + _LUA_VERSION + "\"\n" +
                               "name = \"rules_py\", version = \"" + _RULES_PY_VERSION + "\"\n" +
                               "name = \"aspect_rules_js\", version = \"" + _ASPECT_RULES_JS_VERSION + "\"\n" +
                               "name = \"aspect_rules_ts\", version = \"" + _ASPECT_RULES_TS_VERSION + "\"\n" +
                               "name = \"aspect_rules_jest\", version = \"" + _ASPECT_RULES_JEST_VERSION + "\"\n" +
                               "name = \"rules_nodejs\", version = \"" + _RULES_NODEJS_VERSION + "\"\n" +
                               "dotnet_version = \"" + _DOTNET_VERSION + "\"\n" +
                               "version = \"" + _GO_SDK_VERSION + "\"\n" +
                               "go" + _GO_SDK_VERSION + ".linux-amd64.tar.gz\n" +
                               "version = \"" + _TYPESCRIPT_VERSION + "\"\n" +
                               _TYPESCRIPT_INTEGRITY + "\n" +
                               "python_version = \"" + _PYTHON_VERSION + "\"\n" +
                               "versions = [\"" + _RUST_VERSION + "\"]\n" +
                               "scala_version = \"" + _SCALA_VERSION + "\"\n" +
                               "ch.epfl.scala:scalafix-cli_2.13." + _SCALA_VERSION.split(".")[2] + ":" + _JVM_TOOL_VERSIONS["scalafix"] + "\n" +
                               npm_pins + "\n" +
                               mirror_pins,
        },
    )
