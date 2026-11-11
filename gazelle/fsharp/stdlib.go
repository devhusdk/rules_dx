package fsharp

import "github.com/devhusdk/rules_dx/gazelle/common"

// benchmarkNamespaces are the BenchmarkDotNet namespaces fsharp_bench supplies.
var benchmarkNamespaces = map[string]bool{
	"BenchmarkDotNet.Attributes": true,
	"BenchmarkDotNet.Running":    true,
}

// IsStdLib reports whether spec is exactly a .NET shared framework,
// FSharp.Core or fsharp_bench BenchmarkDotNet namespace.
func IsStdLib(spec string) bool {
	return common.IsDotnetPlatform(spec) || common.IsFSharpCorePlatform(spec) || benchmarkNamespaces[spec]
}
