using module Runfiles

param(
    [string]$Pfx,
    [string]$Out,
    [string]$Timestamp
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Stop-Sign([int]$Code, [string]$Message) {
    [Console]::Error.WriteLine("dx_msix_sign: $Message")
    [Environment]::Exit($Code)
}

function Resolve-Runfile($Runfiles, [string]$Paths, [string]$Suffix) {
    foreach ($path in ($Paths -split " ")) {
        if ($path.EndsWith($Suffix, [System.StringComparison]::OrdinalIgnoreCase)) {
            return $Runfiles.Rlocation($path)
        }
    }
    throw "no $Suffix among $Paths"
}

function Resolve-UserPath([string]$Path) {
    if (-not [System.IO.Path]::IsPathRooted($Path) -and $env:BUILD_WORKING_DIRECTORY) {
        $Path = Join-Path $env:BUILD_WORKING_DIRECTORY $Path
    }
    return [System.IO.Path]::GetFullPath($Path)
}

function Get-FileSha256([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Invoke-Signtool([string]$Signtool, [string[]]$Arguments) {
    $output = & $Signtool @Arguments 2>&1 | Out-String
    return [pscustomobject]@{ Exit = $LASTEXITCODE; Output = $output.Trim() }
}

function Copy-Package([string]$Package, [string]$Target) {
    Copy-Item -LiteralPath $Package -Destination $Target -Force
    Set-ItemProperty -LiteralPath $Target -Name IsReadOnly -Value $false
}

if (-not $Pfx) {
    Stop-Sign 2 "pass -Pfx <certificate.pfx>"
}
if (-not $Out) {
    Stop-Sign 2 "pass -Out <signed.msix>"
}
if ($Timestamp) {
    $uri = $null
    if (-not [System.Uri]::TryCreate($Timestamp, [System.UriKind]::Absolute, [ref]$uri) -or $uri.Scheme -notin @("http", "https")) {
        Stop-Sign 2 "-Timestamp $Timestamp is not an http or https URL"
    }
}
$certificatePath = Resolve-UserPath $Pfx
if (-not (Test-Path -LiteralPath $certificatePath -PathType Leaf)) {
    Stop-Sign 2 "certificate $certificatePath does not exist"
}
$target = Resolve-UserPath $Out

try {
    $runfiles = [Runfiles]::Create()
    $package = Resolve-Runfile $runfiles $env:DX_MSIX_PACKAGE ".msix"
    $identity = Get-Content -LiteralPath (Resolve-Runfile $runfiles $env:DX_MSIX_PACKAGE ".msix.identity.json") -Raw | ConvertFrom-Json
    $signtool = Resolve-Runfile $runfiles $env:DX_MSIX_SIGNTOOL "signtool.exe"
} catch {
    Stop-Sign 1 $_.Exception.Message
}

try {
    $certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new(
        $certificatePath, $env:DX_MSIX_PFX_PASSWORD, [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]::EphemeralKeySet)
} catch {
    Stop-Sign 3 "cannot open certificate ${certificatePath}: $($_.Exception.Message)"
}
if (-not $certificate.HasPrivateKey) {
    Stop-Sign 3 "certificate $certificatePath has no private key"
}
if ($certificate.NotAfter -lt [DateTime]::Now -or $certificate.NotBefore -gt [DateTime]::Now) {
    Stop-Sign 3 "certificate $certificatePath is valid only from $($certificate.NotBefore.ToString("o")) to $($certificate.NotAfter.ToString("o"))"
}
try {
    $publisher = [System.Security.Cryptography.X509Certificates.X500DistinguishedName]::new($identity.publisher)
} catch {
    Stop-Sign 3 "package publisher $($identity.publisher) is not a valid distinguished name: $($_.Exception.Message)"
}
if ($certificate.SubjectName.Decode(0) -cne $publisher.Decode(0)) {
    Stop-Sign 3 "certificate subject $($certificate.Subject) does not match the package publisher $($identity.publisher)"
}

$arguments = @("sign", "/fd", "SHA256", "/f", $certificatePath)
if ($env:DX_MSIX_PFX_PASSWORD) {
    $arguments += @("/p", $env:DX_MSIX_PFX_PASSWORD)
}
$staging = "$target.$PID.signing"
$failCode = 0
$failMessage = ""
try {
    Copy-Package $package $staging
    $result = Invoke-Signtool $signtool ($arguments + @($staging))
    if ($result.Exit -ne 0) {
        $failCode = 1
        $failMessage = "signtool failed with exit code $($result.Exit)`n$($result.Output)"
    } elseif ($Timestamp) {
        Copy-Package $package $staging
        $result = Invoke-Signtool $signtool ($arguments + @("/tr", $Timestamp, "/td", "SHA256", $staging))
        if ($result.Exit -ne 0) {
            $failCode = 4
            $failMessage = "timestamp server $Timestamp rejected the request or could not be reached`n$($result.Output)"
        }
    }
    if ($failCode -eq 0) {
        Move-Item -LiteralPath $staging -Destination $target -Force
    }
} catch {
    $failCode = 1
    $failMessage = $_.Exception.Message
} finally {
    if (Test-Path -LiteralPath $staging) {
        Remove-Item -LiteralPath $staging -Force -ErrorAction SilentlyContinue
    }
}
if ($failCode -ne 0) {
    Stop-Sign $failCode $failMessage
}
Write-Output "signed=$target"
Write-Output "sha256=$(Get-FileSha256 $target)"
Write-Output "publisher=$($identity.publisher)"
Write-Output "signer=$($certificate.Thumbprint)"
Write-Output "timestamp=$(if ($Timestamp) { $Timestamp } else { "none" })"
