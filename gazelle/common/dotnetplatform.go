package common

import (
	_ "embed"
	"strings"
)

//go:embed dotnet_namespaces.txt
var dotnetNamespacesTxt string

//go:embed fsharp_core_namespaces.txt
var fsharpCoreNamespacesTxt string

var (
	dotnetNamespaces     = parseNamespaceList(dotnetNamespacesTxt)
	fsharpCoreNamespaces = parseNamespaceList(fsharpCoreNamespacesTxt)
)

func parseNamespaceList(text string) map[string]struct{} {
	set := make(map[string]struct{})
	for _, line := range strings.Split(text, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		set[line] = struct{}{}
	}
	return set
}

func IsDotnetPlatform(spec string) bool {
	_, ok := dotnetNamespaces[spec]
	return ok
}

func IsFSharpCorePlatform(spec string) bool {
	_, ok := fsharpCoreNamespaces[spec]
	return ok
}
