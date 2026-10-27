function Test-LedgerBalance {
    <#
    .SYNOPSIS
    Tells whether a ledger sums to zero.

    .DESCRIPTION
    This file uses CRLF line endings.
    The description has two lines.

    .PARAMETER Ledger
    The ledger to check.
    #>
    [CmdletBinding()]
    [OutputType([bool])]
    param(
        [Parameter(Mandatory)]
        [object]$Ledger,

        [int]$Tolerance
    )
    (Get-LedgerBalance -Ledger $Ledger) -eq 0
}
