# Bazelisk runs this instead of Bazel on Windows, see .bazeliskrc.
# When BAZEL_SH is unset, Bazel runs genrules with the first bash.exe on PATH,
# which is the WSL launcher on hosts with WSL (#3377). This picks Git bash with
# the rule in cli/process/src/bazel_sh.rs; keep the two in sync.

$ErrorActionPreference = 'Stop'

if (-not $env:BAZEL_SH) {
    $dirs = $env:PATH.Split(';') | Where-Object { $_ } | ForEach-Object { $_.Replace('/', '\').TrimEnd('\') }
    $first = $dirs | ForEach-Object { "$_\bash.exe" } | Where-Object { [IO.File]::Exists($_) } | Select-Object -First 1
    $gitBash = $dirs | Where-Object { [IO.File]::Exists("$_\git.exe") } |
        ForEach-Object { "$([IO.Path]::GetDirectoryName($_))\usr\bin\bash.exe" } |
        Where-Object { [IO.File]::Exists($_) } | Select-Object -First 1
    $systemRoot = "$($env:SystemRoot)".TrimEnd('\', '/')
    if ($gitBash -and $gitBash -ne $first) {
        $env:BAZEL_SH = $gitBash
    } elseif (-not $gitBash -and $first -and $systemRoot -and $first.StartsWith("$systemRoot\", 'OrdinalIgnoreCase')) {
        [Console]::Error.WriteLine("bazel.ps1: the first bash.exe on PATH is the WSL launcher ($first) and Git bash was not found; set BAZEL_SH to Git's usr\bin\bash.exe")
    }
}

# Pass the arguments on exactly as Bazelisk quoted them. Re-quoting $args in
# PowerShell 5.1 drops empty arguments and breaks ones that contain quotes.
if ([Environment]::CommandLine -notmatch '(?s) -File +("[^"]*"|\S+) ?(.*)$') {
    throw 'bazel.ps1 expects to be started by Bazelisk with -File'
}
$bazel = New-Object Diagnostics.ProcessStartInfo $env:BAZEL_REAL, $Matches[2]
$bazel.UseShellExecute = $false
$process = [Diagnostics.Process]::Start($bazel)
$process.WaitForExit()
exit $process.ExitCode
