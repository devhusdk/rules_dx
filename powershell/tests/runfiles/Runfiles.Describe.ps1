BeforeAll {
    Import-Module BazelRunfiles -Force

    function New-TestDirectory {
        $path = Join-Path $env:TEST_TMPDIR ([System.Guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $path | Out-Null
        return $path
    }

    function New-TestManifest {
        param([string[]]$Line)
        $path = Join-Path (New-TestDirectory) 'MANIFEST'
        [System.IO.File]::WriteAllText($path, ($Line -join "`n") + "`n")
        return $path
    }
}

Describe "Resolve-BazelRunfile" {
    It "joins a key to the runfiles directory" {
        $dir = New-TestDirectory
        $context = Get-BazelRunfilesContext -Directory "$dir/"
        Resolve-BazelRunfile '_main/pkg/data.txt' -Context $context | Should -Be "$dir/_main/pkg/data.txt"
    }

    It "looks a key up in the manifest" {
        $manifest = New-TestManifest @('_main/pkg/data.txt /real/data.txt')
        $context = Get-BazelRunfilesContext -ManifestFile $manifest
        Resolve-BazelRunfile '_main/pkg/data.txt' -Context $context | Should -Be '/real/data.txt'
    }

    It "returns null for a key the manifest does not list or lists empty" {
        $manifest = New-TestManifest @('_main/pkg/__init__.py ')
        $context = Get-BazelRunfilesContext -ManifestFile $manifest
        Resolve-BazelRunfile '_main/pkg/missing.txt' -Context $context | Should -BeNullOrEmpty
        Resolve-BazelRunfile '_main/pkg/__init__.py' -Context $context | Should -BeNullOrEmpty
    }

    It "unescapes manifest lines that start with a space" {
        $manifest = New-TestManifest @(' _main/a\sb\bc.txt /real/a b\nc\bd.txt')
        $context = Get-BazelRunfilesContext -ManifestFile $manifest
        Resolve-BazelRunfile '_main/a b\c.txt' -Context $context | Should -Be "/real/a b`nc\d.txt"
    }

    It "resolves a key below a directory the manifest lists" {
        $manifest = New-TestManifest @('_main/tree /real/tree')
        $context = Get-BazelRunfilesContext -ManifestFile $manifest
        Resolve-BazelRunfile '_main/tree/sub/file.txt' -Context $context | Should -Be '/real/tree/sub/file.txt'
    }

    It "maps an apparent repo name through the repo mapping" {
        $mapping = Join-Path (New-TestDirectory) '_repo_mapping'
        [System.IO.File]::WriteAllText($mapping, ",my_dep,my_dep+`n+ext+repo*,other,other+`n")
        $manifest = New-TestManifest @("_repo_mapping $mapping", 'my_dep+/file.txt /real/dep.txt', 'other+/file.txt /real/other.txt')
        $context = Get-BazelRunfilesContext -ManifestFile $manifest
        Resolve-BazelRunfile 'my_dep/file.txt' -Context $context | Should -Be '/real/dep.txt'
        Resolve-BazelRunfile 'other/file.txt' -SourceRepository '+ext+repo_a' -Context $context | Should -Be '/real/other.txt'
        Resolve-BazelRunfile 'other/file.txt' -Context $context | Should -BeNullOrEmpty
    }

    It "returns an absolute path unchanged" {
        $context = Get-BazelRunfilesContext -Directory (New-TestDirectory)
        Resolve-BazelRunfile '/abs/file.txt' -Context $context | Should -Be '/abs/file.txt'
        Resolve-BazelRunfile 'C:\abs\file.txt' -Context $context | Should -Be 'C:\abs\file.txt'
    }

    It "refuses empty and unnormalized paths" {
        $context = Get-BazelRunfilesContext -Directory (New-TestDirectory)
        { Resolve-BazelRunfile '' -Context $context } | Should -Throw 'runfiles path is empty'
        { Resolve-BazelRunfile '_main/../x' -Context $context } | Should -Throw '*not normalized'
        { Resolve-BazelRunfile './x' -Context $context } | Should -Throw '*not normalized'
    }

    It "fails on a missing manifest" {
        { Get-BazelRunfilesContext -ManifestFile (Join-Path (New-TestDirectory) 'missing') } | Should -Throw 'cannot read runfiles manifest*'
    }

    It "finds this test's own runfiles from the environment" {
        $path = Resolve-BazelRunfile '_main/powershell/tests/runfiles/Runfiles.Describe.ps1'
        Test-Path -LiteralPath $path -PathType Leaf | Should -BeTrue
    }
}

Describe "Get-BazelRunfilesEnvironment" {
    It "names the manifest of a manifest context" {
        $manifest = New-TestManifest @('_main/a /real/a')
        $environment = Get-BazelRunfilesEnvironment -Context (Get-BazelRunfilesContext -ManifestFile $manifest)
        $environment.Keys | Should -Be @('RUNFILES_MANIFEST_FILE')
        $environment.RUNFILES_MANIFEST_FILE | Should -Be $manifest
    }

    It "names the directory of a directory context" {
        $dir = New-TestDirectory
        $environment = Get-BazelRunfilesEnvironment -Context (Get-BazelRunfilesContext -Directory $dir)
        $environment.Keys | Should -Be @('RUNFILES_DIR')
        $environment.RUNFILES_DIR | Should -Be $dir
    }
}
