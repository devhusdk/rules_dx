"""Toolchain and ruleset versions."""

RULES_RUST_VERSION = "0.75.0"
RULES_RUST_PROST_VERSION = "0.75.0"
RULES_RUST_WASM_BINDGEN_VERSION = "0.75.0"
RUST_VERSION = "1.99.1"
RUSTFMT_VERSION = "1.99.1"
RUST_EDITION = "2024"

RULES_CC_VERSION = "0.2.28"
GOOGLETEST_VERSION = "1.18.0.bcr.1"
LLVM_VERSION = "0.8.26"
RULES_CUDA_VERSION = "0.4.0-beta1"
CUDA_VERSION = "13.4"
CUDA_REDIST_VERSION = "13.4.2"

TESTED_RULES_GO = "0.65.0"
GAZELLE_VERSION = "0.54.0"
RULES_SHELL_VERSION = "0.8.0"
TESTED_APPLE_SUPPORT = "2.12.0"
TESTED_RULES_APPLE = "5.4.0"
PLATFORMS_VERSION = "1.1.0"
BAZEL_SKYLIB_VERSION = "1.9.2"
RULES_PROTO_VERSION = "7.1.0"
TESTED_PROTOBUF = "36.2"
GO_SDK_VERSION = "1.26.10"
GO_LANGUAGE_FLOOR = "1.26.0"

GO_SDK_SDKS = {
    "darwin_amd64": ["go1.26.10.darwin-amd64.tar.gz", "e820ab780c273ae4a340b79d0f3c4313b74ee029aa3874792145d44b9f4f91da"],
    "darwin_arm64": ["go1.26.10.darwin-arm64.tar.gz", "0b5183492292ada170f9b561a834863808dd02edba599e4ad0e89899330020b3"],
    "linux_amd64": ["go1.26.10.linux-amd64.tar.gz", "ccfe7ae822d2c2f41f4fe7c956199967a4dac22d0ee0d2b2bfdb440aab3b20c5"],
    "linux_arm64": ["go1.26.10.linux-arm64.tar.gz", "3f4227ce7b206a513e794f1393adb473a07b46e94d18bd15d1c684bbb7319797"],
    "windows_amd64": ["go1.26.10.windows-amd64.zip", "7ecd2d7260e82c59bcbd7f1077a07cee2dea607295b858df67195a1e4e09629b"],
    "windows_arm64": ["go1.26.10.windows-arm64.zip", "6831fd8c31091243d5695c8fa3a721c9f4ae2bb36482763e77e57ee01b76489f"],
}

RULES_PYTHON_VERSION = "2.4.6"
RULES_PY_VERSION = "2.0.0-alpha.11"
PYTHON_VERSION = "3.12"

ASPECT_RULES_JS_VERSION = "3.6.0"
ASPECT_RULES_TS_VERSION = "3.10.1"
ASPECT_RULES_JEST_VERSION = "0.26.0"
RULES_NODEJS_VERSION = "6.7.7"
TYPESCRIPT_VERSION = "5.9.3"
TYPEDOC_VERSION = "0.28.20"
VUE_COMPONENT_META_VERSION = "3.3.14"
SVELTE2TSX_VERSION = "0.7.63"
SVELTE_VERSION = "5.57.0"
ASTRO_COMPILER_VERSION = "4.0.0"
MDX_VERSION = "3.1.1"
TYPESCRIPT_INTEGRITY = "sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw=="
PNPM_VERSION = "10.34.5"
NODE_VERSION_PREFIX = "v22."

RULES_JAVA_VERSION = "9.9.0"
RULES_ANDROID_VERSION = "0.8.1"
TESTED_RULES_KOTLIN = "2.4.21"
TESTED_RULES_SCALA = "7.4.1"
SCALA_VERSION = "2.13.18"
TESTED_RULES_JVM_EXTERNAL = "7.2"

RULES_DOTNET_VERSION = "0.22.2"
BAZEL_LIB_VERSION = "3.7.2"
DOTNET_VERSION = "10.0.403"

RUBY_VERSION = "3.4.12"

RULES_ZIG_VERSION = "0.17.0"
ZIG_VERSION = "0.17.0"

RULES_SWIFT_VERSION = "4.2.2"
SWIFT_VERSION = "6.4.1"

RULES_JULIA_VERSION = "0.8.0"
RULES_DART_VERSION = "0.8.1"
DART_SDK_VERSION = "3.13.7"
JULIA_VERSION = "1.12.5"
RULES_PERL_VERSION = "1.5.0"
LUA_VERSION = "5.4.6.bcr.1"

