"""Shard probes over every family, one per capability. Generated from every <lang>/lang.json by //core/modules:lang_projections; refresh with `bazel run //core/modules:update_lang_projections`."""

load("//ansible/private/quality:ansible.bzl", _ANSIBLE_CFG = "QUALITY_CFG")
load("//bicep/private/quality:bicep.bzl", _BICEP_CFG = "QUALITY_CFG")
load("//cc/private/quality:cc.bzl", _CC_CFG = "QUALITY_CFG")
load("//cloudformation/private/quality:cloudformation.bzl", _CLOUDFORMATION_CFG = "QUALITY_CFG")
load("//cmake/private/quality:cmake.bzl", _CMAKE_CFG = "QUALITY_CFG")
load("//core/quality:families/core_probe.bzl", _CORE_PROBE_CFG = "QUALITY_CFG")
load("//core/quality:families/generation.bzl", _GENERATION_CFG = "QUALITY_CFG")
load("//core/quality:families/php.bzl", _PHP_CFG = "QUALITY_CFG")
load("//core/quality:real_core.bzl", "real_make_merged_probe_aspect")
load("//d/private/quality:d.bzl", _D_CFG = "QUALITY_CFG")
load("//dart/private/quality:dart.bzl", _DART_CFG = "QUALITY_CFG")
load("//dockerfile/private/quality:dockerfile.bzl", _DOCKERFILE_CFG = "QUALITY_CFG")
load("//dotnet/private/quality:dotnet.bzl", _DOTNET_CFG = "QUALITY_CFG")
load("//erlang/private/quality:beam.bzl", _BEAM_CFG = "QUALITY_CFG")
load("//fortran/private/quality:fortran.bzl", _FORTRAN_CFG = "QUALITY_CFG")
load("//go/private/quality:go.bzl", _GO_CFG = "QUALITY_CFG")
load("//haskell/private/quality:haskell.bzl", _HASKELL_CFG = "QUALITY_CFG")
load("//java/private/quality:jvm.bzl", _JVM_CFG = "QUALITY_CFG")
load("//javascript/private/quality:js.bzl", _JS_CFG = "QUALITY_CFG")
load("//julia/private/quality:julia.bzl", _JULIA_CFG = "QUALITY_CFG")
load("//kotlin/private/quality:kotlin.bzl", _KOTLIN_CFG = "QUALITY_CFG")
load("//kubernetes/private/quality:kubernetes.bzl", _KUBERNETES_CFG = "QUALITY_CFG")
load("//lua/private/quality:lua.bzl", _LUA_CFG = "QUALITY_CFG")
load("//make/private/quality:make.bzl", _MAKE_CFG = "QUALITY_CFG")
load("//ocaml/private/quality:ocaml.bzl", _OCAML_CFG = "QUALITY_CFG")
load("//perl/private/quality:perl.bzl", _PERL_CFG = "QUALITY_CFG")
load("//powershell/private/quality:powershell.bzl", _POWERSHELL_CFG = "QUALITY_CFG")
load("//python/private/quality:python.bzl", _PYTHON_CFG = "QUALITY_CFG")
load("//rego/private/quality:rego.bzl", _REGO_CFG = "QUALITY_CFG")
load("//ruby/private/quality:ruby.bzl", _RUBY_CFG = "QUALITY_CFG")
load("//rust/private/quality:rust.bzl", _RUST_CFG = "QUALITY_CFG")
load("//scala/private/quality:scala.bzl", _SCALA_CFG = "QUALITY_CFG")
load("//sql/private/quality:sql.bzl", _SQL_CFG = "QUALITY_CFG")
load("//swift/private/quality:swift.bzl", _SWIFT_CFG = "QUALITY_CFG")
load("//terraform/private/quality:terraform.bzl", _TERRAFORM_CFG = "QUALITY_CFG")
load("//zig/private/quality:zig.bzl", _ZIG_CFG = "QUALITY_CFG")

real_merged_format_probe_aspect = real_make_merged_probe_aspect(
    [
        _ANSIBLE_CFG,
        _BICEP_CFG,
        _CC_CFG,
        _CMAKE_CFG,
        _CORE_PROBE_CFG,
        _GENERATION_CFG,
        _D_CFG,
        _DART_CFG,
        _DOTNET_CFG,
        _BEAM_CFG,
        _FORTRAN_CFG,
        _GO_CFG,
        _HASKELL_CFG,
        _JVM_CFG,
        _JS_CFG,
        _JULIA_CFG,
        _KOTLIN_CFG,
        _KUBERNETES_CFG,
        _LUA_CFG,
        _MAKE_CFG,
        _OCAML_CFG,
        _PERL_CFG,
        _POWERSHELL_CFG,
        _PYTHON_CFG,
        _REGO_CFG,
        _RUBY_CFG,
        _RUST_CFG,
        _SCALA_CFG,
        _SQL_CFG,
        _SWIFT_CFG,
        _TERRAFORM_CFG,
        _ZIG_CFG,
    ],
    "format",
)

real_merged_lint_probe_aspect = real_make_merged_probe_aspect(
    [
        _ANSIBLE_CFG,
        _BICEP_CFG,
        _CC_CFG,
        _CLOUDFORMATION_CFG,
        _CMAKE_CFG,
        _CORE_PROBE_CFG,
        _GENERATION_CFG,
        _PHP_CFG,
        _D_CFG,
        _DART_CFG,
        _DOCKERFILE_CFG,
        _DOTNET_CFG,
        _BEAM_CFG,
        _FORTRAN_CFG,
        _GO_CFG,
        _HASKELL_CFG,
        _JVM_CFG,
        _JS_CFG,
        _JULIA_CFG,
        _KOTLIN_CFG,
        _KUBERNETES_CFG,
        _LUA_CFG,
        _MAKE_CFG,
        _OCAML_CFG,
        _PERL_CFG,
        _POWERSHELL_CFG,
        _PYTHON_CFG,
        _REGO_CFG,
        _RUBY_CFG,
        _RUST_CFG,
        _SCALA_CFG,
        _SQL_CFG,
        _SWIFT_CFG,
        _TERRAFORM_CFG,
        _ZIG_CFG,
    ],
    "lint",
)

real_merged_typecheck_probe_aspect = real_make_merged_probe_aspect(
    [
        _CC_CFG,
        _CORE_PROBE_CFG,
        _GENERATION_CFG,
        _PHP_CFG,
        _D_CFG,
        _DART_CFG,
        _DOTNET_CFG,
        _BEAM_CFG,
        _FORTRAN_CFG,
        _GO_CFG,
        _HASKELL_CFG,
        _JVM_CFG,
        _JS_CFG,
        _JULIA_CFG,
        _KOTLIN_CFG,
        _LUA_CFG,
        _OCAML_CFG,
        _PYTHON_CFG,
        _REGO_CFG,
        _RUBY_CFG,
        _RUST_CFG,
        _SCALA_CFG,
        _SWIFT_CFG,
        _TERRAFORM_CFG,
        _ZIG_CFG,
    ],
    "typecheck",
)
