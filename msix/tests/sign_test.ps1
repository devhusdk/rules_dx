using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.IO.Compression

$runfiles = [Runfiles]::Create()
$failures = [System.Collections.Generic.List[string]]::new()
$codeSigning = "1.3.6.1.5.5.7.3.3"

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

function Get-Sha([byte[]]$Bytes) {
    return [System.Convert]::ToHexString([System.Security.Cryptography.SHA256]::HashData($Bytes)).ToLowerInvariant()
}

function New-TestCertificate([string]$Subject, [string]$Path, [string]$Password, [string]$Usage = $codeSigning, [int]$FromDays = -1, [int]$ToDays = 1) {
    $rsa = [System.Security.Cryptography.RSA]::Create(2048)
    $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
        $Subject, $rsa, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $usages = [System.Security.Cryptography.OidCollection]::new()
    $usages.Add([System.Security.Cryptography.Oid]::new($Usage)) | Out-Null
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usages, $false))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
        [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature, $true))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
    $now = [System.DateTimeOffset]::UtcNow
    $certificate = $request.CreateSelfSigned($now.AddDays($FromDays), $now.AddDays($ToDays))
    [System.IO.File]::WriteAllBytes($Path, $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $Password))
    return $certificate
}

function Invoke-Sign([string[]]$Arguments, [string]$Package = $env:DX_MSIX_PACKAGE, [hashtable]$Environment = @{}) {
    $start = [System.Diagnostics.ProcessStartInfo]::new($sign)
    $start.Environment["DX_MSIX_PACKAGE"] = $Package
    foreach ($name in $Environment.Keys) {
        $start.Environment[$name] = $Environment[$name]
    }
    foreach ($argument in $Arguments) {
        $start.ArgumentList.Add($argument)
    }
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($start)
    $errors = $process.StandardError.ReadToEndAsync()
    $output = $process.StandardOutput.ReadToEnd()
    $process.WaitForExit()
    return [pscustomobject]@{ Exit = $process.ExitCode; Output = $output; Stderr = $errors.Result }
}

function Get-StoreCertificate([string]$Thumbprint) {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new("My", "CurrentUser")
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadOnly)
    try {
        return @($store.Certificates | Where-Object { $_.Thumbprint -ceq $Thumbprint })
    } finally {
        $store.Close()
    }
}

function Test-NoStoreCert([string]$Thumbprint, [string]$Label) {
    Assert-Equal 0 (Get-StoreCertificate $Thumbprint).Count "$Label left the certificate in CurrentUser\My"
}

function Test-KeyGone([string]$KeyLine, [string]$Label) {
    $parts = $KeyLine.Substring(4).Split("|")
    if ($parts.Count -lt 3) {
        $failures.Add("$Label has no key line: $KeyLine")
        return
    }
    $exists = $false
    if ($parts[2] -eq "0") {
        $exists = [System.Security.Cryptography.CngKey]::Exists($parts[1], [System.Security.Cryptography.CngProvider]::new($parts[0]))
    } else {
        $csp = [System.Security.Cryptography.CspParameters]::new([int]$parts[2], $parts[0], $parts[1])
        $csp.Flags = [System.Security.Cryptography.CspProviderFlags]::UseExistingKey
        try {
            [System.Security.Cryptography.RSACryptoServiceProvider]::new($csp).Dispose()
            $exists = $true
        } catch {
        }
    }
    Assert-That (-not $exists) "$Label left the private key $($parts[1])"
}

function Read-ArgvLog([string]$Path) {
    $calls = [System.Collections.Generic.List[object]]::new()
    $current = [System.Collections.Generic.List[string]]::new()
    if (Test-Path -LiteralPath $Path) {
        foreach ($line in [System.IO.File]::ReadAllLines($Path)) {
            if ($line -ceq "--") {
                $calls.Add($current.ToArray())
                $current = [System.Collections.Generic.List[string]]::new()
            } else {
                $current.Add($line)
            }
        }
    }
    return , $calls
}

