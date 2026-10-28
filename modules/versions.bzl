"""Single source of truth for toolchain and ruleset versions.

Every consumer-facing (non-dev) `bazel_dep` of the root MODULE.bazel has two numbers:
FLOOR_<DEP> is the oldest version consumers may resolve, written in its `bazel_dep`;
TESTED_<DEP> is the tested-latest version, written in its root-only
`single_version_override`. Dev-only deps keep one <DEP>_VERSION pin.
"""

BAZEL_VERSION = "9.2.0"
BAZELISK_VERSION = "1.29.0"

FLOOR_RULES_RUST = "0.74.0"
TESTED_RULES_RUST = "0.74.0"
FLOOR_RULES_RUST_PROST = "0.74.0"
TESTED_RULES_RUST_PROST = "0.74.0"
FLOOR_RULES_RUST_WASM_BINDGEN = "0.74.0"
TESTED_RULES_RUST_WASM_BINDGEN = "0.74.0"
RUST_VERSION = "1.98.0"
RUSTFMT_VERSION = "1.98.0"
RUST_EDITION = "2021"

FLOOR_RULES_CC = "0.2.27"
TESTED_RULES_CC = "0.2.27"
GOOGLETEST_VERSION = "1.18.0"
FLOOR_LLVM = "0.8.23"
TESTED_LLVM = "0.8.23"
FLOOR_RULES_CUDA = "0.3.0"
TESTED_RULES_CUDA = "0.3.0"
CUDA_VERSION = "13.4"
TOOLCHAINS_LLVM_VERSION = "1.12.0"
CUDA_HOST_LLVM_VERSION = "22.1.8"

FLOOR_RULES_GO = "0.64.2"
TESTED_RULES_GO = "0.64.2"
FLOOR_GAZELLE = "0.54.0"
TESTED_GAZELLE = "0.54.0"
FLOOR_RULES_SHELL = "0.8.0"
TESTED_RULES_SHELL = "0.8.0"
FLOOR_APPLE_SUPPORT = "2.9.1"
TESTED_APPLE_SUPPORT = "2.9.1"
FLOOR_RULES_APPLE = "5.2.0"
TESTED_RULES_APPLE = "5.2.0"
FLOOR_PLATFORMS = "1.1.0"
TESTED_PLATFORMS = "1.1.0"
FLOOR_BAZEL_SKYLIB = "1.9.2"
TESTED_BAZEL_SKYLIB = "1.9.2"
FLOOR_RULES_PROTO = "7.1.0"
TESTED_RULES_PROTO = "7.1.0"
FLOOR_PROTOBUF = "34.0.bcr.1"
TESTED_PROTOBUF = "34.0.bcr.1"
GO_SDK_VERSION = "1.26.10"
GO_LANGUAGE_FLOOR = "1.25.0"

GO_SDK_SDKS = {
    "darwin_amd64": ["go1.26.10.darwin-amd64.tar.gz", "e820ab780c273ae4a340b79d0f3c4313b74ee029aa3874792145d44b9f4f91da"],
    "darwin_arm64": ["go1.26.10.darwin-arm64.tar.gz", "0b5183492292ada170f9b561a834863808dd02edba599e4ad0e89899330020b3"],
    "linux_amd64": ["go1.26.10.linux-amd64.tar.gz", "ccfe7ae822d2c2f41f4fe7c956199967a4dac22d0ee0d2b2bfdb440aab3b20c5"],
    "linux_arm64": ["go1.26.10.linux-arm64.tar.gz", "3f4227ce7b206a513e794f1393adb473a07b46e94d18bd15d1c684bbb7319797"],
    "windows_amd64": ["go1.26.10.windows-amd64.zip", "7ecd2d7260e82c59bcbd7f1077a07cee2dea607295b858df67195a1e4e09629b"],
    "windows_arm64": ["go1.26.10.windows-arm64.zip", "6831fd8c31091243d5695c8fa3a721c9f4ae2bb36482763e77e57ee01b76489f"],
}

FLOOR_RULES_PYTHON = "1.9.0"
TESTED_RULES_PYTHON = "1.9.0"
FLOOR_ASPECT_RULES_PY = "2.0.0-alpha.6"
TESTED_ASPECT_RULES_PY = "2.0.0-alpha.6"
PYTHON_VERSION = "3.12"

FLOOR_ASPECT_RULES_JS = "3.5.2"
TESTED_ASPECT_RULES_JS = "3.5.2"
FLOOR_ASPECT_RULES_TS = "3.10.0"
TESTED_ASPECT_RULES_TS = "3.10.0"
FLOOR_ASPECT_RULES_JEST = "0.26.0"
TESTED_ASPECT_RULES_JEST = "0.26.0"
FLOOR_RULES_NODEJS = "6.7.7"
TESTED_RULES_NODEJS = "6.7.7"
TYPESCRIPT_VERSION = "5.9.3"
TYPEDOC_VERSION = "0.28.20"
TYPESCRIPT_INTEGRITY = "sha512-jl1vZzPDinLr9eUt3J/t7V6FgNEw9QjvBPdysz9KfQDD41fQrC2Y4vKQdiaUpFT4bXlb1RHhLpp8wtm6M5TgSw=="
PNPM_VERSION = "10.34.5"
NODE_VERSION_PREFIX = "v22."

FLOOR_RULES_JAVA = "9.7.0"
TESTED_RULES_JAVA = "9.7.0"
FLOOR_RULES_ANDROID = "0.8.0"
TESTED_RULES_ANDROID = "0.8.0"
FLOOR_RULES_KOTLIN = "2.4.10"
TESTED_RULES_KOTLIN = "2.4.10"
FLOOR_RULES_SCALA = "7.3.0"
TESTED_RULES_SCALA = "7.3.0"
SCALA_VERSION = "2.13.18"
FLOOR_RULES_JVM_EXTERNAL = "7.1"
TESTED_RULES_JVM_EXTERNAL = "7.1"

