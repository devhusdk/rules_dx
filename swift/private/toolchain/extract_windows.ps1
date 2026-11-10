param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Out,
    [Parameter(Mandatory = $true)][string]$Work,
    [Parameter(Mandatory = $true)][string]$XCTestVersion
)

$ErrorActionPreference = "Stop"
$system32 = Join-Path $env:SystemRoot "System32"

function Fail($message) {
    [Console]::Error.WriteLine("extract_windows: $message")
    if ($script:trees -and (Test-Path -LiteralPath $script:trees)) {
        Get-ChildItem -LiteralPath $script:trees -Recurse -Directory -Depth 7 | ForEach-Object { [Console]::Error.WriteLine("  $($_.FullName)") }
    }
    exit 1
}

function Copy-Slice($source, $destination, [long]$offset, [long]$length) {
    $in = [System.IO.File]::OpenRead($source)
    $outStream = [System.IO.File]::Create($destination)
    try {
        $null = $in.Seek($offset, [System.IO.SeekOrigin]::Begin)
        $buffer = New-Object byte[] (4MB)
        $left = $length
        while ($left -gt 0) {
            $want = [int][Math]::Min($buffer.Length, $left)
            $read = $in.Read($buffer, 0, $want)
            if ($read -le 0) { Fail "$source ended $left bytes before the end of the slice at $offset" }
            $outStream.Write($buffer, 0, $read)
            $left -= $read
        }
    } finally {
        $in.Dispose()
        $outStream.Dispose()
    }
}

function Expand-Cab($cab, $destination) {
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    & (Join-Path $system32 "expand.exe") "-F:*" $cab $destination | Out-Null
    if ($LASTEXITCODE -ne 0) { Fail "expand.exe failed on $cab with exit code $LASTEXITCODE" }
}

function Find-Dir($root, $filter) {
    Get-ChildItem -LiteralPath $root -Recurse -Directory -Filter $filter | Select-Object -First 1
}

$Exe = (Resolve-Path -LiteralPath $Exe).Path
$Out = [System.IO.Path]::GetFullPath($Out)
$Work = [System.IO.Path]::GetFullPath($Work)
New-Item -ItemType Directory -Force -Path $Work | Out-Null

$stream = [System.IO.File]::OpenRead($Exe)
try {
    $reader = New-Object System.IO.BinaryReader($stream)
    $stream.Position = 0x3c
    $peOffset = $reader.ReadUInt32()
    $stream.Position = $peOffset + 6
    $sectionCount = $reader.ReadUInt16()
    $stream.Position = $peOffset + 20
    $optionalSize = $reader.ReadUInt16()
    $sectionTable = $peOffset + 24 + $optionalSize
    $burn = $null
    for ($i = 0; $i -lt $sectionCount; $i++) {
        $stream.Position = $sectionTable + 40 * $i
        $name = [System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(8)).TrimEnd([char]0)
        if ($name -eq ".wixburn") {
            $stream.Position = $sectionTable + 40 * $i + 20
            $burn = $reader.ReadUInt32()
        }
    }
    if ($null -eq $burn) { Fail "$Exe has no .wixburn section" }
    $stream.Position = $burn
    $magic = $reader.ReadUInt32()
    $version = $reader.ReadUInt32()
    if ($magic -ne 0x00f14300 -or $version -ne 2) { Fail "unsupported Burn header magic $magic version $version" }
    $null = $reader.ReadBytes(16)
    $stubSize = $reader.ReadUInt32()
    $null = $reader.ReadUInt32()
    $signatureOffset = $reader.ReadUInt32()
    $signatureSize = $reader.ReadUInt32()
    $null = $reader.ReadUInt32()
    $containers = $reader.ReadUInt32()
    if ($containers -ne 2) { Fail "expected 2 Burn containers, found $containers" }
    $uxSize = $reader.ReadUInt32()
    $attachedSize = $reader.ReadUInt32()
} finally {
    $stream.Dispose()
}

$uxCab = Join-Path $Work "ux.cab"
$payloadCab = Join-Path $Work "payload.cab"
Copy-Slice $Exe $uxCab $stubSize $uxSize
Copy-Slice $Exe $payloadCab ($signatureOffset + $signatureSize) $attachedSize

$ux = Join-Path $Work "ux"
$payload = Join-Path $Work "payload"
Expand-Cab $uxCab $ux
Expand-Cab $payloadCab $payload
Remove-Item -LiteralPath $uxCab, $payloadCab -Force

$manifest = Join-Path $ux "0"
if (-not (Test-Path -LiteralPath $manifest)) { Fail "the UX container has no BurnManifest.xml (file 0)" }
[xml]$xml = Get-Content -Raw -LiteralPath $manifest
$packages = Join-Path $Work "packages"
New-Item -ItemType Directory -Force -Path $packages | Out-Null
$renamed = 0
foreach ($p in $xml.GetElementsByTagName("Payload")) {
    if ($p.Packaging -ne "embedded" -or $p.Container -eq "WixUXContainer") { continue }
    $source = Join-Path $payload $p.SourcePath
    if (-not (Test-Path -LiteralPath $source)) { Fail "payload $($p.SourcePath) for $($p.FilePath) is missing from the attached container" }
    $target = Join-Path $packages $p.FilePath
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
    Move-Item -LiteralPath $source -Destination $target
    $renamed++
}
if ($renamed -eq 0) { Fail "BurnManifest.xml named no embedded payloads" }

