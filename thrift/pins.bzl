"""Pinned Apache Thrift release; the thrift compiler is built from this source archive.

The archive is the Apache release tarball, not the GitHub tag archive: only the
release tarball ships the generated parser (thrifty.cc, thrifty.hh, thriftl.cc),
so no bison or flex toolchain is needed. The same archive carries the C++, Rust
and Java generators and runtimes for later codegen work.
"""

THRIFT_VERSION = "0.25.0"
THRIFT_SHA256 = "66da4707214c54c94bac082103dc67adaf9e08925662700f269170a7b534b214"
THRIFT_STRIP_PREFIX = "thrift-0.25.0"
THRIFT_URL = "https://archive.apache.org/dist/thrift/0.25.0/thrift-0.25.0.tar.gz"
