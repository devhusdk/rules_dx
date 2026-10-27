"""Fortran toolchain that links Fortran code against flang-rt built by the C++ toolchain."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_fortran//fortran/private:providers.bzl", "FortranToolchainInfo")

MODULE_FLAG_FORMAT = "-J{}"

PREPROCESSOR_FLAG = "-cpp"

def toolchain_fields(compiler, archiver, all_files, runtime_ccinfo):
    """Returns the FortranToolchainInfo fields; the compiler doubles as the unused linker."""
    return {
        "all_files": all_files,
        "archiver": archiver,
        "compiler": compiler,
        "compiler_flags": [],
        "linker": compiler,
        "linker_flags": [],
        "module_flag_format": MODULE_FLAG_FORMAT,
        "preprocessor_flag": PREPROCESSOR_FLAG,
        "preprocessor_flags": [],
        "runtime_ccinfo": runtime_ccinfo,
        "runtime_libraries": [],
        "supports_module_path": True,
    }

def _dx_fortran_toolchain_impl(ctx):
    compiler = ctx.executable.compiler
    archiver = ctx.executable.archiver
    all_files = depset([compiler, archiver], transitive = [ctx.attr.compiler_files[DefaultInfo].files])
    info = FortranToolchainInfo(**toolchain_fields(compiler, archiver, all_files, ctx.attr.runtime[CcInfo]))
    return [platform_common.ToolchainInfo(fortran = info), info]

dx_fortran_toolchain = rule(
    implementation = _dx_fortran_toolchain_impl,
    doc = "Declares Flang as the rules_fortran compiler with flang-rt as the runtime.",
    attrs = {
        "archiver": attr.label(mandatory = True, allow_single_file = True, executable = True, cfg = "exec", doc = "llvm-ar."),
        "compiler": attr.label(mandatory = True, allow_single_file = True, executable = True, cfg = "exec", doc = "The Flang driver."),
        "compiler_files": attr.label(mandatory = True, allow_files = True, cfg = "exec", doc = "Files the Flang driver reads."),
        "runtime": attr.label(mandatory = True, providers = [CcInfo], doc = "The flang-rt cc_library."),
    },
)
