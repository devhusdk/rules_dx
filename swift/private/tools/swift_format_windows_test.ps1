using module Runfiles

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$runfiles = [Runfiles]::Create()
$swiftFormat = [System.IO.Path]::GetFullPath($runfiles.Rlocation($env:DX_SWIFT_FORMAT))
if (-not (Test-Path -LiteralPath $swiftFormat)) { throw "$swiftFormat is missing" }

$version = & $swiftFormat --version 2>&1
if ($LASTEXITCODE -ne 0) { throw "swift-format --version exited ${LASTEXITCODE}: $version" }

$source = Join-Path $env:TEST_TMPDIR "main.swift"
[System.IO.File]::WriteAllText($source, "let x=1`n", (New-Object System.Text.UTF8Encoding $false))
$formatted = (& $swiftFormat format $source 2>&1) -join "`n"
if ($LASTEXITCODE -ne 0) { throw "swift-format format exited ${LASTEXITCODE}: $formatted" }
if ($formatted.Trim() -ne "let x = 1") { throw "swift-format printed '$formatted', not 'let x = 1'" }
