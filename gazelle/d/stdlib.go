package d

import "strings"

func IsStdLib(module string) bool {
	if module == "object" {
		return true
	}
	root, _, _ := strings.Cut(module, ".")
	switch root {
	case "std", "core", "etc", "ldc":
		return true
	}
	return false
}
