"""Pinned Cap'n Proto release; the capnp compiler is built from this source archive.

The archive is the upstream tag. Its Bazel files live under c++/, so the
repository strips that prefix, and MODULE.bazel rewrites the
`@capnp-cpp//` self-references that cannot resolve outside the BCR module.
"""

CAPNP_VERSION = "1.5.0"
CAPNP_SHA256 = "d5ebdf858e9885c33d4b3f765006d68bd66e9b002bf4d607ff4317ef9c1aac6a"
CAPNP_STRIP_PREFIX = "capnproto-1.5.0/c++"
CAPNP_URL = "https://github.com/capnproto/capnproto/archive/refs/tags/v1.5.0.tar.gz"

# capnpc and capnp Rust crates (the capnp crate version is the release tag).
CAPNP_RUST_VERSION = "0.27.3"
CAPNP_RUST_SHA256 = "5251a6e7d6e52617c33bfaa523a3abd862307720c481779c24c2162d342d63f5"
CAPNP_RUST_STRIP_PREFIX = "capnproto-rust-capnp-v0.27.3"
CAPNP_RUST_URL = "https://github.com/capnproto/capnproto-rust/archive/refs/tags/capnp-v0.27.3.tar.gz"

# capnpc-java plugin, the Java runtime and java.capnp.
CAPNP_JAVA_VERSION = "0.1.16"
CAPNP_JAVA_SHA256 = "904525c4433fb1779bbc4face11887b016f949f1b019755b1886c5f367f4c091"
CAPNP_JAVA_STRIP_PREFIX = "capnproto-java-0.1.16"
CAPNP_JAVA_URL = "https://github.com/capnproto/capnproto-java/archive/refs/tags/v0.1.16.tar.gz"
