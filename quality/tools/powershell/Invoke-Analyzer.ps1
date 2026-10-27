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

# The rules Invoke-Formatter runs, in its own order; it skips every rule the settings leave out.
$FormatterRules = @(
    'PSPlaceCloseBrace',
    'PSPlaceOpenBrace',
    'PSUseConsistentWhitespace',
    'PSUseConsistentIndentation',
    'PSAlignAssignmentStatement',
    'PSUseCorrectCasing',
    'PSAvoidUsingCmdletAliases',
    'PSAvoidUsingDoubleQuotesForConstantString',
    'PSAvoidSemicolonsAsLineTerminators',
    'PSAvoidExclaimOperator',
    'PSAvoidTrailingWhitespace'
)

function Assert-FormatterSettings([string]$Settings) {
    if (-not $Settings) {
        return
    }
    $table = Import-PowerShellDataFile -LiteralPath $Settings
    $rules = $table['Rules']
    $named = @()
    if ($rules -is [System.Collections.IDictionary]) {
        $named = @($rules.Keys | Where-Object { $FormatterRules -contains [string]$_ })
    }
    if ($named.Count -eq 0) {
        throw "$Settings configures no formatter rule; add one of $($FormatterRules -join ', ') under Rules with Enable = `$true, or unbind psscriptanalyzer_config to format with the CodeFormatting preset"
    }
}

function Format-One([string]$Path, [string]$Settings, [bool]$Write) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $bom = $bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF
    $offset = if ($bom) { 3 } else { 0 }
    $strict = [System.Text.UTF8Encoding]::new($false, $true)
    try {
        $source = $strict.GetString($bytes, $offset, $bytes.Length - $offset)
    } catch {
        return [ordered]@{ path = $Path; parse_errors = @([ordered]@{ line = 1; column = 1; message = 'file is not valid UTF-8' }) }
    }
    $tokens = $null
    $errors = $null
    [void][System.Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$errors)
    if ($errors.Count -gt 0) {
        return [ordered]@{
            path         = $Path
            parse_errors = @($errors | ForEach-Object {
                    [ordered]@{
                        line    = [int]$_.Extent.StartLineNumber
                        column  = [int]$_.Extent.StartColumnNumber
                        message = [string]$_.Message
                    }
                })
        }
    }
    if ($source.Length -eq 0) {
        return [ordered]@{ path = $Path; formatted = $true }
    }
    $arguments = @{ ScriptDefinition = $source }
    if ($Settings) {
        $arguments.Settings = $Settings
    }
    $formatted = [string](Invoke-Formatter @arguments)
    $crlf = $false
    $first = $source.IndexOf("`n")
    if ($first -gt 0 -and $source[$first - 1] -eq "`r") {
        $crlf = $true
    }
    $formatted = $formatted.Replace("`r`n", "`n")
    if ($crlf) {
        $formatted = $formatted.Replace("`n", "`r`n")
    }
    $same = [string]::Equals($formatted, $source, [System.StringComparison]::Ordinal)
    if ($Write -and -not $same) {
        $body = [System.Text.UTF8Encoding]::new($bom, $true)
        $directory = [System.IO.Path]::GetDirectoryName([System.IO.Path]::GetFullPath($Path))
        $temp = Join-Path $directory ('.' + [System.IO.Path]::GetFileName($Path) + '.' + [guid]::NewGuid().ToString('N') + '.tmp')
        try {
            [System.IO.File]::WriteAllText($temp, $formatted, $body)
            [System.IO.File]::Move($temp, $Path, $true)
        } finally {
            if (Test-Path -LiteralPath $temp) {
                Remove-Item -LiteralPath $temp -Force
            }
        }
    }
    return [ordered]@{ path = $Path; formatted = $same }
}

function Format-Files($Paths, [string]$Settings, [bool]$Write) {
    Assert-FormatterSettings $Settings
    $failed = $false
    $files = foreach ($path in $Paths) {
        try {
            $entry = Format-One -Path $path -Settings $Settings -Write $Write
        } catch {
            $entry = [ordered]@{ path = $path; parse_errors = @([ordered]@{ line = 1; column = 1; message = [string]$_.Exception.Message }) }
        }
        if ($entry.Contains('parse_errors')) {
            $failed = $true
        }
        $entry
    }
    return @{ Failed = $failed; Report = [ordered]@{ analyzer = '1.25.0'; files = @($files) } }
}

function Main {
    $settings = $null
    $mode = 'lint'
    $paths = [System.Collections.Generic.List[string]]::new()
    for ($i = 0; $i -lt $args.Count; $i++) {
        if ($args[$i] -eq '--settings') {
            $i++
            $settings = [string]$args[$i]
        } elseif ($paths.Count -eq 0 -and $args[$i] -eq '--format-check') {
            $mode = 'check'
        } elseif ($paths.Count -eq 0 -and $args[$i] -eq '--format-write') {
            $mode = 'write'
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
        if ($mode -ne 'lint') {
            $report = Format-Files -Paths $paths -Settings $settings -Write ($mode -eq 'write')
            [Console]::Out.Write((ConvertTo-Json -InputObject $report.Report -Depth 6 -Compress))
            [Console]::Out.Flush()
            if ($report.Failed -and $mode -eq 'write') {
                $script:WriteFailed = $true
            }
            return
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
        if ($mode -eq 'write') {
            $script:WriteFailed = $true
        }
    }
}

# A terminating error, not exit, because the rules_powershell wrapper drops a script's exit code.
$script:WriteFailed = $false
Main @args
if ($script:WriteFailed) {
    throw 'psscriptanalyzer format left a file unwritten; see the report'
}
