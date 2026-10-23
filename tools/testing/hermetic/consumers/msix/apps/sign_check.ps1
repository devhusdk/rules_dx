using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$runfiles = [Runfiles]::Create()
$sign = $runfiles.Rlocation($env:DX_MSIX_SIGN)
$work = $env:TEST_TMPDIR
$password = "dx-consumer-only"

$rsa = [System.Security.Cryptography.RSA]::Create(2048)
$request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
    $env:DX_MSIX_PUBLISHER, $rsa, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
$usages = [System.Security.Cryptography.OidCollection]::new()
$usages.Add([System.Security.Cryptography.Oid]::new("1.3.6.1.5.5.7.3.3")) | Out-Null
$request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usages, $false))
$request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
$now = [System.DateTimeOffset]::UtcNow
$certificate = $request.CreateSelfSigned($now.AddDays(-1), $now.AddDays(1))
$pfx = Join-Path $work "consumer.pfx"
[System.IO.File]::WriteAllBytes($pfx, $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password))

$out = Join-Path $work "signed.msix"
$start = [System.Diagnostics.ProcessStartInfo]::new($sign)
$start.Environment["DX_MSIX_PFX_PASSWORD"] = $password
foreach ($argument in @("-Pfx", $pfx, "-Out", $out)) {
    $start.ArgumentList.Add($argument)
}
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$process = [System.Diagnostics.Process]::Start($start)
$errors = $process.StandardError.ReadToEndAsync()
$output = $process.StandardOutput.ReadToEnd()
$process.WaitForExit()

if ($process.ExitCode -ne 0) {
    throw "app_sign exited $($process.ExitCode): $($errors.Result)"
}
if (-not $output.Contains("signer=$($certificate.Thumbprint)")) {
    throw "app_sign output does not name the signer: $output"
}
$signature = Get-AuthenticodeSignature -LiteralPath $out
if ($null -eq $signature.SignerCertificate -or $signature.SignerCertificate.Thumbprint -ne $certificate.Thumbprint) {
    throw "$out is not signed by the test certificate"
}
Write-Output "sign_check: app_sign signed the package"
