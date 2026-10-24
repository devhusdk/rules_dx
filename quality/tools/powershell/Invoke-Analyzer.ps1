$ErrorActionPreference = 'Stop'

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
        Import-Module PSScriptAnalyzer -RequiredVersion $version -ErrorAction Stop
        if ($settings) {
            $known = @(Get-ScriptAnalyzerRule | ForEach-Object { $_.RuleName })
            $table = Import-PowerShellDataFile -LiteralPath $settings
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
