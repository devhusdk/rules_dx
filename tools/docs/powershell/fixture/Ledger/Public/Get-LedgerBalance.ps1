function Get-LedgerBalance {
    [CmdletBinding()]
    [OutputType([int])]
    param(
        [Parameter(Mandatory)]
        [object]$Ledger
    )
    ($Ledger.Entries | Measure-Object -Property Amount -Sum).Sum

    <#
    .SYNOPSIS
    Sums every entry of a ledger.

    .DESCRIPTION
    The help for this function sits at the end of its body.

    .PARAMETER Ledger
    The ledger to sum.
    #>
}
