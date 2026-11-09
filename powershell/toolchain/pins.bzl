"""Pinned rules_powershell archive and PowerShell 7.5.13 builds."""

PWSH_VERSION = "7.5.13"

RULES_POWERSHELL_VERSION = "0.3.1"

RULES_POWERSHELL_SHA256 = "8ef875aa1dad8dc0f8f568547d2283f3d4b686b8e50374d6cdda4c5688fe6b7f"

RULES_POWERSHELL_URL = "https://github.com/periareon/rules_powershell/releases/download/0.3.1/rules_powershell-0.3.1.tar.gz"

PWSH_DOWNLOAD = "https://github.com/PowerShell/PowerShell/releases/download/v" + PWSH_VERSION + "/"

PWSH_CELLS = {
    "linux_arm64": struct(
        artifact = "powershell-7.5.13-linux-arm64.tar.gz",
        sha256 = "6a556353ccfb0a414b80322334eac29a540a398537d3cf8dcc3ecec9cd8abd66",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:aarch64"],
        binary = "pwsh",
    ),
    "linux_x86_64": struct(
        artifact = "powershell-7.5.13-linux-x64.tar.gz",
        sha256 = "e9256bfc6ac98fcbec97270e8a986577a7346a587837cc80d30595eef3c2546b",
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        binary = "pwsh",
    ),
    "macos_arm64": struct(
        artifact = "powershell-7.5.13-osx-arm64.tar.gz",
        sha256 = "0d2f48707d18a388096bee8820967c2ef78156f4eb74700d8a77c50798b99e94",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:aarch64"],
        binary = "pwsh",
    ),
    "macos_x86_64": struct(
        artifact = "powershell-7.5.13-osx-x64.tar.gz",
        sha256 = "57b18193fb4038b115d2b0bfe284c275e471027cde1ad202e777febf2c7da58f",
        exec_compatible_with = ["@platforms//os:macos", "@platforms//cpu:x86_64"],
        binary = "pwsh",
    ),
    "windows_arm64": struct(
        artifact = "PowerShell-7.5.13-win-arm64.zip",
        sha256 = "fc850ea69b8d476a4c1bab1a2321a0b656260efa425681a62c290d1f7054c9e8",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:aarch64"],
        binary = "pwsh.exe",
    ),
    "windows_x86_64": struct(
        artifact = "PowerShell-7.5.13-win-x64.zip",
        sha256 = "120b8f0b04f3b3f6e8c351ddbe568bf596cdca15eeb2ecb7e724060051a36d65",
        exec_compatible_with = ["@platforms//os:windows", "@platforms//cpu:x86_64"],
        binary = "pwsh.exe",
    ),
}
