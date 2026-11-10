"""Public PowerShell API."""

load(
    "//powershell/private:defs.bzl",
    _powershell_binary = "powershell_binary",
    _powershell_library = "powershell_library",
    _powershell_test = "powershell_test",
)

powershell_binary = _powershell_binary
powershell_library = _powershell_library
powershell_test = _powershell_test
