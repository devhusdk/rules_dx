"""Pinned OCaml compiler packages and rules_ocaml identities; the archive data is in pins.json."""

load("@dx_pins_data//:pins.bzl", "DATA")

_PINS = DATA["dx_ocaml"]

OCAML_VERSION = _PINS["version"]

def _archive(name):
    pin = _PINS["archives"][name]
    return struct(sha256 = pin["sha256"], strip_prefix = pin.get("strip_prefix", ""), urls = pin["urls"])

RULES_OCAML = _archive("rules_ocaml")

OPAM_REPOSITORY = _archive("opam_repository")

OPAM_REPOSITORY_COMMIT = OPAM_REPOSITORY.strip_prefix.removeprefix("opam-repository-")

DUNE = _archive("dune")

OCAML_CELLS = {
    "linux_x86_64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        emitters = ["sys", "vm"],
        packages = [
            struct(
                name = "ocaml",
                archive = _archive("ocaml__linux_x86_64"),
                license = "LGPL-2.1-only WITH OCaml-LGPL-linking-exception",
                license_file = "info/ocaml/licenses/LICENSE",
                binary_placeholders = [
                    "bin/ocamlc.opt",
                    "bin/ocamldep.opt",
                    "bin/ocamldoc.opt",
                    "bin/ocamlobjinfo.opt",
                    "bin/ocamlopt.opt",
                    "bin/x86_64-conda-linux-gnu-ocamlrun-a100",
                    "bin/x86_64-conda-linux-gnu-ocamlrund-a100",
                    "bin/x86_64-conda-linux-gnu-ocamlruni-a100",
                    "lib/ocaml/compiler-libs/ocamlcommon.a",
                    "lib/ocaml/expunge",
                    "lib/ocaml/libcamlrun-x86_64-conda-linux-gnu-a100.so",
                    "lib/ocaml/runtime-launch-info",
                ],
            ),
            struct(
                name = "zstd",
                archive = _archive("zstd__linux_x86_64"),
                license = "BSD-3-Clause",
                license_file = "info/zstd/licenses/LICENSE",
                binary_placeholders = [],
            ),
        ],
    ),
    "linux_arm64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        emitters = ["sys", "vm"],
        packages = [
            struct(
                name = "ocaml",
                archive = _archive("ocaml__linux_arm64"),
                license = "LGPL-2.1-only WITH OCaml-LGPL-linking-exception",
                license_file = "info/ocaml/licenses/LICENSE",
                binary_placeholders = [
                    "bin/aarch64-conda-linux-gnu-ocamlrun-a100",
                    "bin/aarch64-conda-linux-gnu-ocamlrund-a100",
                    "bin/aarch64-conda-linux-gnu-ocamlruni-a100",
                    "bin/ocamlc.opt",
                    "bin/ocamldep.opt",
                    "bin/ocamldoc.opt",
                    "bin/ocamlobjinfo.opt",
                    "bin/ocamlopt.opt",
                    "lib/ocaml/compiler-libs/ocamlcommon.a",
                    "lib/ocaml/expunge",
                    "lib/ocaml/libcamlrun-aarch64-conda-linux-gnu-a100.so",
                    "lib/ocaml/libcamlrun.a",
                    "lib/ocaml/libcamlrun_pic.a",
                    "lib/ocaml/libcamlrund.a",
                    "lib/ocaml/libcamlruni.a",
                    "lib/ocaml/libcomprmarsh.a",
                    "lib/ocaml/runtime-launch-info",
                ],
            ),
            struct(
                name = "zstd",
                archive = _archive("zstd__linux_arm64"),
                license = "BSD-3-Clause",
                license_file = "info/zstd/licenses/LICENSE",
                binary_placeholders = [],
            ),
        ],
    ),
    "macos_arm64": struct(
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        emitters = ["sys", "vm"],
        packages = [
            struct(
                name = "ocaml",
                archive = _archive("ocaml__macos_arm64"),
                license = "LGPL-2.1-only WITH OCaml-LGPL-linking-exception",
                license_file = "info/ocaml/licenses/LICENSE",
                binary_placeholders = [
                    "bin/ocamlc.opt",
                    "bin/ocamldep.opt",
                    "bin/ocamldoc.opt",
                    "bin/ocamlobjinfo.opt",
                    "bin/ocamlopt.opt",
                    "lib/ocaml/compiler-libs/ocamlcommon.a",
                    "lib/ocaml/expunge",
                    "lib/ocaml/libcamlrun.a",
                    "lib/ocaml/libcamlrun_pic.a",
                    "lib/ocaml/libcamlrund.a",
                    "lib/ocaml/libcamlruni.a",
                    "lib/ocaml/libcomprmarsh.a",
                    "lib/ocaml/runtime-launch-info",
                ],
            ),
            struct(
                name = "zstd",
                archive = _archive("zstd__macos_arm64"),
                license = "BSD-3-Clause",
                license_file = "info/zstd/licenses/LICENSE",
                binary_placeholders = [],
            ),
            # Ships only lib/libzstd.a so dune-built executables link zstd statically.
            # Its build hash must follow the zstd pin above.
            struct(
                name = "zstd-static",
                archive = _archive("zstd-static__macos_arm64"),
                license = "BSD-3-Clause",
                license_file = "info/zstd-static/licenses/LICENSE",
                binary_placeholders = [],
            ),
        ],
    ),
}
