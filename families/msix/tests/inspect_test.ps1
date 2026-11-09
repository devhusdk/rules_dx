using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.IO.Compression

$runfiles = [Runfiles]::Create()
$sha = [System.Security.Cryptography.SHA256]::Create()
$failures = [System.Collections.Generic.List[string]]::new()
$machines = @{ 0x8664 = "x64"; 0xaa64 = "arm64" }

function Find-Runfile([string]$Paths, [string]$Suffix) {
    foreach ($path in ($Paths -split " ")) {
        if ($path.EndsWith($Suffix)) {
            return $runfiles.Rlocation($path)
        }
    }
    throw "no $Suffix among $Paths"
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

function Read-Entry($Entry) {
    $stream = $Entry.Open()
    $buffer = [System.IO.MemoryStream]::new()
    try {
        $stream.CopyTo($buffer)
    } finally {
        $stream.Dispose()
    }
    return , $buffer.ToArray()
}

function Get-Sha([byte[]]$Bytes) {
    return [System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}

function Get-Machine([byte[]]$Bytes) {
    $offset = [System.BitConverter]::ToInt32($Bytes, 0x3c)
    $machine = [int][System.BitConverter]::ToUInt16($Bytes, $offset + 4)
    if ($machines.ContainsKey($machine)) {
        return $machines[$machine]
    }
    return ("0x{0:x4}" -f $machine)
}

function Test-Package([string]$Label, [string]$Paths, [string]$Arch) {
    $package = Find-Runfile $Paths ".msix"
    $identity = Get-Content -LiteralPath (Find-Runfile $Paths ".msix.identity.json") -Raw | ConvertFrom-Json -AsHashtable
    Assert-Equal $Arch $identity.arch "$Label identity arch"
    Assert-Equal "$($identity.name)_$($identity.version)_$($Arch)__$($identity.publisher_id)" $identity.package_full_name "$Label package full name"
    Assert-Equal ([System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData([System.IO.File]::ReadAllBytes($package))).ToLowerInvariant()) $identity.package_sha256 "$Label package sha256"

    $zip = [System.IO.Compression.ZipFile]::OpenRead($package)
    try {
        $entries = @{}
        foreach ($entry in $zip.Entries) {
            $entries[$entry.FullName] = Read-Entry $entry
        }
    } finally {
        $zip.Dispose()
    }
    $expected = @($identity.files.Keys) + @("AppxBlockMap.xml", "[Content_Types].xml") | Sort-Object
    Assert-Equal ($expected -join ",") (@($entries.Keys | Sort-Object) -join ",") "$Label entries"
    Assert-That (-not $entries.ContainsKey("AppxSignature.p7x")) "$Label is signed"
    foreach ($name in $identity.files.Keys) {
        if ($entries.ContainsKey($name)) {
            Assert-Equal $identity.files[$name] (Get-Sha $entries[$name]) "$Label $name sha256"
        }
    }
    foreach ($name in $identity.machines.Keys) {
        Assert-Equal $Arch $identity.machines[$name] "$Label identity machine of $name"
        Assert-Equal $Arch (Get-Machine $entries[$name]) "$Label PE machine of $name"
    }
    Assert-Equal "$($identity.executable) greet.dll" (@($identity.machines.Keys) -join " ") "$Label PE files"

    [xml]$manifest = [System.Text.Encoding]::UTF8.GetString($entries["AppxManifest.xml"])
    $ns = [System.Xml.XmlNamespaceManager]::new($manifest.NameTable)
    $ns.AddNamespace("m", "http://schemas.microsoft.com/appx/manifest/foundation/windows10")
    $ns.AddNamespace("b", "http://schemas.microsoft.com/appx/2010/blockmap")
    $node = $manifest.SelectSingleNode("/m:Package/m:Identity", $ns)
    Assert-Equal $identity.name $node.GetAttribute("Name") "$Label manifest name"
    Assert-Equal $identity.publisher $node.GetAttribute("Publisher") "$Label manifest publisher"
    Assert-Equal $identity.version $node.GetAttribute("Version") "$Label manifest version"
    Assert-Equal $Arch $node.GetAttribute("ProcessorArchitecture") "$Label manifest arch"
    $application = $manifest.SelectSingleNode("/m:Package/m:Applications/m:Application", $ns)
    Assert-Equal $identity.executable $application.GetAttribute("Executable") "$Label manifest executable"
    Assert-Equal $identity.application_id $application.GetAttribute("Id") "$Label manifest application id"

    [xml]$blockmap = [System.Text.Encoding]::UTF8.GetString($entries["AppxBlockMap.xml"])
    Assert-Equal "http://www.w3.org/2001/04/xmlenc#sha256" $blockmap.DocumentElement.GetAttribute("HashMethod") "$Label block map hash method"
    $mapped = @()
    foreach ($file in $blockmap.SelectNodes("/b:BlockMap/b:File", $ns)) {
        $name = $file.GetAttribute("Name") -replace "\\", "/"
        $mapped += $name
        $bytes = $entries[$name]
        Assert-Equal $bytes.Length $file.GetAttribute("Size") "$Label block map size of $name"
        $blocks = @($file.SelectNodes("b:Block", $ns))
        Assert-Equal ([Math]::Max(1, [Math]::Ceiling($bytes.Length / 65536))) $blocks.Count "$Label block count of $name"
        for ($i = 0; $i -lt $blocks.Count; $i++) {
            $length = [Math]::Min(65536, $bytes.Length - $i * 65536)
            $hash = $sha.ComputeHash($bytes, $i * 65536, $length)
            Assert-Equal ([System.Convert]::ToBase64String($hash)) $blocks[$i].GetAttribute("Hash") "$Label block $i hash of $name"
        }
    }
    Assert-Equal (@($identity.files.Keys | Sort-Object) -join ",") (@($mapped | Sort-Object) -join ",") "$Label block map files"
    return $entries
}

$x64 = Test-Package "x64" $env:DX_MSIX_X64 "x64"
$arm64 = Test-Package "arm64" $env:DX_MSIX_ARM64 "arm64"
$probe = Test-Package "probe" $env:DX_MSIX_PROBE "x64"

function Test-Contains([byte[]]$Bytes, [string]$Text) {
    return [System.Text.Encoding]::Latin1.GetString($Bytes).Contains($Text)
}
Assert-That (Test-Contains $x64["dxmsix.exe"] "probe=off") "default executable does not print probe=off"
Assert-That (-not (Test-Contains $x64["dxmsix.exe"] "probe=on")) "default executable prints probe=on"
Assert-That (Test-Contains $probe["dxmsix.exe"] "probe=on") "probe executable does not print probe=on"
Assert-That (-not (Test-Contains $probe["dxmsix.exe"] "probe=off")) "probe executable prints probe=off"
Assert-Equal (Get-Sha $x64["share/greeting.txt"]) (Get-Sha $arm64["share/greeting.txt"]) "resource differs between cells"

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "inspect_test: all checks passed"
