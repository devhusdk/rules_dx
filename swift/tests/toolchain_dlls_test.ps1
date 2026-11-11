using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$runfiles = [Runfiles]::Create()
$swiftc = [System.IO.Path]::GetFullPath($runfiles.Rlocation($env:DX_SWIFTC))
$bin = Split-Path -Parent $swiftc
$root = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $bin))
$runtimeBin = Join-Path $root "runtime\usr\bin"
$system32 = Join-Path $env:SystemRoot "System32"
$objdump = Join-Path $bin "llvm-objdump.exe"
$failures = [System.Collections.Generic.List[string]]::new()

foreach ($path in @($objdump, $runtimeBin, $system32)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "$path is missing" }
}

function Get-Imports([string]$Path) {
    $lines = & $objdump -p $Path
    if ($LASTEXITCODE -ne 0) { throw "llvm-objdump -p $Path exited $LASTEXITCODE" }
    $imports = $false
    foreach ($line in $lines) {
        if ($line -match '^\S') { $imports = $line -match '^The Import Tables:' }
        if ($imports -and $line -match '^\s*DLL Name:\s*(\S+)') { $Matches[1] }
    }
}

function Get-Assemblies([string]$Path) {
    $text = [System.Text.Encoding]::Latin1.GetString([System.IO.File]::ReadAllBytes($Path))
    foreach ($match in [regex]::Matches($text, '<dependentAssembly>\s*<assemblyIdentity\s([^>]*?)/?>')) {
        if ($match.Groups[1].Value -match '\bname\s*=\s*"([^"]*)"') { $Matches[1] }
    }
}

function Resolve-Dll([string]$Dll, [string[]]$Assemblies) {
    $name = [System.IO.Path]::GetFileNameWithoutExtension($Dll)
    if ($Assemblies -contains $name) {
        $private = Join-Path $bin "$name\$name.dll"
        if (Test-Path -LiteralPath $private) { return $private }
    }
    $flat = Join-Path $bin $Dll
    if (Test-Path -LiteralPath $flat) { return $flat }
    if ($Dll -match '^(api|ext)-ms-') { return "" }
    $system = Join-Path $system32 $Dll
    if (Test-Path -LiteralPath $system) { return "" }
    $runtime = Join-Path $runtimeBin $Dll
    if (Test-Path -LiteralPath $runtime) { return $runtime }
    return $null
}

$checked = 0
foreach ($tool in @("swiftc.exe", "swift-frontend.exe", "swift-symbolgraph-extract.exe")) {
    $rootExe = Join-Path $bin $tool
    if (-not (Test-Path -LiteralPath $rootExe)) {
        $failures.Add("FAIL: $tool is missing from $bin")
        continue
    }
    $assemblies = @(Get-Assemblies $rootExe)
    $seen = @{}
    $queue = [System.Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($rootExe)
    while ($queue.Count -gt 0) {
        $consumer = $queue.Dequeue()
        if ($seen.ContainsKey($consumer)) { continue }
        $seen[$consumer] = $true
        $checked++
        foreach ($dll in @(Get-Imports $consumer)) {
            $resolved = Resolve-Dll $dll $assemblies
            if ($null -eq $resolved) {
                $failures.Add("FAIL: $([System.IO.Path]::GetFileName($consumer)) (from $tool) needs $dll")
            } elseif ($resolved -ne "") {
                $queue.Enqueue($resolved)
            }
        }
    }
}

$env:PATH = $runtimeBin
$env:Path = $runtimeBin
$output = (& $swiftc -version 2>&1 | Out-String)
$code = $LASTEXITCODE
if ($code -ne 0) {
    $failures.Add(("FAIL: swiftc.exe -version exited 0x{0:X8}: {1}" -f $code, $output.Trim()))
} elseif ($output -notmatch '6\.4\.2') {
    $failures.Add("FAIL: swiftc.exe -version printed no 6.4.2: $($output.Trim())")
}

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine($_) }
    exit 1
}
Write-Output "checked $checked binaries"
