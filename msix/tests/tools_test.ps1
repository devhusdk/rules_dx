using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$runfiles = [Runfiles]::Create()
$failures = [System.Collections.Generic.List[string]]::new()
$machines = @{ 0x8664 = "x64"; 0xaa64 = "arm64" }
$closure = @("signtool.exe", "makeappx.exe", "mssign32.dll", "wintrust.dll", "appxsip.dll", "appxpackaging.dll", "opcservices.dll")

function Get-Machine([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        return "not a PE file"
    }
    $offset = [System.BitConverter]::ToInt32($bytes, 0x3c)
    $machine = [int][System.BitConverter]::ToUInt16($bytes, $offset + 4)
    if ($machines.ContainsKey($machine)) {
        return $machines[$machine]
    }
    return ("0x{0:x4}" -f $machine)
}

function Test-Tools([string]$Arch, [string]$Paths) {
    $signtool = @($Paths -split " " | Where-Object { $_.EndsWith("/$Arch/signtool.exe") })
    if ($signtool.Count -ne 1) {
        $failures.Add("$Arch toolchain has no signtool.exe among $Paths")
        return
    }
    $directory = $signtool[0].Substring(0, $signtool[0].Length - "signtool.exe".Length)
    foreach ($name in $closure) {
        $path = $runfiles.Rlocation($directory + $name)
        if ($null -eq $path -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
            $failures.Add("$Arch toolchain runfiles have no $name")
            continue
        }
        $machine = Get-Machine $path
        if ($machine -ne $Arch) {
            $failures.Add("$Arch $name is built for $machine")
        }
    }
}

Test-Tools "x64" $env:DX_MSIX_TOOLS_X64
Test-Tools "arm64" $env:DX_MSIX_TOOLS_ARM64

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "tools_test: all checks passed"
