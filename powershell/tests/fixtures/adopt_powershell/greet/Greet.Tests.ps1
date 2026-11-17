#!/usr/bin/env pwsh
# Adopt-powershell greet test: Pester when vendored, plain assert otherwise.
# Pinned Gallery dep is Pester 5.7.1 via PSGallery.requirements.psd1 plus
# third_party/powershell/PSGallery.lock.json (explicit-path import only,
# no Install-Module). Unpinned runner rejected.
$ErrorActionPreference = "Stop"
Import-Module (Join-Path $PSScriptRoot "Greet.psm1") -Force
if (Get-Module -ListAvailable -Name Pester) {
    Import-Module Pester -MinimumVersion 5.7.1 -ErrorAction Stop
    $result = Invoke-Pester -Path (Join-Path $PSScriptRoot "Greet.Describe.ps1") -PassThru -Output None
    if ($result.FailedCount -gt 0 -or $result.FailedBlocksCount -gt 0) { exit 1 }
    exit 0
}
$greeting = Get-AdoptGreeting
if ($greeting -ne "hello powershell") {
    Write-Error "unexpected greeting: $greeting"
    exit 1
}
exit 0
