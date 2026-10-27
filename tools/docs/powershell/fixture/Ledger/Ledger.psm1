# Importing this module must never happen while extracting its docs.
Set-Content -Path "$env:TEST_TMPDIR/imported" -Value 'imported'
throw 'imported'

foreach ($file in Get-ChildItem -Path "$PSScriptRoot/Public", "$PSScriptRoot/Private" -Filter *.ps1) {
    . $file.FullName
}
