package ruby

import (
	_ "embed"
	"strings"
)

//go:embed stdlib.txt
var stdlibText string

var otherPlatformStdlib = []string{"win32/registry", "win32/resolv", "win32ole", "fiddle/win32"}

var stdlibSet = func() map[string]struct{} {
	set := make(map[string]struct{})
	for _, line := range strings.Split(stdlibText, "\n") {
		if line = strings.TrimSpace(line); line != "" {
			set[line] = struct{}{}
		}
	}
	for _, name := range otherPlatformStdlib {
		set[name] = struct{}{}
	}
	return set
}()

func IsStdLib(spec string) bool {
	_, ok := stdlibSet[spec]
	return ok
}
