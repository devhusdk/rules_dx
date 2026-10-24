using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.Security

$runfiles = [Runfiles]::Create()
$failures = [System.Collections.Generic.List[string]]::new()
$codeSigning = "1.3.6.1.5.5.7.3.3"
$password = "dx-test-only"

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

function Resolve-Source([string]$Path) {
    $item = Get-Item -LiteralPath $runfiles.Rlocation($Path)
    if ($item.LinkTarget) {
        return $item.ResolveLinkTarget($true).FullName
    }
    return $item.FullName
}

function Invoke-Tool([string]$File, [string[]]$Arguments, [string]$Directory) {
    $start = [System.Diagnostics.ProcessStartInfo]::new($File)
    $start.WorkingDirectory = $Directory
    foreach ($name in @($start.Environment.Keys)) {
        if ($name -match "^(TEST_|RUNFILES_|BUILD_|BAZEL_TEST|JAVA_RUNFILES|PYTHON_RUNFILES|XML_OUTPUT_FILE|TESTBRIDGE_)") {
            $start.Environment.Remove($name) | Out-Null
        }
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
    return [pscustomobject]@{ Exit = $process.ExitCode; Output = $output.Replace("`r", "").Trim(); Stderr = $errors.Result.Replace("`r", "").Trim() }
}

function New-TestCertificate([string]$Subject, [string]$Path) {
    $rsa = [System.Security.Cryptography.RSA]::Create(2048)
    $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
        $Subject, $rsa, [System.Security.Cryptography.HashAlgorithmName]::SHA256, [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
    $usages = [System.Security.Cryptography.OidCollection]::new()
    $usages.Add([System.Security.Cryptography.Oid]::new($codeSigning)) | Out-Null
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension]::new($usages, $false))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
        [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature, $true))
    $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
    $now = [System.DateTimeOffset]::UtcNow
    $certificate = $request.CreateSelfSigned($now.AddDays(-1), $now.AddDays(1))
    [System.IO.File]::WriteAllBytes($Path, $certificate.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pfx, $password))
    return $certificate
}

