param(
    [Parameter(Mandatory = $true)]
    [string]$Request
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$PeMachines = @{
    0x014c = "x86"
    0x01c4 = "arm"
    0x8664 = "x64"
    0xaa64 = "arm64"
}

function Get-FileSha256([string]$Path) {
    $stream = [System.IO.File]::OpenRead($Path)
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        return ([System.BitConverter]::ToString($sha.ComputeHash($stream)) -replace "-", "").ToLowerInvariant()
    } finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Get-PublisherId([string]$Publisher) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha.ComputeHash([System.Text.Encoding]::Unicode.GetBytes($Publisher))
    } finally {
        $sha.Dispose()
    }
    $alphabet = "0123456789abcdefghjkmnpqrstvwxyz"
    $bits = ""
    foreach ($byte in $hash[0..7]) {
        $bits += [System.Convert]::ToString($byte, 2).PadLeft(8, "0")
    }
    $bits += "0"
    $id = ""
    for ($i = 0; $i -lt 65; $i += 5) {
        $id += $alphabet[[System.Convert]::ToInt32($bits.Substring($i, 5), 2)]
    }
    return $id
}

function Get-PeMachine([string]$Path) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        throw "$Path is not a PE file"
    }
    $offset = [System.BitConverter]::ToInt32($bytes, 0x3c)
    if ($offset -lt 0 -or $offset + 6 -gt $bytes.Length -or $bytes[$offset] -ne 0x50 -or $bytes[$offset + 1] -ne 0x45 -or $bytes[$offset + 2] -ne 0 -or $bytes[$offset + 3] -ne 0) {
        throw "$Path has no PE header"
    }
    $machine = [int][System.BitConverter]::ToUInt16($bytes, $offset + 4)
    if ($PeMachines.ContainsKey($machine)) {
        return $PeMachines[$machine]
    }
    return ("0x{0:x4}" -f $machine)
}

function ConvertTo-Xml([string]$Text) {
    return [System.Security.SecurityElement]::Escape($Text)
}

