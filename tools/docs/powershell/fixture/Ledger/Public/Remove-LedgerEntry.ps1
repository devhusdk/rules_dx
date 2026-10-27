function Remove-LedgerEntry {
    param($Ledger, $Index)
    $Ledger.Entries.RemoveAt($Index)
}
