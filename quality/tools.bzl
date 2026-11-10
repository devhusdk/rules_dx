"""All quality tool rows, concatenated from the per-family rows and ordered by id.

support.bzl, adapters.bzl and native_config.bzl derive their tables from TOOLS,
and //quality:update_tools writes the Rust tool list from it.
"""

load("//ansible/private/quality:ansible_tools.bzl", _ANSIBLE = "TOOLS")
load("//quality:families/beam_tools.bzl", _BEAM = "TOOLS")
load("//bicep/private/quality:bicep_tools.bzl", _BICEP = "TOOLS")
load("//quality:families/cc_tools.bzl", _CC = "TOOLS")
load("//cloudformation/private/quality:cloudformation_tools.bzl", _CLOUDFORMATION = "TOOLS")
load("//quality:families/cmake_tools.bzl", _CMAKE = "TOOLS")
load("//quality:families/core_tools.bzl", _CORE = "TOOLS")
load("//quality:families/cue_tools.bzl", _CUE = "TOOLS")
load("//quality:families/d_tools.bzl", _D = "TOOLS")
load("//quality:families/dart_tools.bzl", _DART = "TOOLS")
load("//dockerfile/private/quality:dockerfile_tools.bzl", _DOCKERFILE = "TOOLS")
load("//quality:families/dotnet_tools.bzl", _DOTNET = "TOOLS")
load("//quality:families/fortran_tools.bzl", _FORTRAN = "TOOLS")
load("//quality:families/go_tools.bzl", _GO = "TOOLS")
load("//quality:families/haskell_tools.bzl", _HASKELL = "TOOLS")
load("//quality:families/html_template_tools.bzl", _HTML_TEMPLATE = "TOOLS")
load("//javascript/private/quality:js_tools.bzl", _JS = "TOOLS")
load("//quality:families/jsonnet_tools.bzl", _JSONNET = "TOOLS")
load("//quality:families/julia_tools.bzl", _JULIA = "TOOLS")
load("//quality:families/jvm_tools.bzl", _JVM = "TOOLS")
load("//kubernetes/private/quality:kubernetes_tools.bzl", _KUBERNETES = "TOOLS")
load("//quality:families/latex_tools.bzl", _LATEX = "TOOLS")
load("//quality:families/lua_tools.bzl", _LUA = "TOOLS")
load("//quality:families/make_tools.bzl", _MAKE = "TOOLS")
load("//quality:families/nix_tools.bzl", _NIX = "TOOLS")
load("//quality:families/ocaml_tools.bzl", _OCAML = "TOOLS")
load("//quality:families/perl_tools.bzl", _PERL = "TOOLS")
load("//quality:families/php_tools.bzl", _PHP = "TOOLS")
load("//quality:families/pkl_tools.bzl", _PKL = "TOOLS")
load("//quality:families/powershell_tools.bzl", _POWERSHELL = "TOOLS")
load("//quality:families/python_tools.bzl", _PYTHON = "TOOLS")
load("//rego/private/quality:rego_tools.bzl", _REGO = "TOOLS")
load("//quality:families/ruby_tools.bzl", _RUBY = "TOOLS")
load("//rust/private/quality:rust_tools.bzl", _RUST = "TOOLS")
load("//quality:families/shell_tools.bzl", _SHELL = "TOOLS")
load("//quality:families/sql_tools.bzl", _SQL = "TOOLS")
load("//quality:families/swift_tools.bzl", _SWIFT = "TOOLS")
load("//terraform/private/quality:terraform_tools.bzl", _TERRAFORM = "TOOLS")
load("//quality:families/text_tools.bzl", _TEXT = "TOOLS")
load("//quality:families/typst_tools.bzl", _TYPST = "TOOLS")
load("//quality:families/xml_tools.bzl", _XML = "TOOLS")
load("//quality:families/yaml_tools.bzl", _YAML = "TOOLS")
load("//quality:families/zig_tools.bzl", _ZIG = "TOOLS")

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
