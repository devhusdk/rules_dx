"""Pinned LDC compiler and rules_d identities."""

RULES_D_VERSION = "0.12.1"

RULES_D_SHA256 = "87aaf9bc99f0635df22efcbea67a95bce2d8b4fad172012e0314e2cb79149816"

RULES_D_URL = "https://github.com/bazel-contrib/rules_d/archive/refs/tags/v" + RULES_D_VERSION + ".tar.gz"

LDC_VERSION = "1.43.0"

LDC_RELEASES = "https://github.com/ldc-developers/ldc/releases/download/v" + LDC_VERSION + "/"

LDC_CELLS = {
    "linux_amd64": struct(
        archive = "ldc2-" + LDC_VERSION + "-linux-x86_64.tar.xz",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        linkopts = ["-ldl", "-lm", "-lpthread", "-lrt"],
        runtime = ["gcc-12-base", "libgcc-s1", "zlib1g"],
        sha256 = "35805db60f47fd1a162b2035da84d96beec37e6d1e7b1edf60498a90acfaedce",
        strip_prefix = "ldc2-" + LDC_VERSION + "-linux-x86_64",
    ),
    "linux_arm64": struct(
        archive = "ldc2-" + LDC_VERSION + "-linux-aarch64.tar.xz",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        linkopts = ["-ldl", "-lm", "-lpthread", "-lrt"],
        runtime = ["gcc-12-base", "libgcc-s1", "zlib1g"],
        sha256 = "f2b24edc254d4b9944234f641de3c4b3a36d5752639ec30cb2bbe122931b2283",
        strip_prefix = "ldc2-" + LDC_VERSION + "-linux-aarch64",
    ),
    "macos_arm64": struct(
        archive = "ldc2-" + LDC_VERSION + "-osx-arm64.tar.xz",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        linkopts = ["-ldl", "-lm", "-lpthread"],
        runtime = [],
        sha256 = "5e332349c784d4e0b1c4622284803880f89d97ef1ef97c6b25aa4d6b094c5fed",
        strip_prefix = "ldc2-" + LDC_VERSION + "-osx-arm64",
    ),
}

DFMT_VERSION = "0.15.2"

LIBDPARSE_VERSION = "0.25.1"

DSCANNER_VERSION = "0.16.0-beta.5"

DCD_VERSION = "0.16.2"

CONTAINERS_VERSION = "0.9.0"

INIFILED_VERSION = "1.3.3"

LIBDDOC_VERSION = "0.8.0"

TOOL_SOURCES = {
    "dx_d_containers": struct(
        dropped = ["doc-src", "test", "times.png", "dub.sdl", "meson.build"],
        files = {},
        sha256 = "5e256b84bbdbd2bd625cba0472ea27a1fde6d673d37a85fe971a20d52874acaa",
        strip_prefix = "containers-" + CONTAINERS_VERSION,
        url = "https://github.com/dlang-community/containers/archive/refs/tags/v" + CONTAINERS_VERSION + ".tar.gz",
    ),
    "dx_d_dcd": struct(
        # Only the BSL-1.0 dsymbol subpackage is kept; the GPL-3.0 client and server never reach a build.
        dropped = [
            "CHANGELOG.md",
            "README.md",
            "bash-completion",
            "build.bat",
            "build.sh",
            "buildloop.sh",
            "ci",
            "common",
            "constants-gen",
            "containers",
            "d-test-utils",
            "dub.json",
            "dub.selections.json",
            "installer",
            "libdparse",
            "makefile",
            "man1",
            "msgpack-d",
            "sonar-project.properties",
            "src",
            "teaser.png",
            "tests",
            "dsymbol/subprojects",
        ],
        files = {},
        sha256 = "3c333269fdd8b3b2f7278d4f3a5693ec8450547c2091312a805b9605149b205a",
        strip_prefix = "DCD-" + DCD_VERSION,
        url = "https://github.com/dlang-community/DCD/archive/refs/tags/v" + DCD_VERSION + ".tar.gz",
    ),
    "dx_d_dfmt": struct(
        dropped = ["bin", "tests", "dub.json", "dub.selections.json", "makefile", "Makefile"],
        files = {"githash.txt": "v" + DFMT_VERSION + "\n"},
        sha256 = "baf8da47a743b0f4a27629bd42b3389a82e91e4d4f88b4ce9487dc15b1518f1f",
        strip_prefix = "dfmt-" + DFMT_VERSION,
        url = "https://github.com/dlang-community/dfmt/archive/refs/tags/v" + DFMT_VERSION + ".tar.gz",
    ),
    "dx_d_dscanner": struct(
        dropped = [
            "DCD",
            "Dockerfile",
            "appveyor.yml",
            "build.bat",
            "containers",
            "d-test-utils",
            "dsc.sublime-project",
            "dub.json",
            "dub.selections.json",
            "dubhash.d",
            "inifiled",
            "libddoc",
            "libdparse",
            "makefile",
            "release-windows.sh",
            "release.sh",
            "scripts",
            "setup-ldc-windows.sh",
            "sonar-project.properties",
            "tests",
        ],
        files = {"githash.txt": "v" + DSCANNER_VERSION + "\n"},
        sha256 = "6397d15eb3e2dc6b82dba4c830793db9299b6f4025099f16b0c5482bd39651a2",
        strip_prefix = "D-Scanner-" + DSCANNER_VERSION,
        url = "https://github.com/dlang-community/D-Scanner/archive/refs/tags/v" + DSCANNER_VERSION + ".tar.gz",
    ),
    "dx_d_inifiled": struct(
        dropped = ["test", "package.json"],
        files = {},
        sha256 = "017432594651ab12abaf93aae0918c3d609d0a65800d79168e0b4bd0326ca136",
        strip_prefix = "inifiled-" + INIFILED_VERSION,
        url = "https://github.com/burner/inifiled/archive/refs/tags/v" + INIFILED_VERSION + ".tar.gz",
    ),
    "dx_d_libddoc": struct(
        dropped = ["Makefile", "d-test-utils", "dub.json"],
        files = {},
        sha256 = "9e5657de8aea225f622a00b5cb5ca9f1dac091dd98970152d65876a92669e18e",
        strip_prefix = "libddoc-" + LIBDDOC_VERSION,
        url = "https://github.com/dlang-community/libddoc/archive/refs/tags/v" + LIBDDOC_VERSION + ".tar.gz",
    ),
    "dx_d_libdparse": struct(
        dropped = ["dub.json", "dub.selections.json", "makefile", "Makefile", "tests"],
        files = {},
        sha256 = "83825fda4ce67cac49e1bfdce0051a9e2499a5fadbf3a7f4d0853b487eaebc82",
        strip_prefix = "libdparse-" + LIBDPARSE_VERSION,
        url = "https://github.com/dlang-community/libdparse/archive/refs/tags/v" + LIBDPARSE_VERSION + ".tar.gz",
    ),
}
