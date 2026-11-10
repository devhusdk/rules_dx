Describe "Get-PesterGreeting" {
    It "greets powershell" {
        Get-PesterGreeting | Should -Be "hello powershell"
    }
    It "greets a name" {
        Get-PesterGreeting -Name "dx" | Should -Be "hello dx"
    }
}