function Test-ArgvSign([string]$Label, [string]$LogName, [hashtable]$Extra, [int]$ExpectedExit, [int]$ExpectedCalls, [string[]]$SignArguments) {
    $log = Join-Path $work $LogName
    Remove-Item -LiteralPath $log -ErrorAction SilentlyContinue
    $environment = @{ DX_MSIX_SIGNTOOL = $env:DX_MSIX_ARGV_SIGNTOOL; DX_MSIX_ARGV_FILE = $log } + $Extra
    $result = Invoke-Sign $SignArguments $env:DX_MSIX_PACKAGE $environment
    Assert-Equal $ExpectedExit $result.Exit "$Label exit ($($result.Stderr))"
    $calls = Read-ArgvLog $log
    Assert-Equal $ExpectedCalls $calls.Count "$Label signtool invocations"
    foreach ($call in $calls) {
        $text = $call -join "`n"
        Assert-That (-not $text.Contains($env:DX_MSIX_PFX_PASSWORD)) "$Label signtool argv has the password"
        Assert-That ($call -cnotcontains "/p") "$Label signtool argv has /p"
        Assert-That ($call -cnotcontains "/f") "$Label signtool argv has /f"
        $at = [Array]::IndexOf($call, "/sha1")
        Assert-That ($at -ge 0 -and $call.Count -gt $at + 3 -and $call[$at + 1] -ceq $certificate.Thumbprint -and $call[$at + 2] -ceq "/s" -and $call[$at + 3] -ceq "My") "$Label signtool argv is not /sha1 <thumbprint> /s My: $text"
        $key = @($call | Where-Object { $_.StartsWith("key=") })
        Assert-That ($key.Count -eq 1 -and $key[0] -cne "key=none") "$Label signtool saw no key in the store: $($key -join ',')"
        if ($key.Count -eq 1 -and $key[0] -cne "key=none") {
            $script:lastKey = $key[0]
        }
    }
    return [pscustomobject]@{ Result = $result; Calls = $calls }
}

function Read-Zip([byte[]]$Bytes) {
    $zip = [System.IO.Compression.ZipArchive]::new([System.IO.MemoryStream]::new($Bytes))
    $entries = [ordered]@{}
    try {
        foreach ($entry in $zip.Entries) {
            $stream = $entry.Open()
            $buffer = [System.IO.MemoryStream]::new()
            try {
                $stream.CopyTo($buffer)
            } finally {
                $stream.Dispose()
            }
            $entries[$entry.FullName] = $buffer.ToArray()
        }
    } finally {
        $zip.Dispose()
    }
    return $entries
}

function Get-LocalHeaderOffset([byte[]]$Bytes, [string]$Name) {
    $wanted = [System.Text.Encoding]::UTF8.GetBytes($Name)
    for ($offset = 0; $offset + 30 + $wanted.Length -le $Bytes.Length; $offset++) {
        if ($Bytes[$offset] -ne 0x50 -or [System.BitConverter]::ToUInt32($Bytes, $offset) -ne 0x04034b50) {
            continue
        }
        if ([System.BitConverter]::ToUInt16($Bytes, $offset + 26) -eq $wanted.Length -and
            [System.Text.Encoding]::UTF8.GetString($Bytes, $offset + 30, $wanted.Length) -ceq $Name) {
            return $offset
        }
    }
    return -1
}

function Get-AppxDigests([byte[]]$Content) {
    $text = [System.Text.Encoding]::Latin1.GetString($Content)
    $start = $text.IndexOf("APPX")
    $digests = @{}
    if ($start -lt 0) {
        return $digests
    }
    for ($i = $start + 4; $i + 36 -le $Content.Length; $i += 36) {
        $digests[$text.Substring($i, 4)] = [System.Convert]::ToHexString($Content, $i + 4, 32).ToLowerInvariant()
    }
    return $digests
}

