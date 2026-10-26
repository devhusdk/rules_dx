"""Minimal PowerShell wrappers."""

load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo", _pwsh_binary = "pwsh_binary", _pwsh_library = "pwsh_library", _pwsh_test = "pwsh_test")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_POWERSHELL_LIBRARY_PROVIDES = [
    _PwshInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
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
    provides = DX_EXEC_PROVIDES,
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
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_POWERSHELL_EXEC_SPECS,
    what = "pwsh_*",
    allow_files = _DX_POWERSHELL_EXEC_EXTS,
    upstream_providers = [[_PwshInfo]],
    extra_attrs = dx_lcov_merger_attr(),
    optional_providers = [_PwshInfo],
)

def pwsh_library(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over pwsh_library; srcs are .ps1, .psm1 or .psd1 files."""
    dx_wrap(name, _pwsh_library, _pwsh_library_forward, srcs, visibility = visibility, **kwargs)

def pwsh_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over pwsh_binary; srcs are .ps1 files and optional."""
    dx_wrap_binary(name, _pwsh_binary, _pwsh_binary_forward, srcs, visibility = visibility, **kwargs)

def pwsh_test(name, srcs, visibility = None, **kwargs):
    """Minimal wrapper over pwsh_test; srcs are .ps1 files."""
    dx_wrap_test(name, _pwsh_test, _pwsh_forward_test, srcs, visibility = visibility, **kwargs)
