#!/usr/bin/env pwsh
# Solo test (stdlib-only, exit code is the verdict).
$ErrorActionPreference = "Stop"
Import-Module (Join-Path $PSScriptRoot "Pure.psm1") -Force
$value = Get-PureValue
if ($value -ne "pure") {
    Write-Error "unexpected value: $value"
    exit 1
}
exit 0
