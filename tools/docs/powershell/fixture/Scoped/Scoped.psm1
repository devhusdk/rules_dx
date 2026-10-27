function script:Get-ScopedValue {
    <#
    .SYNOPSIS
    Reads one scoped value.

    .PARAMETER Name
    The value to read.
    #>
    param([string]$Name)
    $script:values[$Name]
}

function private:Get-ScopedSecret {
    'secret'
}

function global:Set-ScopedGlobal {
    param([string]$Name)
    $script:values[$Name] = $true
}

$script:values = @{}
