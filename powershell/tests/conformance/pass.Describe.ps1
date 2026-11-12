Describe "Get-Greeting" {
    It "passing_case" {
        Get-Greeting | Should -Be "hello world"
    }
}
