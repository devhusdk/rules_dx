"""Minimal PowerShell wrappers."""

load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo", _pwsh_binary = "pwsh_binary", _pwsh_library = "pwsh_library", _pwsh_test = "pwsh_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_POWERSHELL_LIBRARY_PROVIDES = [
    _PwshInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_POWERSHELL_EXEC_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_POWERSHELL_LIBRARY_SPECS = [("powershell", ["ps1", "psm1", "psd1"])]
_DX_POWERSHELL_LIBRARY_EXTS = [".ps1", ".psm1", ".psd1"]
_DX_POWERSHELL_EXEC_SPECS = [("powershell", ["ps1"])]
_DX_POWERSHELL_EXEC_EXTS = [".ps1"]

_pwsh_library_forward = dx_library_forward_rule(
    provides = _DX_POWERSHELL_LIBRARY_PROVIDES,
    required_providers = [(_PwshInfo, "PwshInfo")],
    quality_specs = _DX_POWERSHELL_LIBRARY_SPECS,
    what = "pwsh_*",
    allow_files = _DX_POWERSHELL_LIBRARY_EXTS,
    upstream_providers = [[_PwshInfo]],
    runtime = "besteffort",
)

_pwsh_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_POWERSHELL_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_POWERSHELL_EXEC_SPECS,
    what = "pwsh_*",
    allow_files = _DX_POWERSHELL_EXEC_EXTS,
    upstream_providers = [[_PwshInfo]],
    optional_providers = [_PwshInfo],
    runtime = "besteffort",
)

_pwsh_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_POWERSHELL_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_POWERSHELL_EXEC_SPECS,
    what = "pwsh_*",
    allow_files = _DX_POWERSHELL_EXEC_EXTS,
    upstream_providers = [[_PwshInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [_PwshInfo],
)

def powershell_effective_srcs(srcs):
    """Returns the effective direct sources for a PowerShell binary or test shape."""
    return srcs if srcs != None else []

def _pwsh_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _pwsh_library, _pwsh_library_forward, srcs, visibility = visibility, **kwargs)

def _pwsh_wrap_binary(name, srcs, visibility = None, **kwargs):
    dx_wrap_binary(name, _pwsh_binary, _pwsh_binary_forward, srcs, visibility = visibility, **kwargs)

def pwsh_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over pwsh_library."""
    _pwsh_wrap_library(name, srcs, visibility = visibility, **kwargs)

def pwsh_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over pwsh_binary."""
    effective_srcs = powershell_effective_srcs(srcs)
    _pwsh_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

def pwsh_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over pwsh_test."""
    dx_wrap_test(name, _pwsh_test, _pwsh_forward_test, srcs, visibility = visibility, **kwargs)
