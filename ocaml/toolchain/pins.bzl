"""Pinned OCaml compiler packages and rules_ocaml identities."""

OCAML_VERSION = "5.5.2"

CONDA_FORGE = "https://conda.anaconda.org/conda-forge/"

RULES_OCAML_COMMIT = "24fa272c7b59ad902c5c82e71aae397b63f05467"

RULES_OCAML_SHA256 = "a23ad90cff101aa9d0cea279462dad72dddf597fc23be7748fbb1bb63143705c"

RULES_OCAML_URL = "https://github.com/obazl/rules_ocaml/archive/" + RULES_OCAML_COMMIT + ".tar.gz"

OCAML_CELLS = {
    "linux_amd64": struct(
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        emitters = ["sys", "vm"],
        packages = [
            struct(
                name = "ocaml",
                url = CONDA_FORGE + "linux-64/ocaml_linux-64-" + OCAML_VERSION + "-h022e319_1.conda",
                sha256 = "6255afce499f06f0f371ab0db54662fa1d3d1f86c28650534a18371d1121836b",
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
                url = CONDA_FORGE + "linux-64/zstd-1.5.7-hb78ec9c_7.conda",
                sha256 = "47d682b9f6d6ec9eb1a6e6c3e75ea6273e899e78fb7fc59f81d39745009fbc60",
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
                url = CONDA_FORGE + "linux-aarch64/ocaml_linux-aarch64-5.5.2-hbe3a449_1.conda",
                sha256 = "baccdbbfdb82f56b034337afe807512e5a35ce41c8a2114d61341df08eca21c6",
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
                url = CONDA_FORGE + "linux-aarch64/zstd-1.5.7-h9d15635_7.conda",
                sha256 = "427fd14bcb3b8659796fecc682716617409350fb5a98e5b7b47558a10d1a2fc7",
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
                url = CONDA_FORGE + "osx-arm64/ocaml_osx-arm64-5.5.2-h9c96050_1.conda",
                sha256 = "dea8d2c0b992daea33d5b73432c3b3eeddfa1ba9dcc1152ffa1e60a9c38d8ae5",
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
                url = CONDA_FORGE + "osx-arm64/zstd-1.5.7-hf451053_7.conda",
                sha256 = "da867f5092eb0cb746d353694f0098031fd9817a4ce7d5743121209ae0f406ca",
                license = "BSD-3-Clause",
                license_file = "info/zstd/licenses/LICENSE",
                binary_placeholders = [],
            ),
        ],
    ),
}
