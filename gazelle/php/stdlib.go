package php

import (
	_ "embed"
	"strings"
)

//go:embed builtins.txt
var builtinsText string

var builtinSet = func() map[string]struct{} {
	set := make(map[string]struct{})
	for _, line := range strings.Split(builtinsText, "\n") {
		if line = strings.TrimSpace(line); line != "" {
			set[line] = struct{}{}
		}
	}
	return set
}()

func IsStdLib(name string) bool {
	if name == "" || strings.Contains(name, `\`) {
		return false
	}
	_, ok := builtinSet[strings.ToLower(name)]
	return ok
}
