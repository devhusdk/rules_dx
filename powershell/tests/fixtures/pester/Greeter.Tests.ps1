$ErrorActionPreference = "Stop"

Import-Module (Join-Path $PSScriptRoot "Greeter.psm1") -Force
Import-Module Pester -RequiredVersion 5.7.1

$config = New-PesterConfiguration
$config.Run.Path = Join-Path $PSScriptRoot "Greeter.Describe.ps1"
$config.Run.PassThru = $true
$config.Output.Verbosity = "Detailed"
$result = Invoke-Pester -Configuration $config
if ($result.TotalCount -lt 2 -or $result.FailedCount -gt 0 -or $result.FailedBlocksCount -gt 0) { exit 1 }
exit 0
