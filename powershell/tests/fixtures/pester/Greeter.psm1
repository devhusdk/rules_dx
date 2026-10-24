function Get-PesterGreeting {
    [OutputType([string])]
    param(
        [string]$Name = "powershell"
    )
    return "hello $Name"
}