function New-Manifest($Spec) {
    $name = ConvertTo-Xml $Spec.name
    $publisher = ConvertTo-Xml $Spec.publisher
    $logo = ConvertTo-Xml ($Spec.logo -replace "/", "\")
    $display = ConvertTo-Xml $Spec.display_name
    $publisherDisplay = ConvertTo-Xml $Spec.publisher_display_name
    $description = ConvertTo-Xml $Spec.description
    $executable = ConvertTo-Xml $Spec.executable
    $applicationId = ConvertTo-Xml $Spec.application_id
    $extensions = ""
    if ($Spec.alias) {
        $alias = ConvertTo-Xml $Spec.alias
        $extensions = @"

      <Extensions>
        <uap5:Extension Category="windows.appExecutionAlias">
          <uap5:AppExecutionAlias desktop4:Subsystem="console">
            <uap5:ExecutionAlias Alias="$alias"/>
          </uap5:AppExecutionAlias>
        </uap5:Extension>
      </Extensions>
"@
    }
    $console = ""
    if ($Spec.alias) {
        $console = ' desktop4:Subsystem="console" desktop4:SupportsMultipleInstances="true"'
    }
    return @"
<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10" xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10" xmlns:uap5="http://schemas.microsoft.com/appx/manifest/uap/windows10/5" xmlns:desktop4="http://schemas.microsoft.com/appx/manifest/desktop/windows10/4" xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities" IgnorableNamespaces="uap uap5 desktop4 rescap">
  <Identity Name="$name" Publisher="$publisher" Version="$($Spec.version)" ProcessorArchitecture="$($Spec.arch)"/>
  <Properties>
    <DisplayName>$display</DisplayName>
    <PublisherDisplayName>$publisherDisplay</PublisherDisplayName>
    <Logo>$logo</Logo>
  </Properties>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="$($Spec.min_version)" MaxVersionTested="$($Spec.max_version_tested)"/>
  </Dependencies>
  <Resources>
    <Resource Language="en-us"/>
  </Resources>
  <Applications>
    <Application Id="$applicationId" Executable="$executable" EntryPoint="Windows.FullTrustApplication"$console>
      <uap:VisualElements DisplayName="$display" Description="$description" BackgroundColor="transparent" Square150x150Logo="$logo" Square44x44Logo="$logo"/>$extensions
    </Application>
  </Applications>
  <Capabilities>
    <rescap:Capability Name="runFullTrust"/>
  </Capabilities>
</Package>

"@
}

function Test-Machines($Spec) {
    $machines = [ordered]@{}
    foreach ($entry in $Spec.files) {
        if ($entry.kind -ne "exe" -and $entry.kind -ne "lib") {
            continue
        }
        $machine = Get-PeMachine $entry.src
        if ($machine -ne $Spec.arch) {
            throw "$($entry.dest) is built for $machine, but the package targets $($Spec.arch)"
        }
        $machines[$entry.dest] = $machine
    }
    return $machines
}

function Invoke-Makeappx($Spec, [string]$Work) {
    $manifest = Join-Path $Work "AppxManifest.xml"
    [System.IO.File]::WriteAllText($manifest, (New-Manifest $Spec), (New-Object System.Text.UTF8Encoding($false)))
    $lines = @("[Files]", ('"{0}" "AppxManifest.xml"' -f $manifest))
    foreach ($entry in $Spec.files) {
        $lines += ('"{0}" "{1}"' -f [System.IO.Path]::GetFullPath($entry.src), ($entry.dest -replace "/", "\"))
    }
    $map = Join-Path $Work "mapping.txt"
    [System.IO.File]::WriteAllLines($map, [string[]]$lines, (New-Object System.Text.UTF8Encoding($false)))
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = [System.IO.Path]::GetFullPath($Spec.makeappx)
    foreach ($argument in @("pack", "/o", "/h", "SHA256", "/f", $map, "/p", [System.IO.Path]::GetFullPath($Spec.package))) {
        $start.ArgumentList.Add($argument)
    }
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($start)
    $errors = $process.StandardError.ReadToEndAsync()
    $output = $process.StandardOutput.ReadToEnd() + $errors.Result
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) {
        throw "makeappx failed with exit code $($process.ExitCode)`n$output"
    }
    if (-not (Test-Path -LiteralPath $Spec.package -PathType Leaf)) {
        throw "makeappx wrote no package`n$output"
    }
    return $manifest
}

function Invoke-Pack([string]$RequestPath) {
    $spec = Get-Content -LiteralPath $RequestPath -Raw | ConvertFrom-Json
    $machines = Test-Machines $spec
    $work = [System.IO.Path]::GetFullPath("$($spec.package).work")
    if (Test-Path -LiteralPath $work) {
        Remove-Item -LiteralPath $work -Recurse -Force
    }
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $manifest = Invoke-Makeappx $spec $work
        $files = [ordered]@{ "AppxManifest.xml" = (Get-FileSha256 $manifest) }
        foreach ($entry in ($spec.files | Sort-Object -Property dest)) {
            $files[$entry.dest] = Get-FileSha256 $entry.src
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force
    }
    $publisherId = Get-PublisherId $spec.publisher
    $identity = [ordered]@{
        name = $spec.name
        publisher = $spec.publisher
        publisher_id = $publisherId
        version = $spec.version
        arch = $spec.arch
        package_full_name = "$($spec.name)_$($spec.version)_$($spec.arch)__$publisherId"
        package_family_name = "$($spec.name)_$publisherId"
        application_id = $spec.application_id
        executable = $spec.executable
        alias = $spec.alias
        min_version = $spec.min_version
        max_version_tested = $spec.max_version_tested
        makeappx = $spec.makeappx_version
        signed = $false
        machines = $machines
        package_sha256 = Get-FileSha256 $spec.package
        files = $files
    }
    $temporary = "$($spec.identity).tmp"
    [System.IO.File]::WriteAllText([System.IO.Path]::GetFullPath($temporary), ($identity | ConvertTo-Json -Depth 4) + "`n", (New-Object System.Text.UTF8Encoding($false)))
    Move-Item -LiteralPath $temporary -Destination $spec.identity -Force
}

try {
    Invoke-Pack $Request
} catch {
    [Console]::Error.WriteLine("dx_msix: $($_.Exception.Message)")
    exit 1
}
exit 0