function Test-SignedPackage([string]$Label, [string]$Path, $Identity, $Certificate, [bool]$Timestamped) {
    $bytes = [System.IO.File]::ReadAllBytes($Path)
    $entries = Read-Zip $bytes
    $names = @($entries.Keys)
    Assert-Equal "AppxSignature.p7x" $names[-1] "$Label last entry"
    foreach ($name in @($Identity.files.Keys) + @("AppxBlockMap.xml", "[Content_Types].xml", "AppxMetadata/CodeIntegrity.cat")) {
        Assert-That ($names -contains $name) "$Label has no $name"
    }
    foreach ($name in $Identity.files.Keys) {
        if ($entries.Contains($name)) {
            Assert-Equal $Identity.files[$name] (Get-Sha $entries[$name]) "$Label $name sha256"
        }
    }
    if (-not $entries.Contains("AppxSignature.p7x")) {
        return
    }
    $p7x = $entries["AppxSignature.p7x"]
    Assert-Equal "PKCX" ([System.Text.Encoding]::ASCII.GetString($p7x, 0, 4)) "$Label signature magic"
    $cms = [System.Security.Cryptography.Pkcs.SignedCms]::new()
    $cms.Decode($p7x[4..($p7x.Length - 1)])
    try {
        $cms.CheckSignature($true)
    } catch {
        $failures.Add("$Label CMS signature does not verify: $($_.Exception.Message)")
    }
    Assert-Equal "1.3.6.1.4.1.311.2.1.4" $cms.ContentInfo.ContentType.Value "$Label signed content type"
    Assert-Equal 1 $cms.SignerInfos.Count "$Label signer count"
    $signer = $cms.SignerInfos[0]
    Assert-Equal $Certificate.Thumbprint $signer.Certificate.Thumbprint "$Label CMS signer"
    Assert-Equal "2.16.840.1.101.3.4.2.1" $signer.DigestAlgorithm.Value "$Label digest algorithm"
    $countersigned = @($signer.UnsignedAttributes | Where-Object { $_.Oid.Value -in @("1.3.6.1.4.1.311.3.3.1", "1.2.840.113549.1.9.6") }).Count -gt 0
    Assert-Equal $Timestamped $countersigned "$Label timestamp countersignature"

    $digests = Get-AppxDigests $cms.ContentInfo.Content
    $signatureOffset = Get-LocalHeaderOffset $bytes "AppxSignature.p7x"
    Assert-That ($signatureOffset -gt 0) "$Label has no AppxSignature.p7x local header"
    if ($signatureOffset -gt 0) {
        Assert-Equal (Get-Sha $bytes[0..($signatureOffset - 1)]) $digests["AXPC"] "$Label AXPC covers every packaged file record"
    }
    Assert-Equal (Get-Sha $entries["AppxBlockMap.xml"]) $digests["AXBM"] "$Label AXBM is the block map"
    Assert-Equal (Get-Sha $entries["[Content_Types].xml"]) $digests["AXCT"] "$Label AXCT is the content types"
    Assert-Equal (Get-Sha $entries["AppxMetadata/CodeIntegrity.cat"]) $digests["AXCI"] "$Label AXCI is the code integrity catalog"
    Assert-That ($digests.ContainsKey("AXCD")) "$Label has no AXCD digest"

    $windows = Get-AuthenticodeSignature -LiteralPath $Path
    Assert-That ($null -ne $windows.SignerCertificate) "$Label Windows sees no signer"
    if ($null -ne $windows.SignerCertificate) {
        Assert-Equal $Certificate.Thumbprint $windows.SignerCertificate.Thumbprint "$Label Windows signer thumbprint"
        Assert-Equal $Identity.publisher $windows.SignerCertificate.Subject "$Label Windows signer subject"
    }
    Assert-Equal "UnknownError" $windows.Status "$Label Windows status of an untrusted test certificate"
    Assert-That ($windows.StatusMessage.Contains("root certificate which is not trusted")) "$Label Windows status message: $($windows.StatusMessage)"

    $firstData = 30 + [System.BitConverter]::ToUInt16($bytes, 26) + [System.BitConverter]::ToUInt16($bytes, 28)
    $tampered = [byte[]]$bytes.Clone()
    $tampered[$firstData] = $tampered[$firstData] -bxor 1
    $tamperedPath = "$Path.tampered.msix"
    [System.IO.File]::WriteAllBytes($tamperedPath, $tampered)
    Assert-Equal "HashMismatch" (Get-AuthenticodeSignature -LiteralPath $tamperedPath).Status "$Label Windows status of a changed package"
    Assert-That ((Get-Sha $tampered[0..($signatureOffset - 1)]) -ne $digests["AXPC"]) "$Label AXPC ignores a changed byte"
}

function Start-TimestampServer([int]$Status) {
    $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
    $listener.Start()
    $port = $listener.LocalEndpoint.Port
    $listener.Stop()
    $http = [System.Net.HttpListener]::new()
    $http.Prefixes.Add("http://127.0.0.1:$port/")
    $http.Start()
    $job = Start-ThreadJob -ArgumentList $http, $Status -ScriptBlock {
        param($Http, $Status)
        $served = 0
        while ($Http.IsListening) {
            $pending = $Http.GetContextAsync()
            try {
                if (-not $pending.Wait(60000)) {
                    break
                }
            } catch {
                break
            }
            $context = $pending.Result
            $served++
            $context.Response.StatusCode = $Status
            $context.Response.Close()
        }
        $served
    }
    return [pscustomobject]@{ Http = $http; Job = $job; Url = "http://127.0.0.1:$port/" }
}

