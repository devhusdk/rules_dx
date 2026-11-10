"""Unit tests for oci_image_from_binary."""

load("//core:defs.bzl", "expect_equal", "starlark_test")
load(":image.bzl", "image_binary_name", "image_default_main", "image_entrypoint", "image_runtime_dest")
load(":validate_image.bzl", "image_options_error")

_AMD64 = str(Label("//deploy:linux_amd64"))

_ARM64 = str(Label("//deploy:linux_arm64"))

def image_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "image_binary_name reads the target name",
                image_binary_name("//app:server"),
                "server",
            ),
            expect_equal(
                "image_binary_name reads a bare package label",
                image_binary_name("//app/server"),
                "server",
            ),
            expect_equal(
                "image_binary_name reads a local label",
                image_binary_name(":server"),
                "server",
            ),
            expect_equal(
                "image_default_main names the python script of a label",
                image_default_main("python", "//app:server", "other"),
                "app/server.py",
            ),
            expect_equal(
                "image_default_main uses the package for a local label",
                image_default_main("node", ":server", "app"),
                "app/server.js",
            ),
            expect_equal(
                "image_default_main reads a bare package label",
                image_default_main("python", "//app/server", ""),
                "app/server/server.py",
            ),
            expect_equal(
                "image_default_main reads a bare package dotnet label",
                image_default_main("dotnet", "//app/server", ""),
                "app/server/server/net10.0/server.dll",
            ),
            expect_equal(
                "image_default_main reads a bare package node label",
                image_default_main("node", "//app/server", ""),
                "app/server/server.js",
            ),
            expect_equal(
                "image_default_main reads a root label",
                image_default_main("python", "//:server", "app"),
                "server.py",
            ),
            expect_equal(
                "image_default_main names the dotnet assembly of a label",
                image_default_main("dotnet", "//app:server", "other"),
                "app/server/net10.0/server.dll",
            ),
            expect_equal(
                "image_default_main leaves java alone",
                image_default_main("java", ":server", "app"),
                None,
            ),
            expect_equal(
                "image_options_error accepts the defaults",
                image_options_error(":server", "static", [_AMD64]),
                "",
            ),
            expect_equal(
                "image_options_error accepts two platforms",
                image_options_error(":server", "cc", [_AMD64, _ARM64]),
                "",
            ),
            expect_equal(
                "image_options_error rejects an unknown base",
                image_options_error(":server", "alpine", [_AMD64]),
                "oci_image_from_binary: invalid base 'alpine': want one of cc, dotnet, java, node, python, static",
            ),
            expect_equal(
                "image_options_error rejects an empty platform list",
                image_options_error(":server", "static", []),
                "oci_image_from_binary: want at least one platform, one of @rules_dx//deploy:linux_amd64, @rules_dx//deploy:linux_arm64",
            ),
            expect_equal(
                "image_options_error rejects an unknown platform",
                image_options_error(":server", "static", ["//other:windows"]),
                "oci_image_from_binary: invalid platform '//other:windows': want one of @rules_dx//deploy:linux_amd64, @rules_dx//deploy:linux_arm64",
            ),
            expect_equal(
                "image_options_error rejects a repeated platform",
                image_options_error(":server", "static", [_AMD64, _AMD64]),
                "oci_image_from_binary: want each platform once, got " + _AMD64 + ", " + _AMD64,
            ),
            expect_equal(
                "image_options_error accepts a python base with main",
                image_options_error(":server", "python", [_AMD64], main = "app/server.py"),
                "",
            ),
            expect_equal(
                "image_options_error wants main for python",
                image_options_error(":server", "python", [_AMD64]),
                "oci_image_from_binary: base 'python' needs main, the workspace path of the entry script",
            ),
            expect_equal(
                "image_options_error accepts a java base with main_class",
                image_options_error(":server", "java", [_AMD64], main_class = "app.Server"),
                "",
            ),
            expect_equal(
                "image_options_error leaves java main_class to the binary",
                image_options_error(":server", "java", [_AMD64]),
                "",
            ),
            expect_equal(
                "image_options_error rejects main on a static base",
                image_options_error(":server", "static", [_AMD64], main = "server.py"),
                "oci_image_from_binary: main and main_class only apply to the python, java, node and dotnet bases",
            ),
            expect_equal(
                "image_entrypoint runs a binary from /app",
                image_entrypoint("static", "server"),
                (["/app/server"], {}),
            ),
            expect_equal(
                "image_entrypoint runs python from the runfiles",
                image_entrypoint("python", "server", main = "app/server.py"),
                (
                    ["/usr/bin/python3", "/app/server.runfiles/_main/app/server.py"],
                    {"PYTHONPATH": "/app/server.runfiles/_main"},
                ),
            ),
            expect_equal(
                "image_options_error accepts a node base with main",
                image_options_error(":server", "node", [_AMD64], main = "app/server.js"),
                "",
            ),
            expect_equal(
                "image_options_error wants main for node",
                image_options_error(":server", "node", [_AMD64]),
                "oci_image_from_binary: base 'node' needs main, the workspace path of the entry script",
            ),
            expect_equal(
                "image_entrypoint runs node from the runfiles",
                image_entrypoint("node", "server", main = "app/server.js"),
                (["/nodejs/bin/node", "/app/server.runfiles/_main/app/server.js"], {}),
            ),
            expect_equal(
                "image_options_error wants main for dotnet",
                image_options_error(":server", "dotnet", [_AMD64]),
                "oci_image_from_binary: base 'dotnet' needs main, the workspace path of the entry script",
            ),
            expect_equal(
                "image_entrypoint runs dotnet on the assembly in the runfiles",
                image_entrypoint("dotnet", "server", main = "app/server/net10.0/server.dll"),
                (["dotnet", "/app/server.dll.runfiles/_main/app/server/net10.0/server.dll"], {}),
            ),
            expect_equal(
                "image_entrypoint runs java from the jar directory",
                image_entrypoint("java", "server", main_class = "app.Server"),
                (["/usr/bin/java", "-cp", "/app/lib/*", "app.Server"], {}),
            ),
            expect_equal(
                "image_entrypoint reads the java main class from an argument file",
                image_entrypoint("java", "server"),
                (["/usr/bin/java", "-cp", "/app/lib/*", "@/app/lib/main.args"], {}),
            ),
            expect_equal(
                "image_options_error rejects runtime on a static base",
                image_options_error(":server", "static", [_AMD64], runtime = True),
                "oci_image_from_binary: runtime only applies to the python, java, node and dotnet bases",
            ),
            expect_equal(
                "image_options_error accepts runtime on a java base",
                image_options_error(":server", "java", [_AMD64], runtime = True),
                "",
            ),
            expect_equal(
                "image_entrypoint runs python from the runtime layer",
                image_entrypoint("python", "server", main = "app/server.py", runtime = True),
                (
                    ["/opt/python/bin/python3", "/app/server.runfiles/_main/app/server.py"],
                    {"PYTHONPATH": "/app/server.runfiles/_main"},
                ),
            ),
            expect_equal(
                "image_entrypoint runs java from the runtime layer",
                image_entrypoint("java", "server", main_class = "app.Server", runtime = True),
                (["/opt/java/bin/java", "-cp", "/app/lib/*", "app.Server"], {}),
            ),
            expect_equal(
                "image_entrypoint runs node from the runtime layer",
                image_entrypoint("node", "server", main = "app/server.js", runtime = True),
                (["/opt/node/bin/node", "/app/server.runfiles/_main/app/server.js"], {}),
            ),
            expect_equal(
                "image_entrypoint points dotnet at its runtime layer",
                image_entrypoint("dotnet", "server", main = "a.dll", runtime = True),
                (
                    ["/opt/dotnet/dotnet", "/app/server.dll.runfiles/_main/a.dll"],
                    {"DOTNET_ROOT": "/opt/dotnet", "DOTNET_SYSTEM_GLOBALIZATION_INVARIANT": "1"},
                ),
            ),
            expect_equal(
                "image_runtime_dest keeps a listed directory",
                image_runtime_dest("lib/modules", {"include": ["lib/"], "strip": ""}, "/opt/java"),
                "/opt/java/lib/modules",
            ),
            expect_equal(
                "image_runtime_dest drops unlisted files",
                image_runtime_dest("jmods/java.base.jmod", {"include": ["lib/"], "strip": ""}, "/opt/java"),
                None,
            ),
            expect_equal(
                "image_runtime_dest strips the node prefix",
                image_runtime_dest("bin/nodejs/bin/node", {"include": ["bin/"], "strip": "bin/nodejs/"}, "/opt/node"),
                "/opt/node/bin/node",
            ),
            expect_equal(
                "image_runtime_dest drops listed prefixes",
                image_runtime_dest("lib/libpython3.12.so", {"drop": ["lib/libpython"], "include": ["lib/"], "strip": ""}, "/opt/python"),
                None,
            ),
            expect_equal(
                "image_runtime_dest matches a file exactly",
                image_runtime_dest("dotnet", {"include": ["dotnet"], "strip": ""}, "/opt/dotnet"),
                "/opt/dotnet/dotnet",
            ),
        ],
    )
