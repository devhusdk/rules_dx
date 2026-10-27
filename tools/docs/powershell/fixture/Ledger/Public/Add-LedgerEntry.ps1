function Add-LedgerEntry {
    <#
    .SYNOPSIS
    Adds one entry to a ledger.

    .DESCRIPTION
    Appends the entry at the end of the ledger and returns it.
    Amounts are in the smallest currency unit.

    .PARAMETER Ledger
    The ledger to append to.

    .PARAMETER Amount
    The amount in the smallest currency unit.

    .PARAMETER Memo
    A free-text note stored with the entry.

    .PARAMETER Transfer
    Records the entry as a transfer between two accounts.

    .PARAMETER Account
    The counter account of a transfer.

    .EXAMPLE
    Add-LedgerEntry -Ledger $ledger -Amount 250

    Adds an entry of 250.

    .OUTPUTS
    The entry that was added.
    #>
    [CmdletBinding(DefaultParameterSetName = 'Entry')]
    [OutputType([pscustomobject])]
    [Alias('ale')]
    param(
        [Parameter(Mandatory, Position = 0, ValueFromPipeline)]
        [object]$Ledger,

        [Parameter(Mandatory, Position = 1)]
        [int]$Amount,

        [Parameter(ParameterSetName = 'Entry')]
        [Alias('Note')]
        [string]$Memo,

        [Parameter(Mandatory, ParameterSetName = 'Transfer')]
        [switch]$Transfer,

        [Parameter(Mandatory, ParameterSetName = 'Transfer')]
        [string]$Account
    )
    process {
        [pscustomobject]@{ Amount = $Amount; Memo = $Memo; Account = $Account }
    }
}
