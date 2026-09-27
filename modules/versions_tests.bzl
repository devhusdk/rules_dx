"""Canonical version pins stay equal to MODULE.bazel literals."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":dotnet.bzl", _MOD_DOTNET_VERSION = "DOTNET_VERSION", _MOD_RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION")
load(":java-scala-kotlin.bzl", _MOD_SCALA_VERSION = "SCALA_VERSION")
load(":js.bzl", _MOD_TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION")
load(":powershell.bzl", _MOD_PWSH_VERSION = "PWSH_VERSION")
load(":python.bzl", _MOD_PYTHON_VERSION = "PYTHON_VERSION")
load(":ruby.bzl", _MOD_RUBY_VERSION = "RUBY_VERSION", _MOD_RULES_RUBY_VERSION = "RULES_RUBY_VERSION")
load(":rust.bzl", _MOD_RUST_VERSION = "RUST_VERSION")
load(":toolchains.bzl", _MOD_GO_SDK_VERSION = "GO_SDK_VERSION")
load(":versions.bzl", _ASPECT_RULES_JEST_VERSION = "ASPECT_RULES_JEST_VERSION", _ASPECT_RULES_JS_VERSION = "ASPECT_RULES_JS_VERSION", _ASPECT_RULES_PY_VERSION = "ASPECT_RULES_PY_VERSION", _ASPECT_RULES_TS_VERSION = "ASPECT_RULES_TS_VERSION", _BAZEL_LIB_VERSION = "BAZEL_LIB_VERSION", _BAZEL_SKYLIB_VERSION = "BAZEL_SKYLIB_VERSION", _BAZEL_VERSION = "BAZEL_VERSION", _DOTNET_VERSION = "DOTNET_VERSION", _GAZELLE_VERSION = "GAZELLE_VERSION", _GOOGLETEST_VERSION = "GOOGLETEST_VERSION", _GO_SDK_VERSION = "GO_SDK_VERSION", _PLATFORMS_VERSION = "PLATFORMS_VERSION", _PWSH_VERSION = "PWSH_VERSION", _PYTHON_VERSION = "PYTHON_VERSION", _RUBY_VERSION = "RUBY_VERSION", _RULES_CC_VERSION = "RULES_CC_VERSION", _RULES_DOTNET_VERSION = "RULES_DOTNET_VERSION", _RULES_GO_VERSION = "RULES_GO_VERSION", _RULES_JAVA_VERSION = "RULES_JAVA_VERSION", _RULES_JVM_EXTERNAL_VERSION = "RULES_JVM_EXTERNAL_VERSION", _RULES_KOTLIN_VERSION = "RULES_KOTLIN_VERSION", _RULES_POWERSHELL_VERSION = "RULES_POWERSHELL_VERSION", _RULES_PROTO_VERSION = "RULES_PROTO_VERSION", _RULES_PYTHON_VERSION = "RULES_PYTHON_VERSION", _RULES_RUBY_VERSION = "RULES_RUBY_VERSION", _RULES_RUST_PROST_VERSION = "RULES_RUST_PROST_VERSION", _RULES_RUST_VERSION = "RULES_RUST_VERSION", _RULES_SCALA_VERSION = "RULES_SCALA_VERSION", _RULES_SHELL_VERSION = "RULES_SHELL_VERSION", _RUST_VERSION = "RUST_VERSION", _SCALA_VERSION = "SCALA_VERSION", _TYPESCRIPT_INTEGRITY = "TYPESCRIPT_INTEGRITY", _TYPESCRIPT_VERSION = "TYPESCRIPT_VERSION")

def versions_contract_tests(name):
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
            expect_equal("scala module follows canonical", _MOD_SCALA_VERSION, _SCALA_VERSION),
        ],
        file_checks = {
            "//:.bazelversion": _BAZEL_VERSION,
            "//:MODULE.bazel": "name = \"rules_rust\", version = \"" + _RULES_RUST_VERSION + "\"\n" +
                               "name = \"rules_cc\", version = \"" + _RULES_CC_VERSION + "\"\n" +
                               "name = \"googletest\", version = \"" + _GOOGLETEST_VERSION + "\"\n" +
                               "name = \"rules_python\", version = \"" + _RULES_PYTHON_VERSION + "\"\n" +
                               "name = \"rules_go\", version = \"" + _RULES_GO_VERSION + "\"\n" +
                               "name = \"gazelle\", version = \"" + _GAZELLE_VERSION + "\"\n" +
                               "name = \"rules_shell\", version = \"" + _RULES_SHELL_VERSION + "\"\n" +
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
                               "name = \"aspect_rules_py\", version = \"" + _ASPECT_RULES_PY_VERSION + "\"\n" +
                               "name = \"aspect_rules_js\", version = \"" + _ASPECT_RULES_JS_VERSION + "\"\n" +
                               "name = \"aspect_rules_ts\", version = \"" + _ASPECT_RULES_TS_VERSION + "\"\n" +
                               "name = \"aspect_rules_jest\", version = \"" + _ASPECT_RULES_JEST_VERSION + "\"\n" +
                               "dotnet_version = \"" + _DOTNET_VERSION + "\"\n" +
                               "version = \"" + _GO_SDK_VERSION + "\"\n" +
                               "go" + _GO_SDK_VERSION + ".linux-amd64.tar.gz\n" +
                               "version = \"" + _PWSH_VERSION + "\"\n" +
                               "version = \"" + _RUBY_VERSION + "\"\n" +
                               "version = \"" + _TYPESCRIPT_VERSION + "\"\n" +
                               _TYPESCRIPT_INTEGRITY + "\n" +
                               "python_version = \"" + _PYTHON_VERSION + "\"\n" +
                               "versions = [\"" + _RUST_VERSION + "\"]\n" +
                               "scala_version = \"" + _SCALA_VERSION + "\"",
        },
    )
