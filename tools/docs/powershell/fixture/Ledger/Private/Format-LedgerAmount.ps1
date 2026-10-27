function Format-LedgerAmount {
    <#
    .SYNOPSIS
    Formats an amount. Private: never documented.
    #>
    param([int]$Amount)
    '{0:N2}' -f ($Amount / 100)
}
