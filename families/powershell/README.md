# PowerShell

PowerShell test fixtures. They live in the `rules_dx_powershell` module, next to
`rules_dx`. The rules, the toolchain, the Gallery repository rule, the lint and
format aspects and the Gazelle language stay in `rules_dx`, because its quality
tools run on PowerShell. A workspace that only uses the PowerShell rules needs
nothing more than `rules_dx`.
