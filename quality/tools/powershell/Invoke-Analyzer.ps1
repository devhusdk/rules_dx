$ErrorActionPreference = 'Stop'

function Set-ModuleSearchPath {
    $separator = [System.IO.Path]::PathSeparator
    $analyzer = Get-Module -ListAvailable -Name PSScriptAnalyzer | Where-Object { $_.Version -eq [version]'1.25.0' } | Select-Object -First 1
    if (-not $analyzer) {
        throw 'PSScriptAnalyzer 1.25.0 is not on the declared module path'
    }
    $roots = [System.Collections.Generic.List[string]]::new()
    $roots.Add((Split-Path -Parent (Split-Path -Parent $analyzer.Path)))
    foreach ($root in ([string]$env:DX_PWSH_MODULE_ROOTS).Split('|', [System.StringSplitOptions]::RemoveEmptyEntries)) {
        $roots.Add([System.IO.Path]::GetFullPath($root))
    }
    $roots.Add((Join-Path $PSHOME 'Modules'))
    $env:PSModulePath = $roots -join $separator
    $global:PSModuleAutoLoadingPreference = 'None'
}

function Main {
    $settings = $null
    $paths = [System.Collections.Generic.List[string]]::new()
    for ($i = 0; $i -lt $args.Count; $i++) {
        if ($args[$i] -eq '--settings') {
            $i++
            $settings = [string]$args[$i]
        } else {
            $paths.Add([string]$args[$i])
        }
    }
    try {
        $version = '1.25.0'
        Set-ModuleSearchPath
        Import-Module Microsoft.PowerShell.Utility, Microsoft.PowerShell.Management -ErrorAction Stop
        Import-Module PSScriptAnalyzer -RequiredVersion $version -ErrorAction Stop
        if ($settings) {
            $known = @(Get-ScriptAnalyzerRule | ForEach-Object { $_.RuleName })
            $table = Import-PowerShellDataFile -LiteralPath $settings
            $custom = @($table['CustomRulePath'] | Where-Object { $_ })
            if ($custom.Count -gt 0) {
                $recurse = [bool]$table['RecurseCustomRulePath']
                foreach ($rule in @(Get-ScriptAnalyzerRule -CustomRulePath $custom -RecurseCustomRulePath:$recurse)) {
                    $known += [string]$rule.RuleName
                    $known += [string]$rule.RuleName.Split('\')[-1]
                }
            }
            foreach ($key in 'IncludeRules', 'ExcludeRules') {
                foreach ($name in @($table[$key])) {
                    if ($name -and -not [WildcardPattern]::ContainsWildcardCharacters($name) -and $known -notcontains $name) {
                        throw "$key names the unknown rule $name"
                    }
                }
            }
        }
        $files = foreach ($path in $paths) {
            $arguments = @{ Path = $path }
            if ($settings) {
                $arguments.Settings = $settings
            }
            $found = @(Invoke-ScriptAnalyzer @arguments)
            [ordered]@{
                path        = $path
                diagnostics = @($found | ForEach-Object {
                        [ordered]@{
                            rule     = [string]$_.RuleName
                            severity = $_.Severity.ToString()
                            line     = [int]$_.Line
                            column   = [int]$_.Column
                            message  = [string]$_.Message
                        }
                    })
            }
        }
        $report = [ordered]@{ analyzer = $version; files = @($files) }
        [Console]::Out.Write((ConvertTo-Json -InputObject $report -Depth 6 -Compress))
        [Console]::Out.Flush()
    } catch {
        $failure = [ordered]@{ error = [string]$_.Exception.Message }
        [Console]::Out.Write((ConvertTo-Json -InputObject $failure -Compress))
        [Console]::Out.Flush()
    }
}

Main @args
