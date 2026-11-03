"""Pinned rules_powershell archive and PowerShell 7.5.11 builds."""

PWSH_VERSION = "7.5.11"

RULES_POWERSHELL_VERSION = "0.3.1"

RULES_POWERSHELL_SHA256 = "8ef875aa1dad8dc0f8f568547d2283f3d4b686b8e50374d6cdda4c5688fe6b7f"

RULES_POWERSHELL_URL = "https://github.com/periareon/rules_powershell/releases/download/0.3.1/rules_powershell-0.3.1.tar.gz"

PWSH_DOWNLOAD = "https://github.com/PowerShell/PowerShell/releases/download/v" + PWSH_VERSION + "/"

PWSH_CELLS = {
    "linux_arm64": struct(
        artifact = "powershell-7.5.11-linux-arm64.tar.gz",
        sha256 = "830ebda118c731ece3fa7e6b7e8573a21346387cbbca5b2f5e3b9bfe24f96672",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        binary = "pwsh",
    ),
    "linux_x64": struct(
        artifact = "powershell-7.5.11-linux-x64.tar.gz",
        sha256 = "82a8b13d92b0f3ae48e56cf2f3f7961679371736ca90145ca71617c2913ba9d8",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        binary = "pwsh",
    ),
    "osx_arm64": struct(
        artifact = "powershell-7.5.11-osx-arm64.tar.gz",
        sha256 = "93c5d5b71a3937e3dbb68934886d318689f57ecda466443764a9d7101f196e6f",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        binary = "pwsh",
    ),
    "osx_x64": struct(
        artifact = "powershell-7.5.11-osx-x64.tar.gz",
        sha256 = "44828c7173cddb335570a76089f2831b9082997eabafd599b115aaa944f90e69",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:x86_64"],
        binary = "pwsh",
    ),
    "win_arm64": struct(
        artifact = "PowerShell-7.5.11-win-arm64.zip",
        sha256 = "990734d5e116d3709791f261a80925aa8c4f38d51f60656c7e36ae1866a4279c",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        binary = "pwsh.exe",
    ),
    "win_x64": struct(
        artifact = "PowerShell-7.5.11-win-x64.zip",
        sha256 = "75cdab18db9c8ac32f02e82149698166551e42d2a904da4b3a60fb5fcb3ad021",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        binary = "pwsh.exe",
    ),
}
