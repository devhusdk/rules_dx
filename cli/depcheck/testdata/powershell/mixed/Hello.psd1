@{
    RootModule        = 'Hello.psm1'
    ModuleVersion     = '0.1.0'
    # RequiredModules = @('Commented')
    RequiredModules   = @(
        'PSReadLine',
        @{ ModuleName = 'Pester'; ModuleVersion = '5.0.0' },
        @{ ModuleName = 'PSScriptAnalyzer'; RequiredVersion = '1.21.0' }
    )
}
