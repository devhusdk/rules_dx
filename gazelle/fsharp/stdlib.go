package fsharp

import "github.com/devhusdk/rules_dx/gazelle/common"

// IsStdLib reports whether spec is exactly a .NET shared framework or
// FSharp.Core namespace.
func IsStdLib(spec string) bool {
	return common.IsDotnetPlatform(spec) || common.IsFSharpCorePlatform(spec)
}
