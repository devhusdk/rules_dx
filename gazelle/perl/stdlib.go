package perl

import (
	_ "embed"
	"strings"
)

//go:embed corelist.txt
var corelistText string

var corelistSet = func() map[string]struct{} {
	set := make(map[string]struct{})
	for _, line := range strings.Split(corelistText, "\n") {
		if line = strings.TrimSpace(line); line != "" {
			set[line] = struct{}{}
		}
	}
	return set
}()

func IsStdLib(name string) bool {
	_, ok := corelistSet[name]
	return ok
}
