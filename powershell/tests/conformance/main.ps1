Import-Module (Join-Path $PSScriptRoot "Greet.psm1") -Force
Write-Output (Get-Greeting)
