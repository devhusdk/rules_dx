# Bazelisk runs this instead of Bazel on Windows. It runs the dx hook first, as
# tools/bazel does, unless dx itself started this Bazel.
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
$tail = $Matches[2]
# Only a refusal (77) or a stop (130, 143) from the hook stops Bazel. Any other failure, such as a dx too old for
# the hook, an unsupported host or a missing launcher, warns and runs Bazel, so a broken dx never blocks Bazel. The
# hook's stdout goes to stderr, so it never mixes with the output of bazel info or query.
if (-not $env:DX_BAZEL_CHILD) {
    $dx = Join-Path $PSScriptRoot 'dx.cmd'
    $code = 9009
    try {
        if ([IO.File]::Exists($dx)) {
            $hook = New-Object Diagnostics.ProcessStartInfo $dx, "worktree hook bazel -- $tail"
            $hook.UseShellExecute = $false
            $hook.RedirectStandardOutput = $true
            $hook.EnvironmentVariables['DX_BAZEL_CHILD'] = '1'
            $process = [Diagnostics.Process]::Start($hook)
            [Console]::Error.Write($process.StandardOutput.ReadToEnd())
            $process.WaitForExit()
            $code = $process.ExitCode
        }
    } catch {
        [Console]::Error.WriteLine("bazel.ps1: $_")
    }
    if ($code -in 77, 130, 143) {
        exit $code
    }
    if ($code -ne 0) {
        [Console]::Error.WriteLine("bazel.ps1: tools\dx.cmd worktree hook bazel failed (exit $code); running Bazel without it")
    }
}
$bazel = New-Object Diagnostics.ProcessStartInfo $env:BAZEL_REAL, $tail
$bazel.UseShellExecute = $false
$process = [Diagnostics.Process]::Start($bazel)
$process.WaitForExit()
exit $process.ExitCode
