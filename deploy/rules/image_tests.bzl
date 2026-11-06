"""Unit tests for oci_image_from_binary."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":image.bzl", "image_binary_name", "image_default_main", "image_entrypoint", "image_options_error")

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
                "app/server.py",
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
                image_options_error(":server", "static", ["linux/amd64"]),
                "",
            ),
            expect_equal(
                "image_options_error accepts two platforms",
                image_options_error(":server", "cc", ["linux/amd64", "linux/arm64"]),
                "",
            ),
            expect_equal(
                "image_options_error rejects an unknown base",
                image_options_error(":server", "alpine", ["linux/amd64"]),
                "oci_image_from_binary: invalid base 'alpine': want one of cc, dotnet, java, node, python, static",
            ),
            expect_equal(
                "image_options_error rejects an empty platform list",
                image_options_error(":server", "static", []),
                "oci_image_from_binary: want at least one platform, one of linux/amd64, linux/arm64",
            ),
            expect_equal(
                "image_options_error rejects an unknown platform",
                image_options_error(":server", "static", ["windows/amd64"]),
                "oci_image_from_binary: invalid platform 'windows/amd64': want one of linux/amd64, linux/arm64",
            ),
            expect_equal(
                "image_options_error rejects a repeated platform",
                image_options_error(":server", "static", ["linux/amd64", "linux/amd64"]),
                "oci_image_from_binary: want each platform once, got linux/amd64, linux/amd64",
            ),
            expect_equal(
                "image_options_error accepts a python base with main",
                image_options_error(":server", "python", ["linux/amd64"], main = "app/server.py"),
                "",
            ),
            expect_equal(
                "image_options_error wants main for python",
                image_options_error(":server", "python", ["linux/amd64"]),
                "oci_image_from_binary: base 'python' needs main, the workspace path of the entry script",
            ),
            expect_equal(
                "image_options_error accepts a java base with main_class",
                image_options_error(":server", "java", ["linux/amd64"], main_class = "app.Server"),
                "",
            ),
            expect_equal(
                "image_options_error leaves java main_class to the binary",
                image_options_error(":server", "java", ["linux/amd64"]),
                "",
            ),
            expect_equal(
                "image_options_error rejects main on a static base",
                image_options_error(":server", "static", ["linux/amd64"], main = "server.py"),
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
                image_options_error(":server", "node", ["linux/amd64"], main = "app/server.js"),
                "",
            ),
            expect_equal(
                "image_options_error wants main for node",
                image_options_error(":server", "node", ["linux/amd64"]),
                "oci_image_from_binary: base 'node' needs main, the workspace path of the entry script",
            ),
            expect_equal(
                "image_entrypoint runs node from the runfiles",
                image_entrypoint("node", "server", main = "app/server.js"),
                (["/nodejs/bin/node", "/app/server.runfiles/_main/app/server.js"], {}),
            ),
            expect_equal(
                "image_options_error wants main for dotnet",
                image_options_error(":server", "dotnet", ["linux/amd64"]),
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
        ],
    )
