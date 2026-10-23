using module Runfiles

param(
    [string]$Pfx,
    [string]$Out
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-Runfile($Runfiles, [string]$Paths, [string]$Suffix) {
    foreach ($path in ($Paths -split " ")) {
        if ($path.EndsWith($Suffix, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $Runfiles.Rlocation($path)
        }
    }
    throw "no $Suffix among $Paths"
}

function Resolve-UserPath([string]$Path) {
    if (-not [System.IO.Path]::IsPathRooted($Path) -and $env:BUILD_WORKING_DIRECTORY) {
        $Path = Join-Path $env:BUILD_WORKING_DIRECTORY $Path
    }
    return [System.IO.Path]::GetFullPath($Path)
}

function Get-FileSha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

if (-not $Pfx) {
    [Console]::Error.WriteLine("dx_msix_sign: pass -Pfx <certificate.pfx>")
    [Environment]::Exit(2)
}
if (-not $Out) {
    [Console]::Error.WriteLine("dx_msix_sign: pass -Out <signed.msix>")
    [Environment]::Exit(2)
}
$certificate = Resolve-UserPath $Pfx
if (-not (Test-Path -LiteralPath $certificate -PathType Leaf)) {
    [Console]::Error.WriteLine("dx_msix_sign: certificate $certificate does not exist")
    [Environment]::Exit(2)
}
$target = Resolve-UserPath $Out

try {
    $runfiles = [Runfiles]::Create()
    $package = Resolve-Runfile $runfiles $env:DX_MSIX_PACKAGE ".msix"
    $signtool = Resolve-Runfile $runfiles $env:DX_MSIX_SIGNTOOL "signtool.exe"
    Copy-Item -LiteralPath $package -Destination $target -Force
    Set-ItemProperty -LiteralPath $target -Name IsReadOnly -Value $false
    $arguments = @("sign", "/fd", "SHA256", "/f", $certificate)
    if ($env:DX_MSIX_PFX_PASSWORD) {
        $arguments += @("/p", $env:DX_MSIX_PFX_PASSWORD)
    }
    $output = & $signtool @arguments $target 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        Remove-Item -LiteralPath $target -Force
        [Console]::Error.WriteLine("dx_msix_sign: signtool failed with exit code $LASTEXITCODE`n$output")
        [Environment]::Exit(1)
    }
} catch {
    [Console]::Error.WriteLine("dx_msix_sign: $($_.Exception.Message)")
    [Environment]::Exit(1)
}
Write-Output "signed=$target"
Write-Output "sha256=$(Get-FileSha256 $target)"
