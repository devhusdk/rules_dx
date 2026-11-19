function New-DxPesterConfiguration {
    param(
        [Parameter(Mandatory)]
        [string[]]$Path
    )
    $config = New-PesterConfiguration
    $config.Run.Path = $Path
    $config.Run.PassThru = $true
    $sources = @(Get-DxCoverageSource)
    if ($env:COVERAGE_OUTPUT_FILE -and $sources.Count -gt 0) {
        $config.CodeCoverage.Enabled = $true
        $config.CodeCoverage.Path = @($sources | ForEach-Object { $_.Path })
    }
    return $config
}

function Get-DxCoverageSource {
    if (-not $env:DX_POWERSHELL_COVERAGE_SOURCES) {
        return
    }
    foreach ($line in Get-Content -LiteralPath (Join-Path $env:RUNFILES_DIR $env:DX_POWERSHELL_COVERAGE_SOURCES)) {
        $rlocation, $name = $line -split "`t", 2
        [pscustomobject]@{ Path = Get-DxRunfilesPath $rlocation; Name = $name }
    }
}

# Pester covers a file only under the path the script runs from, so this resolves an rlocation
# as the rules_shell launcher does: through the runfiles manifest when one exists.
function Get-DxRunfilesPath {
    param(
        [Parameter(Mandatory)]
        [string]$Rlocation
    )
    $manifest = @($env:RUNFILES_MANIFEST_FILE, "$env:RUNFILES_DIR/MANIFEST", "$($env:RUNFILES_DIR)_manifest") |
        Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Leaf) } |
        Select-Object -First 1
    if ($manifest) {
        $prefix = "$Rlocation "
        foreach ($entry in [System.IO.File]::ReadLines($manifest)) {
            if ($entry.StartsWith($prefix, [System.StringComparison]::Ordinal)) {
                return [System.IO.Path]::GetFullPath($entry.Substring($prefix.Length))
            }
        }
    }
    return Join-Path $env:RUNFILES_DIR $Rlocation
}

function Write-DxPesterCoverage {
    param(
        [Parameter(Mandatory)]
        $Result
    )
    if (-not $env:COVERAGE_OUTPUT_FILE -or -not $Result.CodeCoverage) {
        return
    }
    $executed = @($Result.CodeCoverage.CommandsExecuted)
    $missed = @($Result.CodeCoverage.CommandsMissed)
    $out = [System.Collections.Generic.List[string]]::new()
    foreach ($source in Get-DxCoverageSource) {
        $path = $source.Path
        $hits = @{}
        foreach ($command in $missed | Where-Object { $_.File -eq $path }) {
            $hits[[int]$command.Line] = 0
        }
        foreach ($command in $executed | Where-Object { $_.File -eq $path }) {
            $hits[[int]$command.Line] = 1 + [int]$hits[[int]$command.Line]
        }
        $out.Add("SF:$($source.Name)")
        foreach ($line in $hits.Keys | Sort-Object) {
            $out.Add("DA:$line,$($hits[$line])")
        }
        $out.Add("LF:$($hits.Count)")
        $out.Add("LH:$(@($hits.Values | Where-Object { $_ -gt 0 }).Count)")
        $out.Add("end_of_record")
    }
    [System.IO.File]::WriteAllLines($env:COVERAGE_OUTPUT_FILE, $out)
}

Export-ModuleMember -Function New-DxPesterConfiguration, Write-DxPesterCoverage
