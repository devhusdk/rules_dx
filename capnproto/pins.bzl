"""Pinned Cap'n Proto release; the capnp compiler is built from this source archive.

The archive is the upstream tag. Its Bazel files live under c++/, so the
repository strips that prefix, and MODULE.bazel rewrites the
`@capnp-cpp//` self-references that cannot resolve outside the BCR module.
"""

CAPNP_VERSION = "1.5.0"
CAPNP_SHA256 = "d5ebdf858e9885c33d4b3f765006d68bd66e9b002bf4d607ff4317ef9c1aac6a"
CAPNP_STRIP_PREFIX = "capnproto-1.5.0/c++"
CAPNP_URL = "https://github.com/capnproto/capnproto/archive/refs/tags/v1.5.0.tar.gz"
