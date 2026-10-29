"""Pinned OCaml source archives that dune builds zanuda from.

The closure is what `opam install zanuda.2.1.0` would pull in for OCaml 5.5,
minus the packages that only matter for tests, docs or plugins: resolved by
reading each package's opam `depends` (ignoring with-test, with-doc and
with-dev-setup) from opam-repository and keeping the newest version that
satisfies every constraint (zanuda.opam, ppx_expect_nobase.0.17.3.2,
ppx_inline_test_nobase.v0.17.0.3, angstrom <= 0.16.0). Archives of packages
that are not buildable with dune from the start (mtime) carry a patch that adds
the dune files. Patches live in patches/ and are named `<dir>.<name>.patch`;
they apply in file name order with -p1 inside the package directory.
"""

# The OCaml versions zanuda 2.1.0 accepts (zanuda.opam): [lo, hi) pairs. The
# .cmt format is tied to the exact compiler, so a toolchain bump outside these
# ranges needs a newer zanuda.
ZANUDA_OCAML_RANGES = [
    ("4.14.2", "5.0.0"),
    ("5.3.0", "5.4.0"),
    ("5.5.0", "5.6.0"),
]

ZANUDA_VERSION = "2.1.0"

# One struct per archive: the directory it unpacks to, its opam name, version, url, sha256 and SPDX license.
DUNIVERSE = [
    struct(
        dir = "dune_src",
        name = "dune",
        version = "3.24.2",
        url = "https://github.com/ocaml/dune/releases/download/3.24.2/dune-3.24.2.tbz",
        sha256 = "472798691b0216daf538709f0f4703b3617ef24ad0866c9096068baaba4d762a",
        license = "MIT",
    ),
    struct(
        dir = "base",
        name = "base",
        version = "v0.17.3",
        url = "https://github.com/janestreet/base/archive/refs/tags/v0.17.3.tar.gz",
        sha256 = "3f3f64789ab25bb5cb7f5f907dd651dec9cc9440981822fe75df6b94344b7208",
        license = "MIT",
    ),
    struct(
        dir = "sexplib0",
        name = "sexplib0",
        version = "v0.17.0",
        url = "https://github.com/janestreet/sexplib0/archive/refs/tags/v0.17.0.tar.gz",
        sha256 = "5b0910b5dab8ec63633be5dbf92a3e4863d415d803cad9dddf99dba43ce7498b",
        license = "MIT",
    ),
    struct(
        dir = "sexplib",
        name = "sexplib",
        version = "v0.17.0",
        url = "https://github.com/janestreet/sexplib/archive/refs/tags/v0.17.0.tar.gz",
        sha256 = "da863b42b81235fdcf45eb32c04fb8bde22ff446a779cfb6f989730a35103160",
        license = "MIT",
    ),
    struct(
        dir = "parsexp",
        name = "parsexp",
        version = "v0.17.0",
        url = "https://github.com/janestreet/parsexp/archive/refs/tags/v0.17.0.tar.gz",
        sha256 = "a3d10edbc4f98d16357b644d550fd1c06f4d9aa4990ab8ee6da01276c24d55b5",
        license = "MIT",
    ),
    struct(
        dir = "stdio",
        name = "stdio",
        version = "v0.17.0",
        url = "https://github.com/janestreet/stdio/archive/refs/tags/v0.17.0.tar.gz",
        sha256 = "e7cb473d4bffcf419f307c658cf2599fab03a2b4fe655bfd0be699f8f7af176e",
        license = "MIT",
    ),
    struct(
        dir = "ocaml_intrinsics_kernel",
        name = "ocaml_intrinsics_kernel",
        version = "v0.17.3",
        url = "https://github.com/janestreet/ocaml_intrinsics_kernel/archive/refs/tags/v0.17.3.tar.gz",
        sha256 = "2a967316c51b0f3fec37f42f43b362d87d9ae536ce98e0e921a65c8bbc83cd85",
        license = "MIT",
    ),
    struct(
        dir = "ppx_optcomp",
        name = "ppx_optcomp",
        version = "v0.17.1",
        url = "https://github.com/janestreet/ppx_optcomp/archive/refs/tags/v0.17.1.tar.gz",
        sha256 = "2efe65a0210ed7bda8ddd9e076385b76619f23ac3aee1db5b6140ac1a1feb99d",
        license = "MIT",
    ),
    struct(
        dir = "ppx_sexp_conv",
        name = "ppx_sexp_conv",
        version = "v0.17.1",
        url = "https://github.com/janestreet/ppx_sexp_conv/archive/refs/tags/v0.17.1.tar.gz",
        sha256 = "b850556d05203aa6d97fcbe4be75abc9679b161be059e474c0d934bc0e12fe86",
        license = "MIT",
    ),
    struct(
        dir = "ppxlib_jane",
        name = "ppxlib_jane",
        version = "v0.17.4",
        url = "https://github.com/janestreet/ppxlib_jane/archive/refs/tags/v0.17.4.tar.gz",
        sha256 = "10c4d1a162a8f51eecf35d68118a96aa5f375350ff3abb357b41968faa7b4ef3",
        license = "MIT",
    ),
    struct(
        dir = "ppxlib",
        name = "ppxlib",
        version = "0.39.0",
        url = "https://github.com/ocaml-ppx/ppxlib/releases/download/0.39.0/ppxlib-0.39.0.tbz",
        sha256 = "441a2814dbac3f5dc0c8dcefca263548e9b4d6dda1ef2990ce6a4999e108be3c",
        license = "MIT",
    ),
    struct(
        dir = "ocaml-compiler-libs",
        name = "ocaml-compiler-libs",
        version = "v0.17.0",
        url = "https://github.com/janestreet/ocaml-compiler-libs/archive/refs/tags/v0.17.0.tar.gz",
        sha256 = "9b9644d7351db699e57ddba7c767bb4153e6e988ccf45ead2fb238a3bd75cdc7",
        license = "MIT",
    ),
    struct(
        dir = "ppx_derivers",
        name = "ppx_derivers",
        version = "1.2.1",
        url = "https://github.com/ocaml-ppx/ppx_derivers/archive/1.2.1.tar.gz",
        sha256 = "b6595ee187dea792b31fc54a0e1524ab1e48bc6068d3066c45215a138cc73b95",
        license = "BSD-3-Clause",
    ),
    struct(
        dir = "ppx_inline_test_nobase",
        name = "ppx_inline_test_nobase",
        version = "v0.17.0.3",
        url = "https://github.com/Kakadu/ppx_inline_test_nobase/archive/refs/tags/v0.17.0.3.tar.gz",
        sha256 = "9a4ec14d7b62edf71bfb4be25146b930d240d3caaab34eb186c6113dbc9b4b5a",
        license = "MIT",
    ),
    struct(
        dir = "ppx_expect_nobase",
        name = "ppx_expect_nobase",
        version = "0.17.3.2",
        url = "https://github.com/Kakadu/ppx_expect_nobase/releases/download/0.17.3.2/ppx_expect_nobase-0.17.3.2.tbz",
        sha256 = "d918035e8cc2765e691c1e9d22bda5983569ec7bc7c2da60424b4207c7c71aae",
        license = "MIT",
    ),
    struct(
        dir = "yojson",
        name = "yojson",
        version = "3.0.0",
        url = "https://github.com/ocaml-community/yojson/releases/download/3.0.0/yojson-3.0.0.tbz",
        sha256 = "99414da7609b92a02474ef4b49ecda15edc8cbba5229341b124e7e4695c39610",
        license = "BSD-3-Clause",
    ),
    struct(
        dir = "angstrom",
        name = "angstrom",
        version = "0.16.0",
        url = "https://github.com/inhabitedtype/angstrom/archive/0.16.0.tar.gz",
        sha256 = "47810bd31306d57c0ac83eb3f95b1cb6f8bdda99457aa6e6bcd02f6ddb8410f0",
        license = "BSD-3-Clause",
    ),
    struct(
        dir = "bigstringaf",
        name = "bigstringaf",
        version = "0.10.0",
        url = "https://github.com/inhabitedtype/bigstringaf/archive/0.10.0.tar.gz",
        sha256 = "ed92f5b05fbc11b9defcec734d59b1068f3717a9ae4f9705c16c7f7ac3729f28",
        license = "BSD-3-Clause",
    ),
    struct(
        dir = "ocaml-syntax-shims",
        name = "ocaml-syntax-shims",
        version = "1.0.0",
        url = "https://github.com/ocaml-ppx/ocaml-syntax-shims/releases/download/1.0.0/ocaml-syntax-shims-1.0.0.tbz",
        sha256 = "89b2e193e90a0c168b6ec5ddf6fef09033681bdcb64e11913c97440a2722e8c8",
        license = "MIT",
    ),
    struct(
        dir = "csexp",
        name = "csexp",
        version = "1.5.2",
        url = "https://github.com/ocaml-dune/csexp/releases/download/1.5.2/csexp-1.5.2.tbz",
        sha256 = "1a14dd04bb4379a41990248550628c77913a9c07f3c35c1370b6960e697787ff",
        license = "MIT",
    ),
    struct(
        dir = "mtime",
        name = "mtime",
        version = "2.2.0",
        url = "https://erratique.ch/software/mtime/releases/mtime-2.2.0.tbz",
        sha256 = "f9210a07c4a3eb1756a45fa8a32974d9b5ddad3583c3400442fdfe2c9d63d636",
        license = "ISC",
    ),
    struct(
        dir = "zanuda",
        name = "zanuda",
        version = "2.1.0",
        url = "https://github.com/Kakadu/zanuda/archive/refs/tags/v2.1.0.tar.gz",
        sha256 = "a6dc79c02d89b7d3eb771a409256300a36b6931eae37b74b39f8effc18b355ce",
        license = "LGPL-3.0-only",
    ),
]
