package csharp

import "github.com/devhusdk/rules_dx/gazelle/common"

// IsStdLib reports whether spec is exactly a .NET shared framework namespace.
func IsStdLib(spec string) bool {
	return common.IsDotnetPlatform(spec)
}
