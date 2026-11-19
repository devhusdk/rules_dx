# Finds the runfiles of a Bazel-built PowerShell program, from a runfiles directory or a manifest.

Set-StrictMode -Version Latest

# The context Get-BazelRunfilesContext builds from the environment, made on first use.
$script:DefaultContext = $null

function Get-BazelRunfilesContext {
    <#
    .SYNOPSIS
        Opens runfiles from -Directory, -ManifestFile, or else the environment.
    .DESCRIPTION
        Without arguments it returns the runfiles from the environment, read once: RUNFILES_MANIFEST_FILE
        when RUNFILES_MANIFEST_ONLY=1, then RUNFILES_DIR or TEST_SRCDIR, then RUNFILES_MANIFEST_FILE.
    #>
    [CmdletBinding(DefaultParameterSetName = 'Environment')]
    [OutputType([pscustomobject])]
    param(
        [Parameter(Mandatory, ParameterSetName = 'Directory')]
        [string]$Directory,

        [Parameter(Mandatory, ParameterSetName = 'Manifest')]
        [string]$ManifestFile
    )

    if ($PSCmdlet.ParameterSetName -eq 'Directory') {
        return Initialize-BazelRunfilesContext -Directory $Directory.TrimEnd('/', '\') -Manifest $null -ManifestFile $null
    }
    if ($PSCmdlet.ParameterSetName -eq 'Manifest') {
        return Initialize-BazelRunfilesContext -Directory $null -Manifest (Read-BazelRunfilesManifest $ManifestFile) -ManifestFile $ManifestFile
    }
    if ($null -eq $script:DefaultContext) {
        $script:DefaultContext = Find-BazelRunfilesContext
    }
    return $script:DefaultContext
}

function Find-BazelRunfilesContext {
    $manifest = $env:RUNFILES_MANIFEST_FILE
    if ($manifest -and $env:RUNFILES_MANIFEST_ONLY -eq '1') {
        return Get-BazelRunfilesContext -ManifestFile $manifest
    }
    foreach ($directory in @($env:RUNFILES_DIR, $env:TEST_SRCDIR)) {
        if ($directory -and (Test-Path -LiteralPath $directory -PathType Container)) {
            return Get-BazelRunfilesContext -Directory $directory
        }
    }
    if ($manifest) {
        return Get-BazelRunfilesContext -ManifestFile $manifest
    }
    throw 'cannot find runfiles: set RUNFILES_DIR or RUNFILES_MANIFEST_FILE'
}

function Resolve-BazelRunfile {
    <#
    .SYNOPSIS
        Returns the real path of a runfiles path, or $null when the manifest does not list it.
    .DESCRIPTION
        -Path is `repo/pkg/file`. An apparent repo name maps through the repo mapping of
        -SourceRepository, the canonical name of the calling code's repo ('' for the main repo).
    #>
    [CmdletBinding()]
    [OutputType([string])]
    param(
        [Parameter(Mandatory, Position = 0)]
        [AllowEmptyString()]
        [string]$Path,

        [string]$SourceRepository = '',

        [pscustomobject]$Context
    )

    if ($Path -eq '') {
        throw 'runfiles path is empty'
    }
    if (Test-BazelRunfilesAbsolute $Path) {
        return $Path
    }
    foreach ($segment in $Path.Split('/')) {
        if ($segment -eq '.' -or $segment -eq '..') {
            throw "runfiles path $Path is not normalized"
        }
    }
    if ($null -eq $Context) {
        $Context = Get-BazelRunfilesContext
    }
    $parts = $Path.Split('/', 2)
    $mapped = Get-BazelRunfilesMappedRepository $Context $SourceRepository $parts[0]
    if ($null -ne $mapped) {
        $Path = if ($parts.Count -eq 2) { "$mapped/$($parts[1])" } else { $mapped }
    }
    return Find-BazelRunfilesEntry $Context $Path
}

function Get-BazelRunfilesEnvironment {
    <#
    .SYNOPSIS
        Returns the environment a child Bazel program needs to find the same runfiles.
    #>
    [CmdletBinding()]
    [OutputType([hashtable])]
    param(
        [pscustomobject]$Context
    )

    if ($null -eq $Context) {
        $Context = Get-BazelRunfilesContext
    }
    if ($null -ne $Context.ManifestFile) {
        return @{ RUNFILES_MANIFEST_FILE = $Context.ManifestFile }
    }
    return @{ RUNFILES_DIR = $Context.Directory }
}

function Initialize-BazelRunfilesContext {
    param(
        [AllowNull()][string]$Directory,
        [AllowNull()][hashtable]$Manifest,
        [AllowNull()][string]$ManifestFile
    )

    # [string] turns $null into '', so put the nulls back.
    $state = [pscustomobject]@{
        Directory     = if ($Directory) { $Directory } else { $null }
        Manifest      = $Manifest
        ManifestFile  = if ($ManifestFile) { $ManifestFile } else { $null }
        RepoMapping   = @{}
        PrefixMapping = [System.Collections.Generic.List[string[]]]::new()
    }
    Read-BazelRunfilesRepoMapping $state
    return $state
}

function Read-BazelRunfilesManifest {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "cannot read runfiles manifest $Path"
    }
    $entries = @{}
    foreach ($line in [System.IO.File]::ReadAllLines($Path)) {
        $line = $line.TrimEnd("`r")
        if ($line -eq '') {
            continue
        }
        # A line that starts with a space escapes space, newline and backslash in both fields.
        $escaped = $line.StartsWith(' ')
        if ($escaped) {
            $line = $line.Substring(1)
        }
        $key, $target = $line.Split(' ', 2)
        if ($null -eq $target) {
            $target = ''
        }
        if ($escaped) {
            # \b goes last: every backslash left in a field starts an escape.
            $key = $key.Replace('\s', ' ').Replace('\n', "`n").Replace('\b', '\')
            $target = $target.Replace('\n', "`n").Replace('\b', '\')
        }
        $entries[$key] = $target
    }
    return $entries
}

function Find-BazelRunfilesEntry {
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$Context,

        [Parameter(Mandatory)]
        [string]$Key
    )

    if ($null -eq $Context.Manifest) {
        return "$($Context.Directory)/$Key"
    }
    if ($Context.Manifest.ContainsKey($Key)) {
        $target = $Context.Manifest[$Key]
        if ($target -eq '') {
            return $null
        }
        return $target
    }
    # A key below a listed directory resolves under that directory's real path.
    $prefix = $Key
    while (($cut = $prefix.LastIndexOf('/')) -ge 0) {
        $prefix = $prefix.Substring(0, $cut)
        if ($Context.Manifest.ContainsKey($prefix) -and $Context.Manifest[$prefix] -ne '') {
            return $Context.Manifest[$prefix] + $Key.Substring($cut)
        }
    }
    return $null
}

function Get-BazelRunfilesMappedRepository {
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$Context,

        [AllowEmptyString()]
        [string]$SourceRepository,

        [AllowEmptyString()]
        [string]$Apparent
    )

    $key = "$SourceRepository,$Apparent"
    if ($Context.RepoMapping.ContainsKey($key)) {
        return $Context.RepoMapping[$key]
    }
    foreach ($row in $Context.PrefixMapping) {
        if ($row[1] -eq $Apparent -and $SourceRepository.StartsWith($row[0], [System.StringComparison]::Ordinal)) {
            return $row[2]
        }
    }
    return $null
}