$msis = Get-ChildItem -LiteralPath $packages -Recurse -Filter "*.msi" | Sort-Object FullName
if (-not $msis) { Fail "the bundle holds no MSI packages" }
$trees = Join-Path $Work "trees"
$index = 0
$failed = @()
foreach ($msi in $msis) {
    if ($msi.BaseName -match "android") { continue }
    $index++
    $target = Join-Path $trees ("{0:D2}" -f $index)
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    $log = Join-Path $Work ("msi{0:D2}.log" -f $index)
    $code = $null
    for ($attempt = 0; $attempt -lt 30; $attempt++) {
        $proc = Start-Process -FilePath (Join-Path $system32 "msiexec.exe") -Wait -PassThru -NoNewWindow -ArgumentList @(
            "/a", "`"$($msi.FullName)`"", "/qn", "/L*v", "`"$log`"", "TARGETDIR=`"$target`""
        )
        $code = $proc.ExitCode
        if ($code -ne 1618) { break }
        Start-Sleep -Seconds 10
    }
    if ($code -ne 0 -and $code -ne 3010) {
        if (Test-Path -LiteralPath $log) {
            [Console]::Error.WriteLine("extract_windows: $($msi.Name) log")
            Get-Content -LiteralPath $log | Where-Object { $_ -notmatch "^Property\(" -and $_ -match "Action start|Action ended|Return value|Note: 1:|rror|ail|CustomAction|Cabinet|Media|Source" } |
                Select-Object -First 120 | ForEach-Object { [Console]::Error.WriteLine($_) }
        }
        [Console]::Error.WriteLine("extract_windows: packages")
        Get-ChildItem -LiteralPath $packages -Recurse | ForEach-Object { [Console]::Error.WriteLine("  $($_.FullName) $($_.Length)") }
        $failed += "$($msi.Name) ($code)"
        continue
    }
    Remove-Item -LiteralPath $msi.FullName -Force
    $cab = Join-Path $msi.DirectoryName ($msi.BaseName + "*.cab")
    Remove-Item -Path $cab -Force -ErrorAction SilentlyContinue
}

if ($failed.Count -gt 0) { [Console]::Error.WriteLine("extract_windows: msiexec /a failed for: $($failed -join ', ')") }

$swiftc = Get-ChildItem -LiteralPath $trees -Recurse -Filter "swiftc.exe" |
    Where-Object { $_.Directory.Name -eq "bin" -and $_.Directory.Parent.Name -eq "usr" } | Select-Object -First 1
if (-not $swiftc) { Fail "no usr\bin\swiftc.exe in the extracted packages; msiexec failures: $($failed -join ', ')" }
$dlls = @(Get-ChildItem -LiteralPath $trees -Recurse -Filter "swiftCore.dll" | Where-Object { $_.Directory.Name -eq "bin" })
$runtime = $dlls | Where-Object { $_.FullName -match "Runtimes" } | Select-Object -First 1
if (-not $runtime) { $runtime = $dlls | Select-Object -First 1 }
if (-not $runtime) {
    Get-ChildItem -LiteralPath $trees -Recurse -Filter "swiftCore.dll" | ForEach-Object { [Console]::Error.WriteLine("  found $($_.FullName)") }
    Fail "no bin\swiftCore.dll in the extracted packages"
}
$sdk = Find-Dir $trees "Windows.sdk"
if (-not $sdk) { Fail "no Windows.sdk in the extracted packages" }
$xctest = Find-Dir $trees "XCTest-*"
if (-not $xctest) { Fail "no XCTest-* in the extracted packages" }

$developer = Join-Path $Out "platform\Developer"
New-Item -ItemType Directory -Force -Path (Join-Path $Out "toolchain"), (Join-Path $Out "runtime"), (Join-Path $developer "SDKs"), (Join-Path $developer "Library") | Out-Null
$toolchainUsr = $swiftc.Directory.Parent.FullName
$runtimeUsr = $runtime.Directory.Parent.FullName
Move-Item -LiteralPath $toolchainUsr -Destination (Join-Path $Out "toolchain\usr")
if ($runtimeUsr -eq $toolchainUsr) {
    Copy-Item -LiteralPath (Join-Path $Out "toolchain\usr") -Destination (Join-Path $Out "runtime\usr") -Recurse
} else {
    Move-Item -LiteralPath $runtimeUsr -Destination (Join-Path $Out "runtime\usr")
}
Move-Item -LiteralPath $sdk.FullName -Destination (Join-Path $developer "SDKs\Windows.sdk")
Move-Item -LiteralPath $xctest.FullName -Destination (Join-Path $developer "Library\XCTest-$XCTestVersion")

foreach ($required in @("toolchain\usr\bin\swiftc.exe", "runtime\usr\bin\swiftCore.dll", "platform\Developer\SDKs\Windows.sdk\usr\lib\swift\windows", "platform\Developer\Library\XCTest-$XCTestVersion\usr")) {
    if (-not (Test-Path -LiteralPath (Join-Path $Out $required))) { Fail "$required is missing from the extracted layout" }
}

Remove-Item -LiteralPath $Work -Recurse -Force