function Stop-TimestampServer($Server) {
    $Server.Http.Stop()
    $served = Receive-Job -Job $Server.Job -Wait -AutoRemoveJob
    return [int]($served | Select-Object -Last 1)
}

function Assert-Refused([string]$Label, $Result, [int]$Exit, [string]$Message, [string]$Out) {
    Assert-Equal $Exit $Result.Exit "$Label exit ($($Result.Stderr))"
    Assert-That ($Result.Stderr.Contains($Message)) "$Label stderr: $($Result.Stderr)"
    Assert-Equal "" $Result.Output.Trim() "$Label stdout"
    Assert-That (-not (Test-Path -LiteralPath $Out)) "$Label left $Out"
}

$sign = $runfiles.Rlocation($env:DX_MSIX_SIGN)
$package = Find-Runfile $env:DX_MSIX_PACKAGE ".msix"
$identity = Get-Content -LiteralPath (Find-Runfile $env:DX_MSIX_PACKAGE ".msix.identity.json") -Raw | ConvertFrom-Json -AsHashtable
$unsigned = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
$work = $env:TEST_TMPDIR
$env:DX_MSIX_PFX_PASSWORD = "dx-test-only"

$good = Join-Path $work "good.pfx"
$certificate = New-TestCertificate $identity.publisher $good $env:DX_MSIX_PFX_PASSWORD
$signed = Join-Path $work "signed.msix"
$result = Invoke-Sign @("-Pfx", $good, "-Out", $signed)
Assert-Equal 0 $result.Exit "sign exit ($($result.Stderr))"
if ($result.Exit -eq 0) {
    $digest = (Get-FileHash -LiteralPath $signed -Algorithm SHA256).Hash.ToLowerInvariant()
    $expected = "signed=$signed`nsha256=$digest`npublisher=$($identity.publisher)`nsigner=$($certificate.Thumbprint)`ntimestamp=none"
    Assert-Equal $expected ($result.Output.Trim() -replace "`r", "") "sign output"
    Assert-That ($digest -ne $unsigned) "signed package equals the unsigned package"
    Test-SignedPackage "signed" $signed $identity $certificate $false
}

Test-NoStoreCert $certificate.Thumbprint "sign"

$script:lastKey = ""
$case = Test-ArgvSign "argv" "argv1.log" @{} 0 1 @("-Pfx", $good, "-Out", (Join-Path $work "argv1.msix"))
Test-NoStoreCert $certificate.Thumbprint "argv"
Test-KeyGone $script:lastKey "argv"

$script:lastKey = ""
$case = Test-ArgvSign "argv timestamp" "argv2.log" @{} 0 2 @("-Pfx", $good, "-Out", (Join-Path $work "argv2.msix"), "-Timestamp", "http://127.0.0.1:1/")
if ($case.Calls.Count -eq 2) {
    Assert-That ($case.Calls[1] -ccontains "/tr") "argv timestamp: second signtool call has no /tr"
}
Test-NoStoreCert $certificate.Thumbprint "argv timestamp"
Test-KeyGone $script:lastKey "argv timestamp"

$script:lastKey = ""
Test-ArgvSign "failing signtool" "argv3.log" @{ DX_MSIX_ARGV_EXIT = "1" } 1 1 @("-Pfx", $good, "-Out", (Join-Path $work "argv3.msix")) | Out-Null
Test-NoStoreCert $certificate.Thumbprint "failing signtool"
Test-KeyGone $script:lastKey "failing signtool"

