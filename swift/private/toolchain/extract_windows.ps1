param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Out,
    [Parameter(Mandatory = $true)][string]$Work,
    [Parameter(Mandatory = $true)][string]$XCTestVersion,
    [Parameter(Mandatory = $true)][string]$SevenZip,
    [Parameter(Mandatory = $true)][ValidateSet("windows_arm64", "windows_x86_64")][string]$Cell
)

$ErrorActionPreference = "Stop"
[System.Text.Encoding]::RegisterProvider([System.Text.CodePagesEncodingProvider]::Instance)

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

function Expand-Archive7($archive, $destination) {
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    $output = & $SevenZip x -y -bd "-o$destination" $archive 2>&1
    if ($LASTEXITCODE -ne 0) { Fail "7-Zip failed on $archive with exit code ${LASTEXITCODE}:`n$($output -join "`n")" }
}

function Read-Stream($database, $name) {
    $path = Join-Path $database $name
    if (-not (Test-Path -LiteralPath $path)) { return ,[byte[]]@() }
    return ,[System.IO.File]::ReadAllBytes($path)
}

function Read-Strings($database) {
    $pool = Read-Stream $database "!_StringPool"
    $data = Read-Stream $database "!_StringData"
    if ($pool.Length -lt 4) { Fail "$database has no string pool" }
    $codepage = [System.BitConverter]::ToUInt16($pool, 0)
    $wide = ([System.BitConverter]::ToUInt16($pool, 2) -band 0x8000) -ne 0
    $encoding = if ($codepage -eq 0) { [System.Text.Encoding]::Latin1 } else { [System.Text.Encoding]::GetEncoding([int]$codepage) }
    foreach ($highInMarker in @($false, $true)) {
        $strings = New-Object System.Collections.Generic.List[string]
        $strings.Add("")
        $offset = 0
        $whole = $true
        for ($i = 4; $i + 4 -le $pool.Length; $i += 4) {
            [long]$length = [System.BitConverter]::ToUInt16($pool, $i)
            $references = [System.BitConverter]::ToUInt16($pool, $i + 2)
            if ($length -eq 0 -and $references -ne 0) {
                $i += 4
                if ($i + 4 -gt $pool.Length) { $whole = $false; break }
                $low = [System.BitConverter]::ToUInt16($pool, $i)
                $high = if ($highInMarker) { $references } else { [System.BitConverter]::ToUInt16($pool, $i + 2) }
                $length = ([long]$high -shl 16) + $low
            }
            if ($offset + $length -gt $data.Length) { $whole = $false; break }
            $strings.Add($encoding.GetString($data, $offset, $length))
            $offset += $length
        }
        if ($whole -and $offset -eq $data.Length) { return @{ Strings = $strings; Width = $(if ($wide) { 3 } else { 2 }) } }
    }
    Fail "the string pool of $database does not match its string data"
}

function Read-Reference($bytes, $offset, $width) {
    $value = [int][System.BitConverter]::ToUInt16($bytes, $offset)
    if ($width -eq 3) { $value += ([int]$bytes[$offset + 2]) -shl 16 }
    return $value
}

function Read-Schema($database, $pool) {
    $bytes = Read-Stream $database "!_Columns"
    $width = $pool.Width
    $rowSize = 2 * $width + 4
    $rows = [int][Math]::Floor($bytes.Length / $rowSize)
    $numberAt = $rows * $width
    $nameAt = $numberAt + $rows * 2
    $typeAt = $nameAt + $rows * $width
    $schema = @{}
    for ($row = 0; $row -lt $rows; $row++) {
        $table = $pool.Strings[(Read-Reference $bytes ($row * $width) $width)]
        $type = [System.BitConverter]::ToUInt16($bytes, $typeAt + $row * 2) -bxor 0x8000
        $isString = ($type -band 0x800) -ne 0
        $isBinary = ($type -band (-bnot 0x1000) -band 0xffff) -eq 0x900
        $size = if ($isBinary) { 2 } elseif ($isString) { $width } elseif (($type -band 0xff) -le 2) { 2 } else { 4 }
        if (-not $schema.ContainsKey($table)) { $schema[$table] = New-Object System.Collections.Generic.List[object] }
        $schema[$table].Add(@{
            Number = [System.BitConverter]::ToUInt16($bytes, $numberAt + $row * 2) -bxor 0x8000
            Name = $pool.Strings[(Read-Reference $bytes ($nameAt + $row * $width) $width)]
            Text = $isString -and -not $isBinary
            Size = $size
        })
    }
    return $schema
}

function Read-Table($database, $pool, $schema, $table, $wanted) {
    $rows = New-Object System.Collections.Generic.List[object]
    if (-not $schema.ContainsKey($table)) { return ,$rows }
    $columns = @($schema[$table] | Sort-Object { $_.Number })
    $rowSize = 0
    foreach ($column in $columns) { $rowSize += $column.Size }
    $bytes = Read-Stream $database "!$table"
    $count = [int][Math]::Floor($bytes.Length / $rowSize)
    if ($count * $rowSize -ne $bytes.Length) { Fail "table $table of $database is $($bytes.Length) bytes, not a multiple of its $rowSize byte row" }
    for ($row = 0; $row -lt $count; $row++) { $rows.Add(@{}) }
    $at = 0
    foreach ($column in $columns) {
        if ($wanted -contains $column.Name) {
            for ($row = 0; $row -lt $count; $row++) {
                $offset = $at + $row * $column.Size
                $rows[$row][$column.Name] = if ($column.Text) {
                    $pool.Strings[(Read-Reference $bytes $offset $column.Size)]
                } elseif ($column.Size -eq 4) {
                    [long][System.BitConverter]::ToUInt32($bytes, $offset) - 2147483648
                } else {
                    [int][System.BitConverter]::ToUInt16($bytes, $offset) - 32768
                }
            }
        }
        $at += $count * $column.Size
    }
    foreach ($name in $wanted) {
        if ($count -gt 0 -and -not $rows[0].ContainsKey($name)) { Fail "table $table of $database has no column $name" }
    }
    return ,$rows
}

function Get-LongName($value) {
    $name = $value.Split("|")[-1]
    if ($name -eq "" -or $name -eq ".." -or $name.IndexOfAny([char[]]"/\:") -ge 0) { Fail "unsafe name '$value' in an MSI table" }
    return $name
}

function Resolve-Directory($directories, $redirects, $resolved, $key, $depth) {
    if ($resolved.ContainsKey($key)) { return $resolved[$key] }
    if ($depth -gt 64 -or -not $directories.ContainsKey($key)) { Fail "directory $key does not resolve" }
    if ($redirects.ContainsKey($key)) {
        $resolved[$key] = Resolve-Directory $directories $redirects $resolved $redirects[$key] ($depth + 1)
        return $resolved[$key]
    }
    $entry = $directories[$key]
    $parent = $entry.Directory_Parent
    $path = ""
    if ($parent -ne "" -and $parent -ne $key) {
        $path = Resolve-Directory $directories $redirects $resolved $parent ($depth + 1)
        $name = Get-LongName $entry.DefaultDir.Split(":")[-1]
        if ($name -ne ".") { $path = if ($path -eq "") { $name } else { Join-Path $path $name } }
    }
    $resolved[$key] = $path
    return $path
}

function Get-Sha256($path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($path)
    try { [System.BitConverter]::ToString($sha.ComputeHash($stream)) } finally { $stream.Dispose(); $sha.Dispose() }
}

function Get-PeMachine($path) {
    $stream = [System.IO.File]::OpenRead($path)
    try {
        $reader = New-Object System.IO.BinaryReader($stream)
        $stream.Position = 0x3c
        $peOffset = $reader.ReadUInt32()
        $stream.Position = $peOffset + 4
        [int]$reader.ReadUInt16()
    } finally { $stream.Dispose() }
}

function Expand-Msi($msi, $target, $scratch) {
    $database = Join-Path $scratch "database"
    Expand-Archive7 $msi.FullName $database
    $pool = Read-Strings $database
    $schema = Read-Schema $database $pool
    $directories = @{}
    foreach ($row in (Read-Table $database $pool $schema "Directory" @("Directory", "Directory_Parent", "DefaultDir"))) { $directories[$row.Directory] = $row }
    $redirects = @{}
    foreach ($row in (Read-Table $database $pool $schema "CustomAction" @("Type", "Source", "Target"))) {
        if (($row.Type -band 0x3f) -eq 35 -and $row.Target -match '^\[([^\]]+)\]$' -and $directories.ContainsKey($Matches[1])) { $redirects[$row.Source] = $Matches[1] }
    }
    $components = @{}
    foreach ($row in (Read-Table $database $pool $schema "Component" @("Component", "Directory_"))) { $components[$row.Component] = $row.Directory_ }
    $files = Read-Table $database $pool $schema "File" @("File", "Component_", "FileName")
    if ($files.Count -eq 0) { return }
    $payload = Join-Path $scratch "files"
    foreach ($row in (Read-Table $database $pool $schema "Media" @("Cabinet"))) {
        if ($row.Cabinet -eq "") { continue }
        $cabinet = if ($row.Cabinet.StartsWith("#")) { Join-Path $database $row.Cabinet.Substring(1) } else { Join-Path $msi.DirectoryName $row.Cabinet }
        if (-not (Test-Path -LiteralPath $cabinet)) { Fail "cabinet $($row.Cabinet) of $($msi.Name) is missing" }
        Expand-Archive7 $cabinet $payload
        if (-not $row.Cabinet.StartsWith("#")) { Remove-Item -LiteralPath $cabinet -Force }
    }
    $sources = New-Object "System.Collections.Generic.Dictionary[string,string]" ([System.StringComparer]::OrdinalIgnoreCase)
    if (Test-Path -LiteralPath $payload) {
        foreach ($file in [System.IO.Directory]::EnumerateFiles($payload)) { $sources[[System.IO.Path]::GetFileName($file)] = $file }
    }
    $resolved = @{}
    foreach ($row in $files) {
        if (-not $sources.ContainsKey($row.File)) { Fail "file $($row.File) of $($msi.Name) is in none of its cabinets" }
        if (-not $components.ContainsKey($row.Component_)) { Fail "file $($row.File) of $($msi.Name) names the unknown component $($row.Component_)" }
        $directory = Join-Path $target (Resolve-Directory $directories $redirects $resolved $components[$row.Component_] 0)
        $null = [System.IO.Directory]::CreateDirectory($directory)
        $destination = Join-Path $directory (Get-LongName $row.FileName)
        if (Test-Path -LiteralPath $destination) {
            $existing = New-Object System.IO.FileInfo $destination
            $incoming = New-Object System.IO.FileInfo $sources[$row.File]
            if ($existing.Length -eq $incoming.Length -and (Get-Sha256 $existing.FullName) -eq (Get-Sha256 $incoming.FullName)) {
                [System.IO.File]::Delete($incoming.FullName)
                continue
            }
            Fail "conflicting file $destination from $($msi.Name)"
        }
        [System.IO.File]::Move($sources[$row.File], $destination)
    }
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
    if ($containers -lt 2) { Fail "expected a UX and an attached Burn container, found $containers" }
    $uxSize = $reader.ReadUInt32()
    $attachedSizes = @()
    for ($i = 1; $i -lt $containers; $i++) { $attachedSizes += $reader.ReadUInt32() }
} finally {
    $stream.Dispose()
}

$ux = Join-Path $Work "ux"
$payload = Join-Path $Work "payload"
$uxCab = Join-Path $Work "ux.cab"
Copy-Slice $Exe $uxCab $stubSize $uxSize
Expand-Archive7 $uxCab $ux
Remove-Item -LiteralPath $uxCab -Force
[long]$attachedOffset = $signatureOffset + $signatureSize
foreach ($attachedSize in $attachedSizes) {
    $payloadCab = Join-Path $Work "payload.cab"
    Copy-Slice $Exe $payloadCab $attachedOffset $attachedSize
    Expand-Archive7 $payloadCab $payload
    Remove-Item -LiteralPath $payloadCab -Force
    $attachedOffset += $attachedSize
}

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
foreach ($msi in $msis) {
    if ($msi.BaseName -match "android|noasserts|^dbg\.|^ide\.") { continue }
    $index++
    $scratch = Join-Path $Work ("msi{0:D2}" -f $index)
    New-Item -ItemType Directory -Force -Path $trees | Out-Null
    Expand-Msi $msi $trees $scratch
    Remove-Item -LiteralPath $scratch -Recurse -Force
    Remove-Item -LiteralPath $msi.FullName -Force
}

$swiftc = Get-ChildItem -LiteralPath $trees -Recurse -Filter "swiftc.exe" |
    Where-Object { $_.Directory.Name -eq "bin" -and $_.Directory.Parent.Name -eq "usr" } | Select-Object -First 1
if (-not $swiftc) { Fail "no usr\bin\swiftc.exe in the extracted packages" }
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

$toolBin = Join-Path $Out "toolchain\usr\bin"
$runtimeBin = Join-Path $Out "runtime\usr\bin"
$runtimeSxS = Join-Path $runtimeBin "SxS"
$toolDirs = @($toolBin)
$libexec = Join-Path $Out "toolchain\usr\libexec\swift"
if (Test-Path -LiteralPath $libexec) { $toolDirs += @($libexec) + @(Get-ChildItem -LiteralPath $libexec -Recurse -Directory | ForEach-Object { $_.FullName }) }
$toolDirs = @($toolDirs | Where-Object { (Test-Path -LiteralPath $_) -and (Get-ChildItem -LiteralPath $_ -File -Filter "*.exe" | Select-Object -First 1) })
$identities = @{}
foreach ($toolDir in $toolDirs) {
    foreach ($tool in (Get-ChildItem -LiteralPath $toolDir -File -Filter "*.exe")) {
        $text = [System.Text.Encoding]::Latin1.GetString([System.IO.File]::ReadAllBytes($tool.FullName))
        foreach ($match in [regex]::Matches($text, '<dependentAssembly>\s*<assemblyIdentity\s([^>]*?)/?>')) {
            $attributes = @{}
            foreach ($attribute in [regex]::Matches($match.Groups[1].Value, '(\w+)\s*=\s*"([^"]*)"')) { $attributes[$attribute.Groups[1].Value] = $attribute.Groups[2].Value }
            $assembly = $attributes["name"]
            if (-not $assembly -or $assembly.StartsWith("Microsoft.")) { continue }
            if ($tool.Name -eq "swiftc.exe" -or -not $identities.ContainsKey($assembly)) {
                if (-not $identities.ContainsKey($assembly)) { $identities[$assembly] = $attributes }
            }
        }
    }
}
$runtimeDlls = @(Get-ChildItem -LiteralPath $runtimeBin -File -Filter "*.dll")
foreach ($toolDir in $toolDirs) {
    foreach ($dll in $runtimeDlls) {
        $assembly = $dll.BaseName
        if (Test-Path -LiteralPath (Join-Path $toolDir $dll.Name)) { continue }
        $assemblyDir = Join-Path $toolDir $assembly
        New-Item -ItemType Directory -Force -Path $assemblyDir | Out-Null
        Copy-Item -LiteralPath $dll.FullName -Destination (Join-Path $assemblyDir "$assembly.dll")
        $assemblyManifest = Join-Path $runtimeSxS "$assembly.manifest"
        if (Test-Path -LiteralPath $assemblyManifest) {
            Copy-Item -LiteralPath $assemblyManifest -Destination (Join-Path $assemblyDir "$assembly.manifest")
        } elseif ($identities.ContainsKey($assembly)) {
            $identity = $identities[$assembly]
            $attributes = (@("type", "name", "version", "processorArchitecture", "publicKeyToken") | Where-Object { $identity.ContainsKey($_) } | ForEach-Object { "$_=`"$($identity[$_])`"" }) -join " "
            $manifest = "<?xml version=`"1.0`" encoding=`"UTF-8`" standalone=`"yes`"?>`r`n<assembly xmlns=`"urn:schemas-microsoft-com:asm.v1`" manifestVersion=`"1.0`">`r`n  <assemblyIdentity $attributes/>`r`n  <file name=`"$assembly.dll`"/>`r`n</assembly>`r`n"
            [System.IO.File]::WriteAllText((Join-Path $assemblyDir "$assembly.manifest"), $manifest, (New-Object System.Text.UTF8Encoding $false))
        }
    }
}
foreach ($assembly in $identities.Keys) {
    foreach ($toolDir in $toolDirs) {
        $staged = Join-Path $toolDir "$assembly\$assembly.dll"
        if (-not ((Test-Path -LiteralPath $staged) -or (Test-Path -LiteralPath (Join-Path $toolDir "$assembly.dll")))) { Fail "swiftc SxS assembly $assembly is missing from $toolDir" }
    }
}

foreach ($required in @("toolchain\usr\bin\swiftc.exe", "toolchain\usr\bin\llbuildSwift.dll", "toolchain\usr\bin\_InternalSwiftScan.dll", "runtime\usr\bin\swiftCore.dll", "platform\Developer\SDKs\Windows.sdk\usr\lib\swift\windows", "platform\Developer\Library\XCTest-$XCTestVersion\usr")) {
    if (-not (Test-Path -LiteralPath (Join-Path $Out $required))) { Fail "$required is missing from the extracted layout" }
}

$machines = @{ "windows_x86_64" = 0x8664; "windows_arm64" = 0xAA64 }
foreach ($required in @("toolchain\usr\bin\swiftc.exe", "runtime\usr\bin\swiftCore.dll")) {
    $machine = Get-PeMachine (Join-Path $Out $required)
    if ($machine -ne [int]$machines[$Cell]) { Fail ("$required is PE machine 0x{0:X4}, not the one of $Cell" -f $machine) }
}

Remove-Item -LiteralPath $Work -Recurse -Force