CONSUMER_BAZEL_DEPS = {
    "rules_rust": struct(floor = RULES_RUST_VERSION, tested = RULES_RUST_VERSION),
    "rules_rust_wasm_bindgen": struct(floor = RULES_RUST_WASM_BINDGEN_VERSION, tested = RULES_RUST_WASM_BINDGEN_VERSION),
    "rules_cc": struct(floor = RULES_CC_VERSION, tested = RULES_CC_VERSION),
    "llvm": struct(floor = LLVM_VERSION, tested = LLVM_VERSION),
    "rules_python": struct(floor = RULES_PYTHON_VERSION, tested = RULES_PYTHON_VERSION),
    "rules_go": struct(floor = TESTED_RULES_GO, tested = TESTED_RULES_GO),
    "gazelle": struct(floor = GAZELLE_VERSION, tested = GAZELLE_VERSION),
    "rules_shell": struct(floor = RULES_SHELL_VERSION, tested = RULES_SHELL_VERSION),
    "apple_support": struct(floor = TESTED_APPLE_SUPPORT, tested = TESTED_APPLE_SUPPORT),
    "rules_apple": struct(floor = TESTED_RULES_APPLE, tested = TESTED_RULES_APPLE),
    "platforms": struct(floor = PLATFORMS_VERSION, tested = PLATFORMS_VERSION),
    "bazel_skylib": struct(floor = BAZEL_SKYLIB_VERSION, tested = BAZEL_SKYLIB_VERSION),
    "rules_proto": struct(floor = RULES_PROTO_VERSION, tested = RULES_PROTO_VERSION),
    "protobuf": struct(floor = TESTED_PROTOBUF, tested = TESTED_PROTOBUF),
    "rules_java": struct(floor = RULES_JAVA_VERSION, tested = RULES_JAVA_VERSION),
    "rules_android": struct(floor = RULES_ANDROID_VERSION, tested = RULES_ANDROID_VERSION),
    "rules_kotlin": struct(floor = TESTED_RULES_KOTLIN, tested = TESTED_RULES_KOTLIN),
    "rules_scala": struct(floor = TESTED_RULES_SCALA, tested = TESTED_RULES_SCALA),
    "bazel_lib": struct(floor = BAZEL_LIB_VERSION, tested = BAZEL_LIB_VERSION),
    "rules_jvm_external": struct(floor = TESTED_RULES_JVM_EXTERNAL, tested = TESTED_RULES_JVM_EXTERNAL),
    "rules_zig": struct(floor = RULES_ZIG_VERSION, tested = RULES_ZIG_VERSION),
    "rules_swift": struct(floor = RULES_SWIFT_VERSION, tested = RULES_SWIFT_VERSION),
    "bazel_linux_packages": struct(floor = "0.4.1", tested = "0.4.1"),
    "rules_julia": struct(floor = RULES_JULIA_VERSION, tested = RULES_JULIA_VERSION),
    "rules_dart": struct(floor = RULES_DART_VERSION, tested = RULES_DART_VERSION),
    "rules_perl": struct(floor = RULES_PERL_VERSION, tested = RULES_PERL_VERSION),
    "rules_oci": struct(floor = "2.3.0", tested = "2.3.0"),
    "rules_pkg": struct(floor = "1.3.0", tested = "1.3.0"),
    "lua": struct(floor = LUA_VERSION, tested = LUA_VERSION),
    "rules_py": struct(floor = RULES_PY_VERSION, tested = RULES_PY_VERSION),
    "aspect_rules_js": struct(floor = ASPECT_RULES_JS_VERSION, tested = ASPECT_RULES_JS_VERSION),
    "aspect_rules_ts": struct(floor = ASPECT_RULES_TS_VERSION, tested = ASPECT_RULES_TS_VERSION),
    "aspect_rules_jest": struct(floor = ASPECT_RULES_JEST_VERSION, tested = ASPECT_RULES_JEST_VERSION),
    "avro-cpp": struct(floor = "1.12.0.bcr.1", tested = "1.12.0.bcr.1"),
    "hermetic_launcher": struct(floor = "0.0.17", tested = "0.0.17"),
    "gcc_toolchain": struct(floor = "0.13.0", tested = "0.13.0"),
    "windows_support": struct(floor = "0.4.1", tested = "0.4.1"),
    "google_benchmark": struct(floor = "1.10.0", tested = "1.10.0"),
    "elfutils": struct(floor = "0.195", tested = "0.195"),
    "zlib": struct(floor = "1.3.2.bcr.3", tested = "1.3.2.bcr.3"),
    "rules_rust_prost": struct(floor = "0.75.0", tested = "0.75.0"),
    "rules_cuda": struct(floor = RULES_CUDA_VERSION, tested = RULES_CUDA_VERSION),
}

NPM_HUBS = ["dx_npm_tools"]
NPM_DEV_HUBS = ["dx_npm"]
NPM_LOCKS = [
    "//:pnpm-lock.yaml",
    "//javascript/private/quality:pnpm-lock.yaml",
]

JVM_TOOL_VERSIONS = {
    "scalafix": "0.14.9",
}

JAVA_MAJOR = "21"
JAVA_RUNTIME_VERSION = "remotejdk_" + JAVA_MAJOR
