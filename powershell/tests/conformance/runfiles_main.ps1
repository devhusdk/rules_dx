param([string]$Rlocation = '')

Import-Module BazelRunfiles

$path = if ($Rlocation) { Resolve-BazelRunfile $Rlocation } else { $null }
if ($null -eq $path -or -not (Test-Path -LiteralPath $path -PathType Leaf)) {
    [Console]::Error.WriteLine("cannot read runfile $Rlocation")
    exit 2
}
[Console]::Out.Write([System.IO.File]::ReadAllText($path))
