"""Pinned LDC compiler and rules_d identities."""

RULES_D_VERSION = "0.12.1"

RULES_D_SHA256 = "87aaf9bc99f0635df22efcbea67a95bce2d8b4fad172012e0314e2cb79149816"

RULES_D_URL = "https://github.com/bazel-contrib/rules_d/archive/refs/tags/v" + RULES_D_VERSION + ".tar.gz"

LDC_VERSION = "1.43.0"

LDC_RELEASES = "https://github.com/ldc-developers/ldc/releases/download/v" + LDC_VERSION + "/"

LDC_CELLS = {
    "linux_amd64": struct(
        archive = "ldc2-" + LDC_VERSION + "-linux-x86_64.tar.xz",
        runtime = ["gcc-12-base", "libgcc-s1", "zlib1g"],
        sha256 = "35805db60f47fd1a162b2035da84d96beec37e6d1e7b1edf60498a90acfaedce",
        strip_prefix = "ldc2-" + LDC_VERSION + "-linux-x86_64",
    ),
}

DFMT_VERSION = "0.15.2"

LIBDPARSE_VERSION = "0.25.1"

TOOL_SOURCES = {
    "dx_d_dfmt": struct(
        dropped = ["bin", "tests", "dub.json", "dub.selections.json", "makefile", "Makefile"],
        sha256 = "baf8da47a743b0f4a27629bd42b3389a82e91e4d4f88b4ce9487dc15b1518f1f",
        strip_prefix = "dfmt-" + DFMT_VERSION,
        url = "https://github.com/dlang-community/dfmt/archive/refs/tags/v" + DFMT_VERSION + ".tar.gz",
    ),
    "dx_d_libdparse": struct(
        dropped = ["dub.json", "dub.selections.json", "makefile", "Makefile", "tests"],
        sha256 = "83825fda4ce67cac49e1bfdce0051a9e2499a5fadbf3a7f4d0853b487eaebc82",
        strip_prefix = "libdparse-" + LIBDPARSE_VERSION,
        url = "https://github.com/dlang-community/libdparse/archive/refs/tags/v" + LIBDPARSE_VERSION + ".tar.gz",
    ),
}
