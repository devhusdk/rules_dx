package powershell

import "strings"

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

func IsStdLib(name string) bool { return stdlibModules[strings.ToLower(name)] }
