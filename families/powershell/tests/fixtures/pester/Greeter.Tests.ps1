$ErrorActionPreference = "Stop"

Import-Module (Join-Path $PSScriptRoot "Greeter.psm1") -Force
Import-Module Pester -RequiredVersion 5.7.1
Import-Module DxCoverage

$config = New-DxPesterConfiguration -Path (Join-Path $PSScriptRoot "Greeter.Describe.ps1")
$config.Output.Verbosity = "Detailed"
$result = Invoke-Pester -Configuration $config
Write-DxPesterCoverage -Result $result
if ($result.TotalCount -lt 2 -or $result.FailedCount -gt 0 -or $result.FailedBlocksCount -gt 0) { exit 1 }
exit 0
