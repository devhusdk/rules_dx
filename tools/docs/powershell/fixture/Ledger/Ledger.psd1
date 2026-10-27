@{
    RootModule        = 'Ledger.psm1'
    ModuleVersion     = '1.4.0'
    GUID              = '6a1c6d1e-8f0e-4b8a-9a43-1d2f0c9a7e51'
    Author            = 'rules_dx'
    FunctionsToExport = @('Add-LedgerEntry', 'Get-Ledger*', 'Remove-LedgerEntry', 'Test-LedgerBalance')
    AliasesToExport   = @('ale')
}
