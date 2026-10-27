BeforeAll {
    $script:Script = Join-Path $PSScriptRoot "extract.ps1"
    $script:Fixture = Join-Path $PSScriptRoot "fixture"
    $script:Pwsh = [Environment]::ProcessPath
    $script:Work = Join-Path ([IO.Path]::GetTempPath()) ("extract_script_" + [Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Path $script:Work | Out-Null

    function Invoke-Extract([string[]]$Rels, [string]$Manifest = "", [string]$Name = "out") {
        $tsv = Join-Path $script:Work "$Name.tsv"
        $lines = foreach ($rel in $Rels) { (Join-Path $script:Fixture $rel) + "`t" + "fixture/" + $rel }
        [IO.File]::WriteAllText($tsv, (($lines -join "`n") + "`n"))
        $out = Join-Path $script:Work "$Name.json"
        $err = Join-Path $script:Work "$Name.err"
        $arguments = @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $script:Script, "-Sources", $tsv, "-Out", $out)
        if ($Manifest) {
            $arguments += @("-Manifest", (Join-Path $script:Fixture $Manifest), "-ManifestRel", "fixture/$Manifest")
        }
        $process = Start-Process -FilePath $script:Pwsh -ArgumentList $arguments -NoNewWindow -Wait -PassThru -RedirectStandardError $err -RedirectStandardOutput (Join-Path $script:Work "$Name.stdout")
        [pscustomobject]@{
            ExitCode = $process.ExitCode
            Out      = $out
            Stderr   = (Get-Content -Raw -Path $err)
        }
    }

    $script:LedgerFiles = @(
        "Ledger/Ledger.psm1",
        "Ledger/Private/Format-LedgerAmount.ps1",
        "Ledger/Public/Add-LedgerEntry.ps1",
        "Ledger/Public/Get-LedgerBalance.ps1",
        "Ledger/Public/Remove-LedgerEntry.ps1",
        "Ledger/Public/Test-LedgerBalance.ps1"
    )
}

AfterAll {
    Remove-Item -Recurse -Force -Path $script:Work
}

Describe "extract.ps1" {
    It "parses the Ledger module without importing it" {
        $env:TEST_TMPDIR = $script:Work
        $run = Invoke-Extract $script:LedgerFiles "Ledger/Ledger.psd1" "ledger"
        $run.ExitCode | Should -Be 0
        Test-Path (Join-Path $script:Work "imported") | Should -BeFalse
        $api = Get-Content -Raw -Path $run.Out | ConvertFrom-Json
        $api.schema | Should -Be "dx.powershell.api/1"
        $api.pwsh | Should -Match '^\d+\.\d+\.\d+$'
        $api.manifest.module_version | Should -Be "1.4.0"
        @($api.files.functions.name) | Should -Contain "Add-LedgerEntry"
    }

    It "writes byte-identical JSON on every run" {
        $first = Invoke-Extract $script:LedgerFiles "Ledger/Ledger.psd1" "first"
        $second = Invoke-Extract $script:LedgerFiles "Ledger/Ledger.psd1" "second"
        $one = [IO.File]::ReadAllBytes($first.Out)
        $two = [IO.File]::ReadAllBytes($second.Out)
        [Convert]::ToBase64String($one) | Should -Be ([Convert]::ToBase64String($two))
        $one[-1] | Should -Be 10
        $one[0] | Should -Be ([byte][char]'{')
    }

    It "strips the scope qualifier from function names" {
        $run = Invoke-Extract @("Scoped/Scoped.psm1") "" "scoped"
        $run.ExitCode | Should -Be 0
        $functions = (Get-Content -Raw -Path $run.Out | ConvertFrom-Json).files[0].functions
        @($functions.name) | Should -Be @("Get-ScopedValue", "Get-ScopedSecret", "Set-ScopedGlobal")
        @($functions.scope) | Should -Be @("script", "private", "global")
    }

    It "reports a parse error and writes nothing" {
        $run = Invoke-Extract @("bad/ParseError.psm1") "" "parse"
        $run.ExitCode | Should -Be 1
        $run.Stderr.Trim() | Should -Be "docs_powershell: fixture/bad/ParseError.psm1:2: Missing ')' in function parameter list."
        Test-Path $run.Out | Should -BeFalse
    }

    It "records a dynamic export as dynamic" {
        $run = Invoke-Extract @("bad/DynamicExport.psm1") "" "dynamic"
        $run.ExitCode | Should -Be 0
        (Get-Content -Raw -Path $run.Out | ConvertFrom-Json).files[0].exports.kind | Should -Be "dynamic"
    }

    It "records literal exports as written" {
        $run = Invoke-Extract @("bad/UndefinedExport.psm1") "" "undefined"
        $exports = (Get-Content -Raw -Path $run.Out | ConvertFrom-Json).files[0].exports
        $exports.kind | Should -Be "literal"
        @($exports.functions) | Should -Be @("Get-Thing", "Get-Missing")
    }

    It "records an empty export list" {
        $run = Invoke-Extract @("bad/EmptyExport.psm1") "" "empty"
        $exports = (Get-Content -Raw -Path $run.Out | ConvertFrom-Json).files[0].exports
        $exports.kind | Should -Be "literal"
        @($exports.functions).Count | Should -Be 0
    }

    It "records both case variants of a function" {
        $run = Invoke-Extract @("bad/CaseDuplicate.psm1") "" "case"
        @((Get-Content -Raw -Path $run.Out | ConvertFrom-Json).files[0].functions.name) | Should -Be @("Get-Thing", "Get-THING")
    }

    It "reads a binary RootModule without loading it" {
        $run = Invoke-Extract @("bad/Binary.psm1") "bad/Binary.psd1" "binary"
        $run.ExitCode | Should -Be 0
        (Get-Content -Raw -Path $run.Out | ConvertFrom-Json).manifest.root_module | Should -Be "Binary.dll"
    }

    It "tolerates a using module statement whose module is absent" {
        $run = Invoke-Extract @("bad/UsingModule.psm1") "" "using"
        $run.ExitCode | Should -Be 0
        @((Get-Content -Raw -Path $run.Out | ConvertFrom-Json).files[0].unresolved_uses) | Should -Be @("using module NotInstalledAnywhere")
    }

    It "never imports, dot-sources or invokes code" {
        $text = Get-Content -Raw -Path $script:Script
        $text | Should -Not -Match 'Import-Module|Import-PowerShellDataFile|Invoke-Expression|(?m)^\s*\.\s|&\s'
    }
}
