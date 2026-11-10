"""OCaml toolchain pin and acquisition tests."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "hub_checks")
load(":ocaml.bzl", "COMPILERS", "NATIVE_CC_TOOLS", "TOOLS", "native_cc_env", "toolchain_fields")
load(":opam.bzl", "is_toolchain_package", "parse_depend_names", "parse_depends", "parse_url")
load(":pins.bzl", "OCAML_CELLS", "OCAML_VERSION")
load(":repos.bzl", "DROPPED_PATHS", "EMITTERS", "conda_stem", "dune_package_build", "hub_build", "package_repo_name", "packages_hub_build", "placeholder_files", "relative_prefix")

_LOCK = """opam-version: "2.0"
name: "app"
depends: [
  "astring" {= "0.8.5"}
  "cmdliner" {with-test & = "1.2.0"}
  "cmdliner" {= "1.3.0"}
  "alcotest" {with-test & = "1.7.0"}
  "odoc" {dev}
  "dune" {= "3.16.0"}
  "ocaml" {= "5.5.2" & post}
  "lwt" {>= "5.0"}
]
build: ["dune" "build"]
"""

_URL_OPAM = """opam-version: "2.0"
url {
  src: "https://example.org/a-1.tbz"
  checksum: [
    "sha256=ab12"
    "md5=ff"
  ]
}
"""

def _checks():
    linux = OCAML_CELLS["linux_x86_64"]
    ocaml = [p for p in linux.packages if p.name == "ocaml"][0]
    macos = {p.name: p for p in OCAML_CELLS["macos_arm64"].packages}
    return hub_checks(hub_build(), "//toolchain/type:std", 2 * len([e for c in OCAML_CELLS.values() for e in c.emitters]), ["@rules_ocaml"]) + [
        expect_equal("linux emitters", linux.emitters, ["sys", "vm"]),
        expect_equal("known emitters", sorted(EMITTERS.keys()), ["sys", "vm"]),
        expect_equal("linux packages", sorted([p.name for p in linux.packages]), ["ocaml", "zstd"]),
        expect_contains("ocaml package names the version", ocaml.archive.urls[0], "ocaml_linux-64-" + OCAML_VERSION + "-"),
        expect_equal("macos packages", sorted(macos.keys()), ["ocaml", "zstd", "zstd-static"]),
        expect_contains("macos zstd version", macos["zstd"].archive.urls[0], "/zstd-1.5.7-"),
        expect_contains("macos zstd-static matches the zstd version", macos["zstd-static"].archive.urls[0], "/zstd-static-1.5.7-"),
        expect_equal("ocaml license", ocaml.license, "LGPL-2.1-only WITH OCaml-LGPL-linking-exception"),
        expect_true("the upstream module file is dropped", "MODULE.bazel" in DROPPED_PATHS),
        expect_equal("bytecode compiler", TOOLS["compiler"], "bin/ocamlc.opt"),
        expect_equal("native compiler", TOOLS["native_compiler"], "bin/ocamlopt.opt"),
        expect_equal("each emitter has its compiler", COMPILERS, {"sys": "bin/ocamlopt.opt", "vm": "bin/ocamlc.opt"}),
        expect_equal(
            "native C environment names every tool and puts only the driver directory on PATH",
            native_cc_env("clang", "as-clang", "ld-clang", "llvm-ar", "bin/tc_cc", lib = "ext/dist/lib"),
            {
                "CONDA_OCAML_AR": "llvm-ar",
                "CONDA_OCAML_AS": "as-clang -c",
                "CONDA_OCAML_CC": "clang",
                "CONDA_OCAML_MKDLL": "ld-clang -shared",
                "CONDA_OCAML_MKEXE": "ld-clang",
                "DX_OCAML_LIB": "ext/dist/lib",
                "PATH": "bin/tc_cc",
            },
        ),
        expect_equal(
            "macOS links plugins against the host executable and archives through libtool",
            native_cc_env("clang", "as-clang", "ld-clang", "x/libtool", "bin/tc_cc", macos = True, link_env = {"LLVM_CLANGXX": "x/clang++"}),
            {
                "CONDA_OCAML_AR": "x/libtool",
                "CONDA_OCAML_AS": "as-clang -c",
                "CONDA_OCAML_CC": "clang",
                "CONDA_OCAML_MKDLL": "ld-clang -shared -undefined dynamic_lookup",
                "CONDA_OCAML_MKEXE": "ld-clang",
                "DX_OCAML_LIBTOOL": "x/libtool",
                "LLVM_CLANGXX": "x/clang++",
                "PATH": "bin/tc_cc",
            },
        ),
        expect_equal(
            "a macOS archive wrapper replaces the archiver",
            native_cc_env("clang", "as-clang", "ld-clang", "x/libtool", "p", macos = True, ar_wrapper = "w/libtool_ar.sh")["CONDA_OCAML_AR"],
            "w/libtool_ar.sh",
        ),
        expect_equal("the driver is linked under every tool name", NATIVE_CC_TOOLS, ["ar", "as", "cc", "ld", "mkdll", "mkexe", "ranlib"]),
        expect_equal("conda stem", conda_stem("https://x/linux-64/zstd-1.5.7-h_7.conda"), "zstd-1.5.7-h_7"),
        expect_equal("root files keep a dot prefix", relative_prefix("ld.conf"), "."),
        expect_equal("nested files climb to the root", relative_prefix("lib/ocaml/ld.conf"), "../.."),
        expect_equal(
            "placeholder files split by mode",
            placeholder_files([
                {"_path": "b", "prefix_placeholder": "/p", "file_mode": "binary"},
                {"_path": "t", "prefix_placeholder": "/p"},
                {"_path": "n"},
            ], "text"),
            ["t"],
        ),
        expect_equal("locked versions are read", parse_depends(_LOCK), {"astring": "0.8.5", "cmdliner": "1.3.0", "dune": "3.16.0", "ocaml": "5.5.2"}),
        expect_equal("a file without depends pins nothing", parse_depends("opam-version: \"2.0\"\n"), {}),
        expect_equal("an unpinned entry is skipped", parse_depends("depends: [\"a\" {>= \"1\"}]"), {}),
        expect_equal("a version after a build filter is kept", parse_depends("depends: [\"a\" {build & = \"2\"}]"), {"a": "2"}),
        expect_equal("build dependencies are named", parse_depend_names("depends: [\"dune\" {>= \"2\"} \"a\" \"t\" {with-test}]"), ["dune", "a"]),
        expect_contains("dune package build passes hub deps", dune_package_build("a", "a", "//d", "//r:d.bzl", ["b"], "hub"), "deps = [\"@hub//:b\"]"),
        expect_contains("dune package build names the library", dune_package_build("a-b", "a_b", "//d", "//r:d.bzl"), "library = \"a_b\""),
        expect_equal("url and sha256 are read", parse_url(_URL_OPAM), struct(src = "https://example.org/a-1.tbz", sha256 = "ab12", integrity = "")),
        expect_equal("src may sit on the next line", parse_url("url {\n  src:\n    \"https://x/a.tbz\"\n  checksum: \"sha256=cd\"\n}\n").src, "https://x/a.tbz"),
        expect_equal("single checksum string", parse_url("url {\n  src: \"https://x/a.tgz\"\n  checksum: \"sha256=cd\"\n}\n").sha256, "cd"),
        expect_equal("sha512 becomes integrity", parse_url("url {\n  src: \"https://x/a.tgz\"\n  checksum: [\"sha512=000102ff\"]\n}\n").integrity, "sha512-AAEC/w=="),
        expect_equal("sha512 pads two bytes", parse_url("url {\n  src: \"https://x/a.tgz\"\n  checksum: \"sha512=0001\"\n}\n").integrity, "sha512-AAE="),
        expect_true("compiler packages come from the toolchain", all([is_toolchain_package(n) for n in ["ocaml", "ocaml-base-compiler", "base-unix", "dune"]])),
        expect_true("libraries do not", not any([is_toolchain_package(n) for n in ["astring", "dune-configurator", "ocaml-compiler-libs"]])),
        expect_equal("package repository name", package_repo_name("deps", "ppx-base.v"), "dx_opam_deps_ppx_base_v"),
        expect_contains("hub aliases a package", packages_hub_build("deps", {"astring": "1"}), "alias(name = \"astring\", actual = \"@dx_opam_deps_astring//:astring\""),
        expect_equal("toolchain fields gain extras", toolchain_fields(struct(a = 1), {"b": 2}), {"a": 1, "b": 2}),
    ]

def ocaml_metadata_tests(name):
    """Declares the OCaml toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