$flags = [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::UserKeySet -bor [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::PersistKeySet
foreach ($marker in @($false, $true)) {
    $label = if ($marker) { "stale marker" } else { "existing certificate" }
    $own = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($good, $env:DX_MSIX_PFX_PASSWORD, $flags)
    if ($marker) {
        $own.FriendlyName = "dx_msix_sign temporary"
    }
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new("My", "CurrentUser")
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    $store.Add($own)
    $store.Close()
    $script:lastKey = ""
    try {
        Test-ArgvSign $label "argv-$($marker).log" @{} 0 1 @("-Pfx", $good, "-Out", (Join-Path $work "argv-$($marker).msix")) | Out-Null
        $left = Get-StoreCertificate $certificate.Thumbprint
        if ($marker) {
            Assert-Equal 0 $left.Count "$label left the certificate in CurrentUser\My"
            Test-KeyGone $script:lastKey $label
        } else {
            Assert-Equal 1 $left.Count "$label removed the user's certificate"
            Assert-That ($left.Count -eq 1 -and $left[0].HasPrivateKey) "$label removed the user's private key"
        }
    } finally {
        $store = [System.Security.Cryptography.X509Certificates.X509Store]::new("My", "CurrentUser")
        $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
        foreach ($found in $store.Certificates.Find([System.Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint, $certificate.Thumbprint, $false)) {
            $store.Remove($found)
        }
        $store.Close()
        try {
            $key = [System.Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPrivateKey($own)
            if ($key -is [System.Security.Cryptography.RSACng]) {
                $key.Key.Delete()
            }
        } catch {
        }
    }
}

$public = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new($certificate.RawData)
$public.FriendlyName = "user public only"
$store = [System.Security.Cryptography.X509Certificates.X509Store]::new("My", "CurrentUser")
$store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
$store.Add($public)
$store.Close()
$script:lastKey = ""
try {
    Test-ArgvSign "keyless certificate" "argv-keyless.log" @{} 0 1 @("-Pfx", $good, "-Out", (Join-Path $work "argv-keyless.msix")) | Out-Null
    $left = Get-StoreCertificate $certificate.Thumbprint
    Assert-Equal 1 $left.Count "keyless certificate was not restored to CurrentUser\My"
    Assert-That ($left.Count -eq 1 -and -not $left[0].HasPrivateKey) "keyless certificate got a private key"
    Assert-That ($left.Count -eq 1 -and $left[0].FriendlyName -ceq "user public only") "keyless certificate lost its FriendlyName"
    Test-KeyGone $script:lastKey "keyless certificate"
} finally {
    $store = [System.Security.Cryptography.X509Certificates.X509Store]::new("My", "CurrentUser")
    $store.Open([System.Security.Cryptography.X509Certificates.OpenFlags]::ReadWrite)
    foreach ($found in $store.Certificates.Find([System.Security.Cryptography.X509Certificates.X509FindType]::FindByThumbprint, $certificate.Thumbprint, $false)) {
        $store.Remove($found)
    }
    $store.Close()
}

$arm64 = Get-Content -LiteralPath (Find-Runfile $env:DX_MSIX_ARM64 ".msix.identity.json") -Raw | ConvertFrom-Json -AsHashtable
Assert-Equal "arm64" $arm64.arch "arm64 package arch"
$signedArm64 = Join-Path $work "signed-arm64.msix"
$result = Invoke-Sign @("-Pfx", $good, "-Out", $signedArm64) $env:DX_MSIX_ARM64
Assert-Equal 0 $result.Exit "arm64 sign exit ($($result.Stderr))"
if ($result.Exit -eq 0) {
    Test-SignedPackage "arm64" $signedArm64 $arm64 $certificate $false
}
Test-NoStoreCert $certificate.Thumbprint "arm64 sign"
Assert-Equal $unsigned (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant() "unsigned package changed on disk"
Assert-Equal $unsigned $identity.package_sha256 "unsigned package differs from its identity"

$rejected = Join-Path $work "rejected.msix"
$wrong = Join-Path $work "wrong.pfx"
New-TestCertificate "CN=SomeoneElse" $wrong $env:DX_MSIX_PFX_PASSWORD | Out-Null
Assert-Refused "publisher mismatch" (Invoke-Sign @("-Pfx", $wrong, "-Out", $rejected)) 3 "certificate subject CN=SomeoneElse does not match the package publisher $($identity.publisher)" $rejected

$unspaced = Join-Path $work "unspaced.pfx"
New-TestCertificate ($identity.publisher -replace ", ", ",") $unspaced $env:DX_MSIX_PFX_PASSWORD | Out-Null
$unspacedOut = Join-Path $work "unspaced.msix"
$result = Invoke-Sign @("-Pfx", $unspaced, "-Out", $unspacedOut)
Assert-Equal 0 $result.Exit "publisher without spaces exit ($($result.Stderr))"

$expired = Join-Path $work "expired.pfx"
New-TestCertificate $identity.publisher $expired $env:DX_MSIX_PFX_PASSWORD $codeSigning -3 -2 | Out-Null
Assert-Refused "expired certificate" (Invoke-Sign @("-Pfx", $expired, "-Out", $rejected)) 3 "certificate $expired is valid only from" $rejected
$future = Join-Path $work "future.pfx"
New-TestCertificate $identity.publisher $future $env:DX_MSIX_PFX_PASSWORD $codeSigning 2 3 | Out-Null
Assert-Refused "not yet valid certificate" (Invoke-Sign @("-Pfx", $future, "-Out", $rejected)) 3 "certificate $future is valid only from" $rejected

$env:DX_MSIX_PFX_PASSWORD = "not-the-password"
Assert-Refused "wrong password" (Invoke-Sign @("-Pfx", $good, "-Out", $rejected)) 3 "dx_msix_sign: cannot open certificate $good" $rejected
Test-NoStoreCert $certificate.Thumbprint "wrong password"
$env:DX_MSIX_PFX_PASSWORD = "dx-test-only"

$public = Join-Path $work "public.cer"
[System.IO.File]::WriteAllBytes($public, $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Cert))
Assert-Refused "public certificate" (Invoke-Sign @("-Pfx", $public, "-Out", $rejected)) 3 "certificate $public has no private key" $rejected

$server = Join-Path $work "server.pfx"
New-TestCertificate $identity.publisher $server $env:DX_MSIX_PFX_PASSWORD "1.3.6.1.5.5.7.3.1" | Out-Null
Assert-Refused "server certificate" (Invoke-Sign @("-Pfx", $server, "-Out", $rejected)) 1 "dx_msix_sign: signtool failed with exit code 1" $rejected

$kept = Join-Path $work "kept.msix"
[System.IO.File]::WriteAllText($kept, "existing")
$result = Invoke-Sign @("-Pfx", $server, "-Out", $kept)
Assert-Equal 1 $result.Exit "failed sign over an existing file exit ($($result.Stderr))"
Assert-Equal "existing" ([System.IO.File]::ReadAllText($kept)) "failed sign changed the existing output"
Assert-Equal 0 @(Get-ChildItem -LiteralPath $work -Filter "kept.*.signing.msix").Count "failed sign left a staging file"

foreach ($case in @(@{ Label = "rejected timestamp"; Status = 500 }, @{ Label = "invalid timestamp response"; Status = 200 })) {
    $timestamp = Start-TimestampServer $case.Status
    $result = Invoke-Sign @("-Pfx", $good, "-Out", $rejected, "-Timestamp", $timestamp.Url)
    $served = Stop-TimestampServer $timestamp
    Assert-Refused $case.Label $result 4 "dx_msix_sign: timestamp server $($timestamp.Url) rejected the request, sent an invalid response, or could not be reached" $rejected
    Assert-That ($served -ge 1) "$($case.Label): the server got no request"
}

$closed = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$closed.Start()
$closedUrl = "http://127.0.0.1:$($closed.LocalEndpoint.Port)/"
$closed.Stop()
Assert-Refused "unreachable timestamp" (Invoke-Sign @("-Pfx", $good, "-Out", $rejected, "-Timestamp", $closedUrl)) 4 "timestamp server $closedUrl rejected" $rejected
$timestamp = Start-TimestampServer 200
$result = Invoke-Sign @("-Pfx", $server, "-Out", $rejected, "-Timestamp", $timestamp.Url)
$served = Stop-TimestampServer $timestamp
Assert-Refused "server certificate with timestamp" $result 1 "dx_msix_sign: signtool failed with exit code 1" $rejected
Assert-Equal 0 $served "server certificate with timestamp: the server got a request"
Assert-Refused "timestamp scheme" (Invoke-Sign @("-Pfx", $good, "-Out", $rejected, "-Timestamp", "ftp://127.0.0.1/")) 2 "-Timestamp ftp://127.0.0.1/ is not an http or https URL" $rejected

Assert-Refused "missing certificate" (Invoke-Sign @("-Out", $rejected)) 2 "dx_msix_sign: pass -Pfx" $rejected
Assert-Refused "missing output" (Invoke-Sign @("-Pfx", $good)) 2 "dx_msix_sign: pass -Out" $rejected
Assert-Refused "absent certificate" (Invoke-Sign @("-Pfx", (Join-Path $work "absent.pfx"), "-Out", $rejected)) 2 "does not exist" $rejected

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "sign_test: all checks passed"
