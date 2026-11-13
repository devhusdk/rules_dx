"""OCaml toolchain pin and acquisition tests."""

load("//core:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load("//core:toolchain.bzl", "hub_checks")
load("//ocaml/private/toolchain:ocaml.bzl", "COMPILERS", "NATIVE_CC_TOOLS", "TOOLS", "native_cc_env", "toolchain_fields")
load("//ocaml/private/toolchain:opam.bzl", "pick_annotation")
load("//ocaml/private/toolchain:pins.bzl", "OCAML_CELLS", "OCAML_VERSION")
load("//ocaml/private/toolchain:repos.bzl", "DROPPED_PATHS", "EMITTERS", "conda_stem", "dune_package_build", "hub_build", "placeholder_files", "relative_prefix", "softlink_paths")

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
            "lld links executables and shared objects",
            native_cc_env("clang", "as-clang", "ld-clang", "llvm-ar", "bin/tc_cc", lld = True),
            {
                "CONDA_OCAML_AR": "llvm-ar",
                "CONDA_OCAML_AS": "as-clang -c",
                "CONDA_OCAML_CC": "clang",
                "CONDA_OCAML_MKDLL": "ld-clang -shared -fuse-ld=lld",
                "CONDA_OCAML_MKEXE": "ld-clang -fuse-ld=lld",
                "PATH": "bin/tc_cc",
            },
        ),
        expect_equal(
            "macOS links through lld after the plugin flags",
            native_cc_env("clang", "as-clang", "ld-clang", "x/libtool", "bin/tc_cc", macos = True, lld = True),
            {
                "CONDA_OCAML_AR": "x/libtool",
                "CONDA_OCAML_AS": "as-clang -c",
                "CONDA_OCAML_CC": "clang",
                "CONDA_OCAML_MKDLL": "ld-clang -shared -undefined dynamic_lookup -fuse-ld=lld",
                "CONDA_OCAML_MKEXE": "ld-clang -fuse-ld=lld",
                "DX_OCAML_LIBTOOL": "x/libtool",
                "PATH": "bin/tc_cc",
            },
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
        expect_equal(
            "only softlink entries are materialized",
            softlink_paths([
                {"_path": "lib/libzstd.so.1", "path_type": "softlink"},
                {"_path": "lib/libzstd.so.1.5.7", "path_type": "hardlink"},
                {"_path": "info"},
            ]),
            ["lib/libzstd.so.1"],
        ),
        expect_contains("dune package build passes hub deps", dune_package_build("a", "a", "//d", "//r:d.bzl", ["b"]), "deps = [\"//b\"]"),
        expect_contains("dune package build names the library", dune_package_build("a-b", "a_b", "//d", "//r:d.bzl"), "library = \"a_b\""),
        expect_equal("afl-persistent default", [str(pick_annotation("afl-persistent", {}).build_file), pick_annotation("afl-persistent", {}).strip_prefix], [str(Label("//ocaml/private:afl-persistent.BUILD.bazel")), "ocaml-afl-persistent-1.4"]),
        expect_equal("cmdliner default", [str(pick_annotation("cmdliner", {}).build_file), pick_annotation("cmdliner", {}).strip_prefix], [str(Label("//ocaml/private:cmdliner.BUILD.bazel")), ""]),
        expect_equal("crowbar default", [str(pick_annotation("crowbar", {}).build_file), pick_annotation("crowbar", {}).strip_prefix], [str(Label("//ocaml/private:crowbar.BUILD.bazel")), ""]),
        expect_equal("consumer annotation replaces the default", pick_annotation("crowbar", {"crowbar": struct(build_file = "x", library = "", strip_prefix = "")}).build_file, "x"),
        expect_equal("unknown package has no annotation", pick_annotation("astring", {}), None),
        expect_equal("toolchain fields gain extras", toolchain_fields(struct(a = 1), {"b": 2}), {"a": 1, "b": 2}),
    ]

def ocaml_metadata_tests(name, size):
    """Declares the OCaml toolchain metadata test."""
    starlark_test(
        name = name,
        size = size,
        mode = "load",
        checks = _checks(),
    )
