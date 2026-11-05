"""OCaml toolchain pin and acquisition tests."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":ocaml.bzl", "COMPILERS", "TOOLS", "native_cc_env", "toolchain_fields")
load(":opam.bzl", "is_toolchain_package", "parse_depends", "parse_url")
load(":pins.bzl", "OCAML_CELLS", "OCAML_VERSION", "OPAM_REPOSITORY_COMMIT", "OPAM_REPOSITORY_SHA256", "OPAM_REPOSITORY_URL", "RULES_OCAML_COMMIT", "RULES_OCAML_SHA256", "RULES_OCAML_URL")
load(":repos.bzl", "DROPPED_PATHS", "EMITTERS", "conda_stem", "hub_build", "package_repo_name", "packages_hub_build", "placeholder_files", "relative_prefix")

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
    linux = OCAML_CELLS["linux_amd64"]
    ocaml = [p for p in linux.packages if p.name == "ocaml"][0]
    return [
        expect_equal("ocaml version", OCAML_VERSION, "5.5.2"),
        expect_equal("pinned cells", sorted(OCAML_CELLS.keys()), ["linux_amd64", "linux_arm64", "macos_arm64"]),
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
                "CONDA_OCAML_AS": "as-clang -c",
                "CONDA_OCAML_CC": "clang",
                "CONDA_OCAML_MKDLL": "ld-clang -shared",
                "CONDA_OCAML_MKEXE": "ld-clang",
                "PATH": "ext/dist/bin:/usr/bin:/bin",
            },
        ),
        expect_true("hub loads nothing", "load(" not in hub_build()),
        expect_true("hub names no rules_ocaml toolchain type", "toolchain/type" not in hub_build().replace("//ocaml/toolchain/type", "")),
        expect_true("hub names no rules_ocaml constraint", "@rules_ocaml" not in hub_build()),
        expect_contains("hub uses the rules_dx std type", hub_build(), "//ocaml/toolchain/type:std"),
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
        expect_equal("opam snapshot digest", len(OPAM_REPOSITORY_SHA256), 64),
        expect_contains("opam snapshot url names the commit", OPAM_REPOSITORY_URL, OPAM_REPOSITORY_COMMIT),
        expect_equal("locked versions are read", parse_depends(_LOCK), {"astring": "0.8.5", "cmdliner": "1.3.0", "dune": "3.16.0", "ocaml": "5.5.2"}),
        expect_equal("a file without depends pins nothing", parse_depends("opam-version: \"2.0\"\n"), {}),
        expect_equal("an unpinned entry is skipped", parse_depends("depends: [\"a\" {>= \"1\"}]"), {}),
        expect_equal("a version after a build filter is kept", parse_depends("depends: [\"a\" {build & = \"2\"}]"), {"a": "2"}),
        expect_equal("url and sha256 are read", parse_url(_URL_OPAM), struct(src = "https://example.org/a-1.tbz", sha256 = "ab12")),
        expect_equal("single checksum string", parse_url("url {\n  src: \"https://x/a.tgz\"\n  checksum: \"sha256=cd\"\n}\n").sha256, "cd"),
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
