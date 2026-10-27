package powershell

import "strings"

// stdlibModules ships in $PSHOME/Modules of the pinned pwsh 7.5.4
// (PWSH_VERSION in modules/versions.bzl), linux_x64 and win_x64 together, plus
// the Microsoft.PowerShell.Core snap-in. Pester is a Gallery module, not here.
var stdlibModules = map[string]bool{
	"cimcmdlets":                         true,
	"microsoft.powershell.archive":       true,
	"microsoft.powershell.core":          true,
	"microsoft.powershell.diagnostics":   true,
	"microsoft.powershell.host":          true,
	"microsoft.powershell.management":    true,
	"microsoft.powershell.psresourceget": true,
	"microsoft.powershell.security":      true,
	"microsoft.powershell.utility":       true,
	"microsoft.wsman.management":         true,
	"packagemanagement":                  true,
	"powershellget":                      true,
	"psdiagnostics":                      true,
	"psreadline":                         true,
	"threadjob":                          true,
}

// IsStdLib reports whether name is a module that ships with pwsh. PowerShell
// module names are case-insensitive, so this check is too.
func IsStdLib(name string) bool { return stdlibModules[strings.ToLower(name)] }