FLOOR_RULES_DOTNET = "0.22.1"
TESTED_RULES_DOTNET = "0.22.1"
FLOOR_BAZEL_LIB = "3.7.2"
TESTED_BAZEL_LIB = "3.7.2"
DOTNET_VERSION = "10.0.201"

FLOOR_RULES_POWERSHELL = "0.2.0"
TESTED_RULES_POWERSHELL = "0.2.0"
PWSH_VERSION = "7.5.4"

FLOOR_RULES_RUBY = "0.28.0"
TESTED_RULES_RUBY = "0.28.0"
RUBY_VERSION = "3.4.9"

FLOOR_RULES_ZIG = "0.14.0"
TESTED_RULES_ZIG = "0.14.0"
ZIG_VERSION = "0.16.0"

RULES_JULIA_VERSION = "0.8.0"
JULIA_VERSION = "1.12.5"
FLOOR_RULES_PERL = "1.5.0"
TESTED_RULES_PERL = "1.5.0"
FLOOR_LUA = "5.4.6.bcr.1"
TESTED_LUA = "5.4.6.bcr.1"

# Every consumer-facing bazel_dep of the root MODULE.bazel, keyed by module name.
# //modules:consumer_deps_test checks it against MODULE.bazel in both directions.
CONSUMER_BAZEL_DEPS = {
    "rules_rust": struct(floor = FLOOR_RULES_RUST, tested = TESTED_RULES_RUST),
    "rules_rust_wasm_bindgen": struct(floor = FLOOR_RULES_RUST_WASM_BINDGEN, tested = TESTED_RULES_RUST_WASM_BINDGEN),
    "rules_cc": struct(floor = FLOOR_RULES_CC, tested = TESTED_RULES_CC),
    "llvm": struct(floor = FLOOR_LLVM, tested = TESTED_LLVM),
    "rules_cuda": struct(floor = FLOOR_RULES_CUDA, tested = TESTED_RULES_CUDA),
    "rules_python": struct(floor = FLOOR_RULES_PYTHON, tested = TESTED_RULES_PYTHON),
    "rules_go": struct(floor = FLOOR_RULES_GO, tested = TESTED_RULES_GO),
    "gazelle": struct(floor = FLOOR_GAZELLE, tested = TESTED_GAZELLE),
    "rules_shell": struct(floor = FLOOR_RULES_SHELL, tested = TESTED_RULES_SHELL),
    "apple_support": struct(floor = FLOOR_APPLE_SUPPORT, tested = TESTED_APPLE_SUPPORT),
    "rules_apple": struct(floor = FLOOR_RULES_APPLE, tested = TESTED_RULES_APPLE),
    "platforms": struct(floor = FLOOR_PLATFORMS, tested = TESTED_PLATFORMS),
    "bazel_skylib": struct(floor = FLOOR_BAZEL_SKYLIB, tested = TESTED_BAZEL_SKYLIB),
    "rules_proto": struct(floor = FLOOR_RULES_PROTO, tested = TESTED_RULES_PROTO),
    "protobuf": struct(floor = FLOOR_PROTOBUF, tested = TESTED_PROTOBUF),
    "rules_rust_prost": struct(floor = FLOOR_RULES_RUST_PROST, tested = TESTED_RULES_RUST_PROST),
    "rules_java": struct(floor = FLOOR_RULES_JAVA, tested = TESTED_RULES_JAVA),
    "rules_android": struct(floor = FLOOR_RULES_ANDROID, tested = TESTED_RULES_ANDROID),
    "rules_kotlin": struct(floor = FLOOR_RULES_KOTLIN, tested = TESTED_RULES_KOTLIN),
    "rules_scala": struct(floor = FLOOR_RULES_SCALA, tested = TESTED_RULES_SCALA),
    "rules_dotnet": struct(floor = FLOOR_RULES_DOTNET, tested = TESTED_RULES_DOTNET),
    "bazel_lib": struct(floor = FLOOR_BAZEL_LIB, tested = TESTED_BAZEL_LIB),
    "rules_jvm_external": struct(floor = FLOOR_RULES_JVM_EXTERNAL, tested = TESTED_RULES_JVM_EXTERNAL),
    "rules_powershell": struct(floor = FLOOR_RULES_POWERSHELL, tested = TESTED_RULES_POWERSHELL),
    "rules_ruby": struct(floor = FLOOR_RULES_RUBY, tested = TESTED_RULES_RUBY),
    "rules_zig": struct(floor = FLOOR_RULES_ZIG, tested = TESTED_RULES_ZIG),
    "rules_perl": struct(floor = FLOOR_RULES_PERL, tested = TESTED_RULES_PERL),
    "lua": struct(floor = FLOOR_LUA, tested = TESTED_LUA),
    "aspect_rules_py": struct(floor = FLOOR_ASPECT_RULES_PY, tested = TESTED_ASPECT_RULES_PY),
    "aspect_rules_js": struct(floor = FLOOR_ASPECT_RULES_JS, tested = TESTED_ASPECT_RULES_JS),
    "aspect_rules_ts": struct(floor = FLOOR_ASPECT_RULES_TS, tested = TESTED_ASPECT_RULES_TS),
    "aspect_rules_jest": struct(floor = FLOOR_ASPECT_RULES_JEST, tested = TESTED_ASPECT_RULES_JEST),
    "rules_nodejs": struct(floor = FLOOR_RULES_NODEJS, tested = TESTED_RULES_NODEJS),
}
