"""All quality tool rows, concatenated from the per-family rows and ordered by id.

support.bzl, adapters.bzl and native_config.bzl derive their tables from TOOLS,
and //core/quality:update_tools writes the Rust tool list from it.
"""

load("//ansible/private/quality:ansible_tools.bzl", _ANSIBLE = "TOOLS")
load("//core/quality:families/beam_tools.bzl", _BEAM = "TOOLS")
load("//bicep/private/quality:bicep_tools.bzl", _BICEP = "TOOLS")
load("//core/quality:families/cc_tools.bzl", _CC = "TOOLS")
load("//cloudformation/private/quality:cloudformation_tools.bzl", _CLOUDFORMATION = "TOOLS")
load("//cmake/private/quality:cmake_tools.bzl", _CMAKE = "TOOLS")
load("//core/quality:families/core_tools.bzl", _CORE = "TOOLS")
load("//core/quality:families/cue_tools.bzl", _CUE = "TOOLS")
load("//d/private/quality:d_tools.bzl", _D = "TOOLS")
load("//dart/private/quality:dart_tools.bzl", _DART = "TOOLS")
load("//dockerfile/private/quality:dockerfile_tools.bzl", _DOCKERFILE = "TOOLS")
load("//core/quality:families/dotnet_tools.bzl", _DOTNET = "TOOLS")
load("//fortran/private/quality:fortran_tools.bzl", _FORTRAN = "TOOLS")
load("//go/private/quality:go_tools.bzl", _GO = "TOOLS")
load("//haskell/private/quality:haskell_tools.bzl", _HASKELL = "TOOLS")
load("//core/quality:families/html_template_tools.bzl", _HTML_TEMPLATE = "TOOLS")
load("//javascript/private/quality:js_tools.bzl", _JS = "TOOLS")
load("//core/quality:families/jsonnet_tools.bzl", _JSONNET = "TOOLS")
load("//julia/private/quality:julia_tools.bzl", _JULIA = "TOOLS")
load("//core/quality:families/jvm_tools.bzl", _JVM = "TOOLS")
load("//kubernetes/private/quality:kubernetes_tools.bzl", _KUBERNETES = "TOOLS")
load("//core/quality:families/latex_tools.bzl", _LATEX = "TOOLS")
load("//core/quality:families/lua_tools.bzl", _LUA = "TOOLS")
load("//make/private/quality:make_tools.bzl", _MAKE = "TOOLS")
load("//core/quality:families/nix_tools.bzl", _NIX = "TOOLS")
load("//core/quality:families/oci_image_tools.bzl", _OCI_IMAGE = "TOOLS")
load("//core/quality:families/ocaml_tools.bzl", _OCAML = "TOOLS")
load("//core/quality:families/perl_tools.bzl", _PERL = "TOOLS")
load("//core/quality:families/php_tools.bzl", _PHP = "TOOLS")
load("//core/quality:families/pkl_tools.bzl", _PKL = "TOOLS")
load("//core/quality:families/powershell_tools.bzl", _POWERSHELL = "TOOLS")
load("//core/quality:families/python_tools.bzl", _PYTHON = "TOOLS")
load("//rego/private/quality:rego_tools.bzl", _REGO = "TOOLS")
load("//core/quality:families/ruby_tools.bzl", _RUBY = "TOOLS")
load("//rust/private/quality:rust_tools.bzl", _RUST = "TOOLS")
load("//core/quality:families/shell_tools.bzl", _SHELL = "TOOLS")
load("//core/quality:families/sql_tools.bzl", _SQL = "TOOLS")
load("//core/quality:families/swift_tools.bzl", _SWIFT = "TOOLS")
load("//terraform/private/quality:terraform_tools.bzl", _TERRAFORM = "TOOLS")
load("//core/quality:families/text_tools.bzl", _TEXT = "TOOLS")
load("//core/quality:families/typst_tools.bzl", _TYPST = "TOOLS")
load("//core/quality:families/xml_tools.bzl", _XML = "TOOLS")
load("//core/quality:families/yaml_tools.bzl", _YAML = "TOOLS")
load("//core/quality:families/zig_tools.bzl", _ZIG = "TOOLS")

FAMILY_TOOLS = {
    "ansible": _ANSIBLE,
    "beam": _BEAM,
    "bicep": _BICEP,
    "cc": _CC,
    "cloudformation": _CLOUDFORMATION,
    "cmake": _CMAKE,
    "core": _CORE,
    "cue": _CUE,
    "d": _D,
    "dart": _DART,
    "dockerfile": _DOCKERFILE,
    "dotnet": _DOTNET,
    "fortran": _FORTRAN,
    "go": _GO,
    "haskell": _HASKELL,
    "html_template": _HTML_TEMPLATE,
    "js": _JS,
    "jsonnet": _JSONNET,
    "julia": _JULIA,
    "jvm": _JVM,
    "kubernetes": _KUBERNETES,
    "latex": _LATEX,
    "lua": _LUA,
    "make": _MAKE,
    "nix": _NIX,
    "oci_image": _OCI_IMAGE,
    "ocaml": _OCAML,
    "perl": _PERL,
    "php": _PHP,
    "pkl": _PKL,
    "powershell": _POWERSHELL,
    "python": _PYTHON,
    "rego": _REGO,
    "ruby": _RUBY,
    "rust": _RUST,
    "shell": _SHELL,
    "sql": _SQL,
    "swift": _SWIFT,
    "terraform": _TERRAFORM,
    "text": _TEXT,
    "typst": _TYPST,
    "xml": _XML,
    "yaml": _YAML,
    "zig": _ZIG,
}

TOOLS = sorted([t for rows in FAMILY_TOOLS.values() for t in rows], key = lambda t: t.id)
