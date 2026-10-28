"""Pinned FlatBuffers release shared by flatc and every runtime.

flatc, the C++ runtime and the Rust runtime all come from this one archive, so
bumping it moves them together. The Rust runtime overlay reuses the same url
and sha256 (Bazel's repository cache is keyed by sha256, so nothing is
downloaded twice) with a different strip_prefix.
"""

FLATBUFFERS_VERSION = "25.12.19"
FLATBUFFERS_SHA256 = "f81c3162b1046fe8b84b9a0dbdd383e24fdbcf88583b9cb6028f90d04d90696a"
FLATBUFFERS_STRIP_PREFIX = "flatbuffers-25.12.19"
FLATBUFFERS_RUST_STRIP_PREFIX = "flatbuffers-25.12.19/rust/flatbuffers"
FLATBUFFERS_URL = "https://github.com/google/flatbuffers/archive/refs/tags/v25.12.19.tar.gz"

# The Rust runtime re-exports bitflags.
BITFLAGS_VERSION = "2.13.2"
BITFLAGS_SHA256 = "3ded4057c258ba199e2d26386d3af3780957ecaee6c4ef4041c6b4b8b97c0b06"
BITFLAGS_STRIP_PREFIX = "bitflags-2.13.2"
BITFLAGS_URL = "https://static.crates.io/crates/bitflags/bitflags-2.13.2.crate"
