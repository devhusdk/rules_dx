using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.IO.Compression.FileSystem

$runfiles = [Runfiles]::Create()
$failures = [System.Collections.Generic.List[string]]::new()
$windowsPowerShell = Join-Path ([Environment]::GetFolderPath("System")) "WindowsPowerShell\v1.0\powershell.exe"
$sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$userProfile = (Get-ItemProperty -LiteralPath "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$sid").ProfileImagePath
$aliases = Join-Path $userProfile "AppData\Local\Microsoft\WindowsApps"

function Find-Runfile([string]$Paths, [string]$Suffix) {
    foreach ($path in ($Paths -split " ")) {
        if ($path.EndsWith($Suffix)) {
            return $runfiles.Rlocation($path)
        }
    }
    throw "no $Suffix among $Paths"
}

function Assert-That([bool]$Condition, [string]$What) {
    if (-not $Condition) {
        $failures.Add($What)
    }
}

function Assert-Equal($Expected, $Actual, [string]$What) {
    if ("$Expected" -cne "$Actual") {
        $failures.Add("${What}: expected '$Expected', got '$Actual'")
    }
}

function Invoke-Process([string]$File, [string[]]$Arguments) {
    $start = [System.Diagnostics.ProcessStartInfo]::new($File)
    foreach ($argument in $Arguments) {
        $start.ArgumentList.Add($argument)
    }
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($start)
    $errors = $process.StandardError.ReadToEndAsync()
    $output = $process.StandardOutput.ReadToEnd()
    $process.WaitForExit()
    return [pscustomobject]@{ Exit = $process.ExitCode; Output = $output; Stderr = $errors.Result }
}

function Invoke-Appx([string]$Command) {
    $result = Invoke-Process $windowsPowerShell @("-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "`$ErrorActionPreference = 'Stop'; $Command")
    if ($result.Exit -ne 0) {
        throw "Windows PowerShell failed ($($result.Exit)) running: $Command`n$($result.Stderr)"
    }
    return $result.Output
}

function Remove-TestPackage([string]$Name) {
    Invoke-Appx "Get-AppxPackage -Name '$Name' | Remove-AppxPackage" | Out-Null
}

function Test-Install([string]$Label, [string]$Paths, [scriptblock]$Check) {
    $package = Find-Runfile $Paths ".msix"
    $identity = Get-Content -LiteralPath (Find-Runfile $Paths ".msix.identity.json") -Raw | ConvertFrom-Json -AsHashtable
    $layout = Join-Path $env:TEST_TMPDIR $Label
    [System.IO.Compression.ZipFile]::ExtractToDirectory($package, $layout)
    Remove-TestPackage $identity.name
    try {
        Invoke-Appx "Add-AppxPackage -Register '$layout\AppxManifest.xml'" | Out-Null
        $installed = Invoke-Appx "Get-AppxPackage -Name '$($identity.name)' | Select-Object PackageFullName, PackageFamilyName, PublisherId, Publisher, Version, Architecture, SignatureKind | ConvertTo-Json" | ConvertFrom-Json
        Assert-Equal $identity.package_full_name $installed.PackageFullName "$Label installed full name"
        Assert-Equal $identity.package_family_name $installed.PackageFamilyName "$Label installed family name"
        Assert-Equal $identity.publisher_id $installed.PublisherId "$Label installed publisher id"
        Assert-Equal $identity.publisher $installed.Publisher "$Label installed publisher"
        Assert-Equal $identity.version $installed.Version "$Label installed version"
        $run = Invoke-Process (Join-Path $aliases $identity.alias) @()
        & $Check $identity $run
    } finally {
        Remove-TestPackage $identity.name
    }
    $left = Invoke-Appx "@(Get-AppxPackage -Name '$($identity.name)').Count"
    Assert-Equal "0" $left.Trim() "$Label is still installed"
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $aliases $identity.alias))) "$Label alias is still present"
}

$developerMode = Get-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\AppModelUnlock" -Name AllowDevelopmentWithoutDevLicense -ErrorAction SilentlyContinue
if ($null -eq $developerMode -or $developerMode.AllowDevelopmentWithoutDevLicense -ne 1) {
    [Console]::Error.WriteLine("install_test: turn on Windows Developer Mode; it registers unsigned packages")
    [Environment]::Exit(1)
}

Test-Install "x64" $env:DX_MSIX_X64 {
    param($Identity, $Run)
    Assert-Equal 0 $Run.Exit "x64 exit ($($Run.Stderr))"
    $expected = "resource=hello from the packaged resource`nlibrary=greeting from greet.dll`npackage=$($Identity.package_full_name)`nprobe=off"
    Assert-Equal $expected ($Run.Output.Trim() -replace "`r", "") "x64 output"
}

Test-Install "probe" $env:DX_MSIX_PROBE {
    param($Identity, $Run)
    Assert-Equal 0 $Run.Exit "probe exit ($($Run.Stderr))"
    Assert-That ($Run.Output.Contains("package=$($Identity.package_full_name)")) "probe output: $($Run.Output)"
    Assert-That ($Run.Output.Contains("probe=on")) "probe output: $($Run.Output)"
}

Test-Install "noresource" $env:DX_MSIX_NORESOURCE {
    param($Identity, $Run)
    Assert-Equal 3 $Run.Exit "missing resource exit"
    Assert-That ($Run.Stderr.Contains("dxmsix: missing resource")) "missing resource stderr: $($Run.Stderr)"
}

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "install_test: all checks passed"
