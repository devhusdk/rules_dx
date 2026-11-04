# Greet module for the adopt-powershell consumer.
function Get-AdoptGreeting {
    [OutputType([string])]
    param(
        [string]$Name = "powershell"
    )
    return "hello $Name"
}
