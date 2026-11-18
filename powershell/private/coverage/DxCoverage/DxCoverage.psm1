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
        [pscustomobject]@{ Path = Get-DxSourcePath (Join-Path $env:RUNFILES_DIR $rlocation); Name = $name }
    }
}

# Since rules_shell 0.10 the launcher's rlocation prefers the runfiles manifest, so scripts run
# from the file a runfiles symlink points to, and Pester only covers a file under that path.
function Get-DxSourcePath {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )
    $item = Get-Item -LiteralPath $Path
    $target = $item.ResolveLinkTarget($false)
    if ($target) {
        return $target.FullName
    }
    return $item.FullName
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