function Read-BazelRunfilesRepoMapping {
    param(
        [Parameter(Mandatory)]
        [pscustomobject]$Context
    )

    $file = Find-BazelRunfilesEntry $Context '_repo_mapping'
    if ($null -eq $file -or -not (Test-Path -LiteralPath $file -PathType Leaf)) {
        return
    }
    foreach ($line in [System.IO.File]::ReadAllLines($file)) {
        $fields = $line.TrimEnd("`r").Split(',')
        if ($fields.Count -ne 3) {
            continue
        }
        # Bazel writes one `prefix*` row for every repo of a module extension that shares a mapping.
        if ($fields[0].EndsWith('*')) {
            $Context.PrefixMapping.Add(@($fields[0].Substring(0, $fields[0].Length - 1), $fields[1], $fields[2]))
        } else {
            $Context.RepoMapping["$($fields[0]),$($fields[1])"] = $fields[2]
        }
    }
}

function Test-BazelRunfilesAbsolute {
    param(
        [Parameter(Mandatory)]
        [string]$Path
    )

    return $Path.StartsWith('/') -or $Path.StartsWith('\') -or $Path -match '^[A-Za-z]:[\\/]'
}

Export-ModuleMember -Function Get-BazelRunfilesContext, Resolve-BazelRunfile, Get-BazelRunfilesEnvironment
