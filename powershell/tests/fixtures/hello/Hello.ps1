#!/usr/bin/env pwsh
# Hello entry point for the PowerShell foundation fixture.
Import-Module (Join-Path $PSScriptRoot "Greet.psm1") -Force
Write-Output (Get-Greeting)
