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
