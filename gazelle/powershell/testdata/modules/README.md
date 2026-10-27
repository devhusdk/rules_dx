# PowerShell Module Generation

Each directory with modules gets one `pwsh_library` named after the directory.
Its sources are the `.psm1` files and each `.psd1` manifest that sits next to a
`.psm1` with the same name. A lone data file such as
`PSGallery.requirements.psd1` stays out. Each `Name.Tests.ps1` script gets its
own `pwsh_test` named `name_test`.

Imports come from `Import-Module`, `using module`, `#Requires -Modules` and the
`RequiredModules` key of a manifest. Each import resolves to the
`pwsh_library` that owns a module with that exact name, so names match
case-sensitively. Modules that ship with pwsh need no dependency.

Generation removes only the rules it could have written itself. Rules such as
`pwsh_binary` and tests whose script is not `*.Tests.ps1` stay as written.
