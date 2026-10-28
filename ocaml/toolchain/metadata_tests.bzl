"""OCaml toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":ocaml.bzl", "COMPILERS", "TOOLS", "native_cc_env", "toolchain_fields")
load(":pins.bzl", "OCAML_CELLS", "OCAML_VERSION", "RULES_OCAML_COMMIT", "RULES_OCAML_SHA256", "RULES_OCAML_URL")
load(":repos.bzl", "DROPPED_PATHS", "EMITTERS", "conda_stem", "placeholder_files", "relative_prefix")

def _checks():
    linux = OCAML_CELLS["linux_amd64"]
    ocaml = [p for p in linux.packages if p.name == "ocaml"][0]
    return [
        expect_equal("ocaml version", OCAML_VERSION, "5.5.2"),
        expect_equal("pinned cells", sorted(OCAML_CELLS.keys()), ["linux_amd64"]),
        expect_equal("linux emitters", linux.emitters, ["sys", "vm"]),
        expect_equal("known emitters", sorted(EMITTERS.keys()), ["sys", "vm"]),
        expect_equal("linux packages", sorted([p.name for p in linux.packages]), ["ocaml", "zstd"]),
        expect_contains("ocaml package names the version", ocaml.url, "ocaml_linux-64-5.5.2-"),
        expect_true("package digests are sha256", all([len(p.sha256) == 64 for p in linux.packages])),
        expect_equal("ocaml license", ocaml.license, "LGPL-2.1-only WITH OCaml-LGPL-linking-exception"),
        expect_equal("rules_ocaml commit", len(RULES_OCAML_COMMIT), 40),
        expect_equal("rules_ocaml digest", len(RULES_OCAML_SHA256), 64),
        expect_contains("rules_ocaml url names the commit", RULES_OCAML_URL, RULES_OCAML_COMMIT),
        expect_true("the upstream module file is dropped", "MODULE.bazel" in DROPPED_PATHS),
        expect_equal("bytecode compiler", TOOLS["compiler"], "bin/ocamlc.opt"),
        expect_equal("native compiler", TOOLS["native_compiler"], "bin/ocamlopt.opt"),
        expect_equal("each emitter has its compiler", COMPILERS, {"sys": "bin/ocamlopt.opt", "vm": "bin/ocamlc.opt"}),
        expect_equal(
            "native C environment names every tool and keeps the dist bin first on PATH",
            native_cc_env("clang", "as-clang", "ld-clang", "llvm-ar", "ext/dist/bin"),
            {
                "CONDA_OCAML_AR": "llvm-ar",
                "CONDA_OCAML_AS": "as-clang -c -Wno-unused-command-line-argument",
                "CONDA_OCAML_CC": "clang -Wno-unused-command-line-argument",
                "CONDA_OCAML_MKDLL": "ld-clang -shared -Wno-unused-command-line-argument",
                "CONDA_OCAML_MKEXE": "ld-clang -Wno-unused-command-line-argument",
                "PATH": "ext/dist/bin:/usr/bin:/bin",
            },
        ),
        expect_equal(
            "native C environment carries the toolchain flags",
            native_cc_env("clang", "clang", "clang", "llvm-ar", "d", ["-Bcrt"], ["-Bcrt"], ["--sysroot=/dev/null"], ["-fPIC"])["CONDA_OCAML_MKEXE"],
            "clang --sysroot=/dev/null -Wno-unused-command-line-argument",
        ),
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
        expect_equal("toolchain fields gain extras", toolchain_fields(struct(a = 1), {"b": 2}), {"a": 1, "b": 2}),
    ]

def ocaml_metadata_tests(name):
    """Declares the OCaml toolchain metadata test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )
