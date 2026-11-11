#!/usr/bin/env pwsh
# Plain powershell_test for the hello fixture; a throw is the verdict.
$ErrorActionPreference = "Stop"
Import-Module (Join-Path $PSScriptRoot "Greet.psm1") -Force
$greeting = Get-Greeting
if ($greeting -ne "hello powershell") {
    throw "unexpected greeting: $greeting"
}
