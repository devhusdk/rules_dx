package fsharp

import "github.com/devhusdk/rules_dx/gazelle/common"

var benchmarkNamespaces = map[string]bool{
	"BenchmarkDotNet.Attributes": true,
	"BenchmarkDotNet.Running":    true,
}

func IsStdLib(spec string) bool {
	return common.IsDotnetPlatform(spec) || common.IsFSharpCorePlatform(spec) || benchmarkNamespaces[spec]
}
