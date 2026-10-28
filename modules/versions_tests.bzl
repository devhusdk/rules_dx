"""Canonical version pins stay equal to MODULE.bazel literals."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//tools/testing:bazel_pins.bzl", "NESTED_BAZEL_SHA256", "NESTED_BAZEL_VERSION")
load(":cuda.bzl", _CUDA_COMPONENTS = "CUDA_COMPONENTS", _CUDA_HOST_SYSROOT = "CUDA_HOST_SYSROOT", _CUDA_MAX_HOST_CLANG_MAJOR = "CUDA_MAX_HOST_CLANG_MAJOR", _MOD_CUDA_HOST_LLVM_VERSION = "CUDA_HOST_LLVM_VERSION", _MOD_CUDA_VERSION = "CUDA_VERSION", _MOD_RULES_CUDA_VERSION = "RULES_CUDA_VERSION", _MOD_TOOLCHAINS_LLVM_VERSION = "TOOLCHAINS_LLVM_VERSION")
load(":dotnet.bzl", _MOD_DOTNET_VERSION = "DOTNET_VERSION", _MOD_RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION")
load(":java-scala-kotlin.bzl", _JAVA_RUNTIME_VERSION = "JAVA_RUNTIME_VERSION", _JVM_TOOL_VERSIONS = "JVM_TOOL_VERSIONS", _MOD_SCALA_VERSION = "SCALA_VERSION")
load(":js.bzl", _MOD_TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION", _NPM_DEV_HUBS = "NPM_DEV_HUBS", _NPM_HUBS = "NPM_HUBS", _NPM_LOCKS = "NPM_LOCKS")
load(":powershell.bzl", _MOD_PWSH_VERSION = "PWSH_VERSION")
load(":python.bzl", _MOD_PYTHON_VERSION = "PYTHON_VERSION")
load(":ruby.bzl", _GEM_CHECKSUMS = "GEM_CHECKSUMS", _MOD_RUBY_VERSION = "RUBY_VERSION", _MOD_RULES_RUBY_VERSION = "RULES_RUBY_VERSION", _PORTABLE_RUBY_CHECKSUMS = "PORTABLE_RUBY_CHECKSUMS")
load(":rust.bzl", _CRATE_BAZEL_LOCK = "CRATE_BAZEL_LOCK", _CRATE_CARGO_LOCK = "CRATE_CARGO_LOCK", _MOD_RUST_VERSION = "RUST_VERSION", _RUST_CRATE_MANIFESTS = "RUST_CRATE_MANIFESTS")
load(":toolchains.bzl", _MOD_GO_LANGUAGE_FLOOR = "GO_LANGUAGE_FLOOR", _MOD_GO_SDK_VERSION = "GO_SDK_VERSION")
load(":versions.bzl", _APPLE_SUPPORT_VERSION = "FLOOR_APPLE_SUPPORT", _ASPECT_RULES_JEST_VERSION = "FLOOR_ASPECT_RULES_JEST", _ASPECT_RULES_JS_VERSION = "FLOOR_ASPECT_RULES_JS", _ASPECT_RULES_PY_VERSION = "FLOOR_ASPECT_RULES_PY", _ASPECT_RULES_TS_VERSION = "FLOOR_ASPECT_RULES_TS", _BAZELISK_VERSION = "BAZELISK_VERSION", _BAZEL_LIB_VERSION = "FLOOR_BAZEL_LIB", _BAZEL_SKYLIB_VERSION = "FLOOR_BAZEL_SKYLIB", _BAZEL_VERSION = "BAZEL_VERSION", _CUDA_HOST_LLVM_VERSION = "CUDA_HOST_LLVM_VERSION", _CUDA_VERSION = "CUDA_VERSION", _DOTNET_VERSION = "DOTNET_VERSION", _GAZELLE_VERSION = "FLOOR_GAZELLE", _GOOGLETEST_VERSION = "GOOGLETEST_VERSION", _GO_LANGUAGE_FLOOR = "GO_LANGUAGE_FLOOR", _GO_SDK_SDKS = "GO_SDK_SDKS", _GO_SDK_VERSION = "GO_SDK_VERSION", _JULIA_VERSION = "JULIA_VERSION", _LLVM_VERSION = "FLOOR_LLVM", _LUA_VERSION = "FLOOR_LUA", _PLATFORMS_VERSION = "FLOOR_PLATFORMS", _PNPM_VERSION = "PNPM_VERSION", _PROTOBUF_VERSION = "FLOOR_PROTOBUF", _PWSH_VERSION = "PWSH_VERSION", _PYTHON_VERSION = "PYTHON_VERSION", _RUBY_VERSION = "RUBY_VERSION", _RULES_ANDROID_VERSION = "FLOOR_RULES_ANDROID", _RULES_APPLE_VERSION = "FLOOR_RULES_APPLE", _RULES_CC_VERSION = "FLOOR_RULES_CC", _RULES_CUDA_VERSION = "FLOOR_RULES_CUDA", _RULES_DOTNET_VERSION = "FLOOR_RULES_DOTNET", _RULES_GO_VERSION = "FLOOR_RULES_GO", _RULES_JAVA_VERSION = "FLOOR_RULES_JAVA", _RULES_JULIA_VERSION = "FLOOR_RULES_JULIA", _RULES_JVM_EXTERNAL_VERSION = "FLOOR_RULES_JVM_EXTERNAL", _RULES_KOTLIN_VERSION = "FLOOR_RULES_KOTLIN", _RULES_NODEJS_VERSION = "FLOOR_RULES_NODEJS", _RULES_PERL_VERSION = "FLOOR_RULES_PERL", _RULES_POWERSHELL_VERSION = "FLOOR_RULES_POWERSHELL", _RULES_PROTO_VERSION = "FLOOR_RULES_PROTO", _RULES_PYTHON_VERSION = "FLOOR_RULES_PYTHON", _RULES_RUBY_VERSION = "FLOOR_RULES_RUBY", _RULES_RUST_PROST_VERSION = "FLOOR_RULES_RUST_PROST", _RULES_RUST_VERSION = "FLOOR_RULES_RUST", _RULES_RUST_WASM_BINDGEN_VERSION = "FLOOR_RULES_RUST_WASM_BINDGEN", _RULES_SCALA_VERSION = "FLOOR_RULES_SCALA", _RULES_SHELL_VERSION = "FLOOR_RULES_SHELL", _RULES_ZIG_VERSION = "FLOOR_RULES_ZIG", _RUSTFMT_VERSION = "RUSTFMT_VERSION", _RUST_EDITION = "RUST_EDITION", _RUST_VERSION = "RUST_VERSION", _SCALA_VERSION = "SCALA_VERSION", _TOOLCHAINS_LLVM_VERSION = "TOOLCHAINS_LLVM_VERSION", _TYPEDOC_VERSION = "TYPEDOC_VERSION", _TYPESCRIPT_INTEGRITY = "TYPESCRIPT_INTEGRITY", _TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION", _ZIG_VERSION = "ZIG_VERSION")

def versions_contract_tests(name):
    """Fails when a tracked copy drifts from its canonical pin."""
    go_archive_pins = []
    for platform in _GO_SDK_SDKS:
        for pin in _GO_SDK_SDKS[platform]:
            go_archive_pins.append(pin)
    cuda_pins = [
        "\n".join([
            "sha256 = \"" + pin.sha256 + "\"",
            "strip_prefix = \"" + pin.strip_prefix + "\"",
            "url = \"" + pin.url + "\"",
            "version = \"" + pin.version + "\"",
        ])
        for pin in _CUDA_COMPONENTS.values()
    ]
    gem_pins = [_GEM_CHECKSUMS[gem] for gem in _GEM_CHECKSUMS]
    portable_ruby_pins = [_PORTABLE_RUBY_CHECKSUMS[name] for name in _PORTABLE_RUBY_CHECKSUMS]
    mirror_pins = "\n".join(
        go_archive_pins +
        gem_pins +
        portable_ruby_pins + [
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
        "name = \"toolchains_llvm\", version = \"" + _TOOLCHAINS_LLVM_VERSION + "\"",
        "llvm_version = \"" + _CUDA_HOST_LLVM_VERSION + "\"",
        "nvcc_version = \"" + _CUDA_VERSION + "\"",
        "sha256 = \"" + _CUDA_HOST_SYSROOT.sha256 + "\"",
        _CUDA_HOST_SYSROOT.url,
    ] + cuda_pins)
    starlark_test(
        name = name,
        mode = "execution",
        checks = [
            expect_equal("dotnet module follows canonical", _MOD_DOTNET_VERSION, _DOTNET_VERSION),
            expect_equal("dotnet rules module follows canonical", _MOD_RULES_DOTNET_VERSION, _RULES_DOTNET_VERSION),
            expect_equal("ruby module follows canonical", _MOD_RUBY_VERSION, _RUBY_VERSION),
            expect_equal("ruby rules module follows canonical", _MOD_RULES_RUBY_VERSION, _RULES_RUBY_VERSION),
            expect_equal("python module follows canonical", _MOD_PYTHON_VERSION, _PYTHON_VERSION),
            expect_equal("typescript module follows canonical", _MOD_TYPESCRIPT_VERSION, _TYPESCRIPT_VERSION),
            expect_equal("pwsh module follows canonical", _MOD_PWSH_VERSION, _PWSH_VERSION),
            expect_equal("rust module follows canonical", _MOD_RUST_VERSION, _RUST_VERSION),
            expect_equal("go sdk module follows canonical", _MOD_GO_SDK_VERSION, _GO_SDK_VERSION),
            expect_equal("go floor module follows canonical", _MOD_GO_LANGUAGE_FLOOR, _GO_LANGUAGE_FLOOR),
            expect_equal("scala module follows canonical", _MOD_SCALA_VERSION, _SCALA_VERSION),
            expect_equal("cuda module follows canonical", _MOD_CUDA_VERSION, _CUDA_VERSION),
            expect_equal("cuda rules module follows canonical", _MOD_RULES_CUDA_VERSION, _RULES_CUDA_VERSION),
            expect_equal("cuda host llvm module follows canonical", _MOD_CUDA_HOST_LLVM_VERSION, _CUDA_HOST_LLVM_VERSION),
            expect_equal("cuda host llvm major is accepted by the pinned nvcc", int(_CUDA_HOST_LLVM_VERSION.split(".")[0]) <= _CUDA_MAX_HOST_CLANG_MAJOR, True),
            expect_equal("toolchains_llvm module follows canonical", _MOD_TOOLCHAINS_LLVM_VERSION, _TOOLCHAINS_LLVM_VERSION),
            expect_equal("nested bazel pin follows canonical", NESTED_BAZEL_VERSION, _BAZEL_VERSION),
            expect_equal("nested bazel pins every platform", sorted(NESTED_BAZEL_SHA256.keys()), ["darwin-arm64", "darwin-x86_64", "linux-arm64", "linux-x86_64", "windows-x86_64"]),
        ],
        file_blocks = {
            "//:MODULE.bazel": [manifests_block],
        },
        file_checks = {
            "//.devcontainer:Dockerfile.prebuilt": "bazelisk/releases/download/v" + _BAZELISK_VERSION + "/",
            "//:.bazelrc": "build --java_runtime_version=" + _JAVA_RUNTIME_VERSION + "\n" +
                           "build --tool_java_runtime_version=" + _JAVA_RUNTIME_VERSION + "\n" +
                           "common --check_direct_dependencies=error",
            "//:.bazelversion": _BAZEL_VERSION,
            "//:cuda/README.md": cuda_stanza,
            "//tools/testing/hermetic:consumers/cuda/module_fragment.bazel": cuda_stanza,
            "//:package.json": "\"packageManager\": \"pnpm@" + _PNPM_VERSION + "\"",
            "//quality/tools/javascript:package.json": "\"typedoc\": \"" + _TYPEDOC_VERSION + "\",\n    \"typescript\": \"" + _TYPESCRIPT_VERSION + "\",",
            "//:rustfmt.toml": "edition = \"" + _RUST_EDITION + "\"",
            "//third_party/go:go.mod": "go " + _GO_LANGUAGE_FLOOR,
            "//:MODULE.bazel": "name = \"rules_rust\", version = \"" + _RULES_RUST_VERSION + "\"\n" +
                               "name = \"rules_rust_wasm_bindgen\", version = \"" + _RULES_RUST_WASM_BINDGEN_VERSION + "\"\n" +
                               "name = \"protobuf\", version = \"" + _PROTOBUF_VERSION + "\"\n" +
                               "name = \"rules_android\", version = \"" + _RULES_ANDROID_VERSION + "\"\n" +
                               "name = \"rules_cc\", version = \"" + _RULES_CC_VERSION + "\"\n" +
                               "name = \"llvm\", version = \"" + _LLVM_VERSION + "\"\n" +
                               "name = \"rules_cuda\", version = \"" + _RULES_CUDA_VERSION + "\"\n" +
                               "name = \"toolchains_llvm\", version = \"" + _TOOLCHAINS_LLVM_VERSION + "\"\n" +
                               "llvm_version = \"" + _CUDA_HOST_LLVM_VERSION + "\"\n" +
                               "cuda_host_llvm = use_extension(\"@toolchains_llvm//toolchain/extensions:llvm.bzl\", \"llvm\", dev_dependency = True)\n" +
                               "nvcc_version = \"" + _CUDA_VERSION + "\"\n" +
                               "sha256 = \"" + _CUDA_HOST_SYSROOT.sha256 + "\"\n" +
                               _CUDA_HOST_SYSROOT.url + "\n" +
                               "\n".join(cuda_pins) + "\n" +
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
                               "name = \"rules_rust_prost\", version = \"" + _RULES_RUST_PROST_VERSION + "\"\n" +
                               "name = \"rules_java\", version = \"" + _RULES_JAVA_VERSION + "\"\n" +
                               "name = \"rules_kotlin\", version = \"" + _RULES_KOTLIN_VERSION + "\"\n" +
                               "name = \"rules_scala\", version = \"" + _RULES_SCALA_VERSION + "\"\n" +
                               "name = \"rules_dotnet\", version = \"" + _RULES_DOTNET_VERSION + "\"\n" +
                               "name = \"bazel_lib\", version = \"" + _BAZEL_LIB_VERSION + "\"\n" +
                               "name = \"rules_jvm_external\", version = \"" + _RULES_JVM_EXTERNAL_VERSION + "\"\n" +
                               "name = \"rules_powershell\", version = \"" + _RULES_POWERSHELL_VERSION + "\"\n" +
                               "name = \"rules_ruby\", version = \"" + _RULES_RUBY_VERSION + "\"\n" +
                               "name = \"rules_zig\", version = \"" + _RULES_ZIG_VERSION + "\"\n" +
                               "zig.toolchain(zig_version = \"" + _ZIG_VERSION + "\")\n" +
                               "name = \"rules_julia\", version = \"" + _RULES_JULIA_VERSION + "\"\n" +
                               "julia.toolchain(version = \"" + _JULIA_VERSION + "\")\n" +
                               "name = \"rules_perl\", version = \"" + _RULES_PERL_VERSION + "\"\n" +
                               "name = \"lua\", version = \"" + _LUA_VERSION + "\"\n" +
                               "name = \"aspect_rules_py\", version = \"" + _ASPECT_RULES_PY_VERSION + "\"\n" +
                               "name = \"aspect_rules_js\", version = \"" + _ASPECT_RULES_JS_VERSION + "\"\n" +
                               "name = \"aspect_rules_ts\", version = \"" + _ASPECT_RULES_TS_VERSION + "\"\n" +
                               "name = \"aspect_rules_jest\", version = \"" + _ASPECT_RULES_JEST_VERSION + "\"\n" +
                               "name = \"rules_nodejs\", version = \"" + _RULES_NODEJS_VERSION + "\"\n" +
                               "dotnet_version = \"" + _DOTNET_VERSION + "\"\n" +
                               "version = \"" + _GO_SDK_VERSION + "\"\n" +
                               "go" + _GO_SDK_VERSION + ".linux-amd64.tar.gz\n" +
                               "version = \"" + _PWSH_VERSION + "\"\n" +
                               "version = \"" + _RUBY_VERSION + "\"\n" +
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