function Read-Zip([string]$Path) {
    $zip = [System.IO.Compression.ZipFile]::OpenRead($Path)
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

function Copy-Consumer([string]$Paths, [string]$Workspace) {
    foreach ($pair in ($Paths -split " ")) {
        $source, $dest = $pair -split "=", 2
        $target = Join-Path $Workspace $dest
        New-Item -ItemType Directory -Force -Path (Split-Path $target) | Out-Null
        $text = [System.IO.File]::ReadAllText((Resolve-Source $source))
        if ($dest.EndsWith("BUILD.bazel")) {
            $kept = foreach ($line in ($text -split "`r?`n")) {
                $line.Replace("@rules_dx//", "@my_dx//")
            }
            $text = $kept -join "`n"
        }
        [System.IO.File]::WriteAllText($target, $text)
    }
}

$short = Join-Path ([System.IO.Path]::GetTempPath()) ("dxc" + [System.Guid]::NewGuid().ToString("N").Substring(0, 8))
$workspace = Join-Path $short "ws"
$base = Join-Path $short "out"
New-Item -ItemType Directory -Force -Path $workspace | Out-Null
$bazel = (Get-Command bazel -ErrorAction Stop).Source
$source = Split-Path (Resolve-Source $env:DX_MSIX_MODULE) -Parent

try {
    $root = [System.IO.File]::ReadAllText((Resolve-Source $env:DX_MSIX_MODULE))
    function Get-RootVersion([string]$Pattern) {
        $found = [regex]::Match($root, $Pattern)
        if (-not $found.Success) {
            throw "root MODULE.bazel has no match for $Pattern"
        }
        return $found.Groups[1].Value
    }
    $dxVersion = Get-RootVersion 'module\(\s*name = "rules_dx",\s*version = "([^"]+)"'
    $ccVersion = Get-RootVersion 'bazel_dep\(name = "rules_cc", version = "([^"]+)"'
    $llvmVersion = Get-RootVersion 'bazel_dep\(name = "llvm", version = "([^"]+)"'
    $fragment = [System.IO.File]::ReadAllText((Resolve-Source $env:DX_MSIX_FRAGMENT))
    $module = "module(name = `"hermetic_consumer`")`n`n" +
        "bazel_dep(name = `"rules_dx`", repo_name = `"my_dx`", version = `"$dxVersion`")`n" +
        "bazel_dep(name = `"rules_cc`", version = `"$ccVersion`")`n" +
        "bazel_dep(name = `"llvm`", version = `"$llvmVersion`")`n" +
        "register_toolchains(`"@llvm//toolchain:all`")`n`n" + $fragment.TrimEnd() +
        "`nlocal_path_override(`n    module_name = `"rules_dx`",`n    path = `"$($source.Replace('\', '/'))`",`n)`n"
    [System.IO.File]::WriteAllText((Join-Path $workspace "MODULE.bazel"), $module)
    Copy-Item -LiteralPath (Resolve-Source $env:DX_MSIX_BAZELVERSION) -Destination (Join-Path $workspace ".bazelversion")
    [System.IO.File]::WriteAllText((Join-Path $workspace ".bazelrc"), @"
common --repo_env=BAZEL_MSVC_RUNTIME_VISUAL_STUDIO_EULA=1
common --repo_env=BAZEL_WINDOWS_SDK_EULA=1
common --host_platform=@@rules_dx+//tools/platforms:windows_msvc_host
common --extra_execution_platforms=@@rules_dx+//tools/platforms:windows_msvc_host,@platforms//host
"@)
    Copy-Consumer $env:DX_MSIX_FILES $workspace

    $startup = @("--output_base=$base", "--windows_enable_symlinks")
    $build = Invoke-Tool $bazel ($startup + @("build", "//examples/msix-app/app:app", "//examples/msix-app/app:app_sign")) $workspace
    Assert-Equal 0 $build.Exit "consumer build exit ($($build.Stderr))"
    $info = Invoke-Tool $bazel ($startup + @("info", "bazel-bin")) $workspace
    Assert-Equal 0 $info.Exit "consumer info exit ($($info.Stderr))"
    $bin = $info.Output
    $package = Join-Path $bin "examples/msix-app/app/app.msix"
    $identity = Get-Content -LiteralPath "$package.identity.json" -Raw | ConvertFrom-Json -AsHashtable
    Assert-Equal "DxMsix.Example" $identity.name "consumer identity name"
    Assert-Equal "CN=DxMsixExample" $identity.publisher "consumer publisher"
    Assert-Equal "1.2.3.0" $identity.version "consumer version"
    $unsigned = Read-Zip $package
    Assert-That (-not $unsigned.Contains("AppxSignature.p7x")) "the built consumer package is signed"
    Assert-That ($unsigned.Contains("share/greeting.txt")) "the consumer package has no resource"
    Assert-That ($unsigned.Contains("greet.dll")) "the consumer package has no native dependency"

    $good = Join-Path $short "good.pfx"
    $certificate = New-TestCertificate $identity.publisher $good
    $wrong = Join-Path $short "wrong.pfx"
    New-TestCertificate "CN=Someone Else" $wrong | Out-Null
    $env:DX_MSIX_PFX_PASSWORD = $password

    $refusedOut = Join-Path $short "refused.msix"
    $refused = Invoke-Tool $bazel ($startup + @("run", "//examples/msix-app/app:app_sign", "--", "-Pfx", $wrong, "-Out", $refusedOut)) $workspace
    Assert-That ($refused.Exit -ne 0) "a publisher mismatch signed the consumer package"
    Assert-That ($refused.Stderr.Contains("dx_msix_sign:")) "a publisher mismatch gave no signer message: $($refused.Stderr)"
    Assert-That (-not (Test-Path -LiteralPath $refusedOut)) "a publisher mismatch wrote $refusedOut"

    $missing = Invoke-Tool $bazel ($startup + @("run", "//examples/msix-app/app:app_sign", "--", "-Out", $refusedOut)) $workspace
    Assert-That ($missing.Stderr.Contains("pass -Pfx <certificate.pfx>")) "a missing certificate gave: $($missing.Stderr)"
    Assert-That (-not (Test-Path -LiteralPath $refusedOut)) "a missing certificate wrote $refusedOut"

    $signedOut = Join-Path $short "signed.msix"
    $signed = Invoke-Tool $bazel ($startup + @("run", "//examples/msix-app/app:app_sign", "--", "-Pfx", $good, "-Out", $signedOut)) $workspace
    Assert-Equal 0 $signed.Exit "consumer sign exit ($($signed.Stderr))"
    $bytes = [System.IO.File]::ReadAllBytes($signedOut)
    $expected = "signed=$signedOut`nsha256=$(Get-Sha $bytes)`npublisher=$($identity.publisher)`nsigner=$($certificate.Thumbprint)`ntimestamp=none"
    Assert-Equal $expected $signed.Output "consumer sign output"
    Assert-That ((Get-Sha $bytes) -ne (Get-Sha ([System.IO.File]::ReadAllBytes($package)))) "the signed package equals the unsigned package"

    $entries = Read-Zip $signedOut
    Assert-Equal "AppxSignature.p7x" @($entries.Keys)[-1] "last entry of the signed package"
    foreach ($name in $identity.files.Keys) {
        Assert-Equal $identity.files[$name] (Get-Sha $entries[$name]) "signed package $name"
    }
    $p7x = $entries["AppxSignature.p7x"]
    Assert-Equal "PKCX" ([System.Text.Encoding]::ASCII.GetString($p7x, 0, 4)) "signature magic"
    $cms = [System.Security.Cryptography.Pkcs.SignedCms]::new()
    $cms.Decode($p7x[4..($p7x.Length - 1)])
    $cms.CheckSignature($true)
    Assert-Equal $certificate.Thumbprint $cms.SignerInfos[0].Certificate.Thumbprint "CMS signer"
    $content = [System.Text.Encoding]::Latin1.GetString($cms.ContentInfo.Content)
    $axbm = $content.IndexOf("AXBM")
    Assert-That ($axbm -ge 0) "the signature has no block map digest"
    if ($axbm -ge 0) {
        Assert-Equal (Get-Sha $entries["AppxBlockMap.xml"]) ([System.Convert]::ToHexString($cms.ContentInfo.Content, $axbm + 4, 32).ToLowerInvariant()) "signed block map digest"
    }
    $windows = Get-AuthenticodeSignature -LiteralPath $signedOut
    Assert-Equal $certificate.Thumbprint $windows.SignerCertificate.Thumbprint "Windows signer thumbprint"
    Assert-Equal $identity.publisher $windows.SignerCertificate.Subject "Windows signer subject"
    $tampered = [byte[]]$bytes.Clone()
    $tampered[30 + [System.BitConverter]::ToUInt16($bytes, 26) + [System.BitConverter]::ToUInt16($bytes, 28)] = $tampered[30 + [System.BitConverter]::ToUInt16($bytes, 26) + [System.BitConverter]::ToUInt16($bytes, 28)] -bxor 1
    $tamperedPath = Join-Path $short "tampered.msix"
    [System.IO.File]::WriteAllBytes($tamperedPath, $tampered)
    Assert-Equal "HashMismatch" (Get-AuthenticodeSignature -LiteralPath $tamperedPath).Status "Windows status of a changed package"

    $probe = Invoke-Tool $bazel ($startup + @("build", "//examples/msix-app/app:app", "--copt=-DDX_CONSUMER_PROBE")) $workspace
    Assert-Equal 0 $probe.Exit "consumer probe build exit ($($probe.Stderr))"
    $probed = Read-Zip $package
    Assert-That ([System.Text.Encoding]::Latin1.GetString($probed["dxmsix.exe"]).Contains("probe=on")) "the consumer copt did not reach the packaged executable"
} catch {
    $failures.Add("aborted: $($_.Exception.Message)")
} finally {
    Invoke-Tool $bazel ($startup + @("shutdown")) $workspace | Out-Null
    Remove-Item -LiteralPath $short -Recurse -Force -ErrorAction SilentlyContinue
}

if ($failures.Count -gt 0) {
    $failures | ForEach-Object { [Console]::Error.WriteLine("FAIL: $_") }
    [Environment]::Exit(1)
}
Write-Output "consumer_sign_test: all checks passed"
