function Add-Tally {
    <#
    .SYNOPSIS
    Adds one mark to a tally.

    .PARAMETER Name
    The tally to add to.

    .PARAMETER Count
    How many marks to add.

    .EXAMPLE
    Add-Tally -Name votes -Count 2
    #>
    param(
        [string]$Name,
        [int]$Count = 1
    )
    Update-TallyStore -Name $Name -Delta $Count
}

function Update-TallyStore {
    param([string]$Name, [int]$Delta)
    $script:store[$Name] += $Delta
}

function global:Clear-Tally {
    $script:store.Clear()
}

$script:store = @{}
Export-ModuleMember -Function Add-Tally, Clear-Tally
