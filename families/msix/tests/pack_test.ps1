using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$runfiles = [Runfiles]::Create()
$failures = [System.Collections.Generic.List[string]]::new()

function Find-Runfile([string]$Paths, [string]$Name) {
    foreach ($path in ($Paths -split " ")) {
        if ($path.EndsWith("/" + $Name)) {
            return $runfiles.Rlocation($path)
        }
    }
    throw "no $Name among $Paths"
}

function Assert-That([bool]$Condition, [string]$What) {
    if (-not $Condition) {
        $failures.Add($What)
    }
}

function Assert-Equal($Expected, $Actual, [string]$What) {
    if ("$Expected" -cne "$Actual") {
        $failures.Add("${What}: expected '$Expected', got '$Actual'")
    }
}

function Get-Sha([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

$pack = $runfiles.Rlocation($env:DX_MSIX_PACK)
$fake = $runfiles.Rlocation($env:DX_MSIX_FAKE_MAKEAPPX)
$exeX64 = Find-Runfile $env:DX_MSIX_PE_X64 "dxmsix.exe"
$dllX64 = Find-Runfile $env:DX_MSIX_PE_X64 "greet.dll"
$exeArm64 = Find-Runfile $env:DX_MSIX_PE_ARM64 "dxmsix.exe"
$dllArm64 = Find-Runfile $env:DX_MSIX_PE_ARM64 "greet.dll"
$greeting = Find-Runfile $env:DX_MSIX_FIXTURES "greeting.txt"
$notice = Find-Runfile $env:DX_MSIX_FIXTURES "NOTICE.txt"
$logo = Find-Runfile $env:DX_MSIX_FIXTURES "logo.png"
$pwsh = (Get-Process -Id $PID).Path
$microsoft = "CN=Microsoft Corporation, O=Microsoft Corporation, L=Redmond, S=Washington, C=US"

function New-Spec([string]$Arch, [string]$Exe, [string]$Dll, [string]$Alias) {
    return [ordered]@{
        alias = $Alias
        application_id = "App"
        arch = $Arch
        description = "Packs <everything> & more"
        display_name = 'Dx & "Pack" <Test>'
        executable = "dxmsix.exe"
        files = @(
            [ordered]@{ dest = "dxmsix.exe"; kind = "exe"; src = $Exe },
            [ordered]@{ dest = "Assets/logo.png"; kind = "logo"; src = $logo },
            [ordered]@{ dest = "greet.dll"; kind = "lib"; src = $Dll },
            [ordered]@{ dest = "share/greeting.txt"; kind = "data"; src = $greeting },
            [ordered]@{ dest = "licenses/NOTICE.txt"; kind = "notice"; src = $notice }
        )
        logo = "Assets/logo.png"
        makeappx = $fake
        makeappx_version = "0.0.0.0"
        max_version_tested = "10.0.26100.0"
        min_version = "10.0.17763.0"
        name = "Dx.Pack"
        publisher = $microsoft
        publisher_display_name = "Microsoft Corporation"
        version = "1.2.3.0"
    }
}

function Invoke-Pack([string]$Name, $Spec, [bool]$Fail = $false) {
    $dir = Join-Path $env:TEST_TMPDIR $Name
    $capture = Join-Path $dir "capture"
    New-Item -ItemType Directory -Path $capture -Force | Out-Null
    $Spec.package = Join-Path $dir "out.msix"
    $Spec.identity = Join-Path $dir "out.msix.identity.json"
    $request = Join-Path $dir "request.json"
    Set-Content -LiteralPath $request -Value ($Spec | ConvertTo-Json -Depth 5) -Encoding utf8NoBOM
    $env:DX_FAKE_CAPTURE = $capture
    $env:DX_FAKE_MAKEAPPX_FAIL = if ($Fail) { "1" } else { $null }
    $start = [System.Diagnostics.ProcessStartInfo]::new($pwsh)
    foreach ($argument in @("-NoLogo", "-NoProfile", "-NonInteractive", "-File", $pack, $request)) {
        $start.ArgumentList.Add($argument)
    }
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($start)
    $errors = $process.StandardError.ReadToEndAsync()
    $output = $process.StandardOutput.ReadToEnd()
    $process.WaitForExit()
    return [pscustomobject]@{
        Capture = $capture
        Exit = $process.ExitCode
        Identity = $Spec.identity
        Output = $output
        Package = $Spec.package
        Stderr = $errors.Result
    }
}

$x64 = New-Spec "x64" $exeX64 $dllX64 "dxmsix.exe"
$result = Invoke-Pack "x64" $x64
Assert-Equal 0 $result.Exit "x64 exit ($($result.Stderr))"
Assert-Equal "" $result.Stderr.Trim() "x64 stderr"
if ($result.Exit -eq 0) {
    $identity = Get-Content -LiteralPath $result.Identity -Raw | ConvertFrom-Json -AsHashtable
    Assert-Equal "8wekyb3d8bbwe" $identity.publisher_id "publisher id"
    Assert-Equal "Dx.Pack_1.2.3.0_x64__8wekyb3d8bbwe" $identity.package_full_name "package full name"
    Assert-Equal "Dx.Pack_8wekyb3d8bbwe" $identity.package_family_name "package family name"
    Assert-Equal $microsoft $identity.publisher "identity publisher"
    Assert-Equal "x64" $identity.arch "identity arch"
    Assert-Equal "False" $identity.signed "identity signed"
    Assert-Equal "dxmsix.exe=x64 greet.dll=x64" (($identity.machines.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join " ") "machines"
    Assert-Equal (Get-Sha $result.Package) $identity.package_sha256 "package sha256"
    Assert-Equal (Get-Sha (Join-Path $result.Capture "AppxManifest.xml")) $identity.files["AppxManifest.xml"] "manifest sha256"
    foreach ($entry in $x64.files) {
        Assert-Equal (Get-Sha $entry.src) $identity.files[$entry.dest] "$($entry.dest) sha256"
    }
    Assert-Equal 6 $identity.files.Count "identity file count"
    Assert-That (-not (Test-Path -LiteralPath "$($result.Package).work")) "work directory is left behind"
    Assert-That (-not (Test-Path -LiteralPath "$($result.Identity).tmp")) "temporary identity is left behind"

    $arguments = @(Get-Content -LiteralPath (Join-Path $result.Capture "args.txt"))
    Assert-Equal "pack /o /h SHA256 /f" ($arguments[0..4] -join " ") "makeappx arguments"
    Assert-Equal "/p $($result.Package)" ($arguments[6..7] -join " ") "makeappx output"
    $mapping = @(Get-Content -LiteralPath (Join-Path $result.Capture "mapping.txt"))
    Assert-Equal "[Files]" $mapping[0] "mapping header"
    Assert-That ($mapping[1].EndsWith('AppxManifest.xml" "AppxManifest.xml"')) "mapping manifest line: $($mapping[1])"
    Assert-That ($mapping -contains ('"{0}" "share\greeting.txt"' -f $greeting)) "mapping has no share\greeting.txt line"
    Assert-That ($mapping -contains ('"{0}" "Assets\logo.png"' -f $logo)) "mapping has no Assets\logo.png line"
    Assert-Equal 7 $mapping.Count "mapping line count"

    [xml]$manifest = Get-Content -LiteralPath (Join-Path $result.Capture "AppxManifest.xml") -Raw
    $ns = [System.Xml.XmlNamespaceManager]::new($manifest.NameTable)
    $ns.AddNamespace("m", "http://schemas.microsoft.com/appx/manifest/foundation/windows10")
    $ns.AddNamespace("uap5", "http://schemas.microsoft.com/appx/manifest/uap/windows10/5")
    $ns.AddNamespace("desktop4", "http://schemas.microsoft.com/appx/manifest/desktop/windows10/4")
    $node = $manifest.SelectSingleNode("/m:Package/m:Identity", $ns)
    Assert-Equal "Dx.Pack" $node.GetAttribute("Name") "manifest name"
    Assert-Equal $microsoft $node.GetAttribute("Publisher") "manifest publisher"
    Assert-Equal "1.2.3.0" $node.GetAttribute("Version") "manifest version"
    Assert-Equal "x64" $node.GetAttribute("ProcessorArchitecture") "manifest arch"
    Assert-Equal $x64.display_name $manifest.SelectSingleNode("/m:Package/m:Properties/m:DisplayName", $ns).InnerText "manifest display name"
    Assert-Equal "Assets\logo.png" $manifest.SelectSingleNode("/m:Package/m:Properties/m:Logo", $ns).InnerText "manifest logo"
    $application = $manifest.SelectSingleNode("/m:Package/m:Applications/m:Application", $ns)
    Assert-Equal "dxmsix.exe" $application.GetAttribute("Executable") "manifest executable"
    Assert-Equal "console" $application.GetAttribute("Subsystem", "http://schemas.microsoft.com/appx/manifest/desktop/windows10/4") "manifest subsystem"
    Assert-Equal "dxmsix.exe" $manifest.SelectSingleNode("//uap5:ExecutionAlias", $ns).GetAttribute("Alias") "manifest alias"
    $device = $manifest.SelectSingleNode("/m:Package/m:Dependencies/m:TargetDeviceFamily", $ns)
    Assert-Equal "10.0.17763.0" $device.GetAttribute("MinVersion") "manifest min version"
}

$arm64 = New-Spec "arm64" $exeArm64 $dllArm64 ""
$result = Invoke-Pack "arm64" $arm64
Assert-Equal 0 $result.Exit "arm64 exit ($($result.Stderr))"
if ($result.Exit -eq 0) {
    $identity = Get-Content -LiteralPath $result.Identity -Raw | ConvertFrom-Json -AsHashtable
    Assert-Equal "Dx.Pack_1.2.3.0_arm64__8wekyb3d8bbwe" $identity.package_full_name "arm64 package full name"
    Assert-Equal "dxmsix.exe=arm64 greet.dll=arm64" (($identity.machines.GetEnumerator() | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join " ") "arm64 machines"
    $text = Get-Content -LiteralPath (Join-Path $result.Capture "AppxManifest.xml") -Raw
    Assert-That ($text.Contains('ProcessorArchitecture="arm64"')) "arm64 manifest arch"
    Assert-That (-not $text.Contains("ExecutionAlias")) "manifest without alias declares one"
    Assert-That (-not $text.Contains("desktop4:Subsystem")) "manifest without alias declares a console subsystem"
}

$result = Invoke-Pack "mismatch" (New-Spec "x64" $exeX64 $dllArm64 "dxmsix.exe")
Assert-Equal 1 $result.Exit "mismatch exit"
Assert-That ($result.Stderr.Contains("dx_msix: greet.dll is built for arm64, but the package targets x64")) "mismatch stderr: $($result.Stderr)"
Assert-That (-not (Test-Path -LiteralPath $result.Identity)) "mismatch wrote an identity"
Assert-That (-not (Test-Path -LiteralPath (Join-Path $result.Capture "args.txt"))) "mismatch ran makeappx"

$result = Invoke-Pack "notpe" (New-Spec "x64" $exeX64 $greeting "dxmsix.exe")
Assert-Equal 1 $result.Exit "non-PE exit"
Assert-That ($result.Stderr.Contains("greeting.txt is not a PE file")) "non-PE stderr: $($result.Stderr)"

$result = Invoke-Pack "makeappx_fails" (New-Spec "x64" $exeX64 $dllX64 "dxmsix.exe") $true
Assert-Equal 1 $result.Exit "makeappx failure exit"
Assert-That ($result.Stderr.Contains("dx_msix: makeappx failed with exit code 3")) "makeappx failure stderr: $($result.Stderr)"
Assert-That ($result.Stderr.Contains("fake failure")) "makeappx output is not reported: $($result.Stderr)"
Assert-That (-not (Test-Path -LiteralPath $result.Identity)) "makeappx failure wrote an identity"
Assert-That (-not (Test-Path -LiteralPath "$($result.Package).work")) "makeappx failure left the work directory"

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "pack_test: all checks passed"
