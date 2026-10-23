using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.IO.Compression

$runfiles = [Runfiles]::Create()
$failures = [System.Collections.Generic.List[string]]::new()

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

function New-TestCertificate([string]$Subject, [string]$Path, [string]$Password) {
    $rsa = [System.Security.Cryptography.RSA]::Create(2048)
    $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
        $Subject, $rsa, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $usages = [System.Security.Cryptography.OidCollection]::new()
    $usages.Add([System.Security.Cryptography.Oid]::new("1.3.6.1.5.5.7.3.3")) | Out-Null
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usages, $false))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
        [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature, $true))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
    $now = [System.DateTimeOffset]::UtcNow
    $certificate = $request.CreateSelfSigned($now.AddDays(-1), $now.AddDays(1))
    [System.IO.File]::WriteAllBytes($Path, $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $Password))
    return $certificate.Thumbprint
}

function Invoke-Sign([string[]]$Arguments) {
    $start = [System.Diagnostics.ProcessStartInfo]::new($sign)
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

$sign = $runfiles.Rlocation($env:DX_MSIX_SIGN)
$package = Find-Runfile $env:DX_MSIX_X64 ".msix"
$identity = Get-Content -LiteralPath (Find-Runfile $env:DX_MSIX_X64 ".msix.identity.json") -Raw | ConvertFrom-Json -AsHashtable
$unsigned = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
$work = $env:TEST_TMPDIR
$env:DX_MSIX_PFX_PASSWORD = "dx-test-only"

$good = Join-Path $work "good.pfx"
$thumbprint = New-TestCertificate $identity.publisher $good $env:DX_MSIX_PFX_PASSWORD
$signed = Join-Path $work "signed.msix"
$result = Invoke-Sign @("-Pfx", $good, "-Out", $signed)
Assert-Equal 0 $result.Exit "sign exit ($($result.Stderr))"
$digest = ""
if (Test-Path -LiteralPath $signed) {
    $digest = (Get-FileHash -LiteralPath $signed -Algorithm SHA256).Hash.ToLowerInvariant()
}
Assert-That ($result.Output.Contains("signed=$signed")) "sign output: $($result.Output)"
Assert-That ($result.Output.Contains("sha256=$digest")) "sign digest output: $($result.Output)"
Assert-That ($digest -ne $unsigned) "signed package equals the unsigned package"
Assert-Equal $unsigned $identity.package_sha256 "unsigned package changed"
if ($result.Exit -eq 0) {
    $signature = Get-AuthenticodeSignature -LiteralPath $signed
    Assert-That ($null -ne $signature.SignerCertificate) "signed package has no signer"
    if ($null -ne $signature.SignerCertificate) {
        Assert-Equal $thumbprint $signature.SignerCertificate.Thumbprint "signer thumbprint"
        Assert-Equal $identity.publisher $signature.SignerCertificate.Subject "signer subject"
    }
    $zip = [System.IO.Compression.ZipFile]::OpenRead($signed)
    try {
        $names = @($zip.Entries | ForEach-Object { $_.FullName })
    } finally {
        $zip.Dispose()
    }
    Assert-That ($names -contains "AppxSignature.p7x") "signed package has no AppxSignature.p7x"
    foreach ($name in $identity.files.Keys) {
        Assert-That ($names -contains $name) "signed package lost $name"
    }
}

$wrong = Join-Path $work "wrong.pfx"
New-TestCertificate "CN=SomeoneElse" $wrong $env:DX_MSIX_PFX_PASSWORD | Out-Null
$rejected = Join-Path $work "rejected.msix"
$result = Invoke-Sign @("-Pfx", $wrong, "-Out", $rejected)
Assert-Equal 1 $result.Exit "wrong subject exit"
Assert-That ($result.Stderr.Contains("dx_msix_sign: signtool failed")) "wrong subject stderr: $($result.Stderr)"
Assert-That (-not (Test-Path -LiteralPath $rejected)) "wrong subject left a package"

$result = Invoke-Sign @("-Out", (Join-Path $work "none.msix"))
Assert-Equal 2 $result.Exit "missing certificate exit"
Assert-That ($result.Stderr.Contains("dx_msix_sign: pass -Pfx")) "missing certificate stderr: $($result.Stderr)"

$result = Invoke-Sign @("-Pfx", (Join-Path $work "absent.pfx"), "-Out", (Join-Path $work "none.msix"))
Assert-Equal 2 $result.Exit "absent certificate exit"
Assert-That ($result.Stderr.Contains("does not exist")) "absent certificate stderr: $($result.Stderr)"

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "sign_test: all checks passed"
