package d

import "strings"

// IsStdLib reports whether module ships with the D compiler: the std, core,
// etc and ldc package trees and the object module.
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
