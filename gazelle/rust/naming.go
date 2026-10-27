package rust

import (
	"fmt"
	"github.com/devhusdk/rules_dx/gazelle/common"
	"strings"
)

func IntegrationTestName(stem string) string {
	if strings.HasSuffix(stem, "_test") {
		return stem
	}
	return common.UnitTestName(stem)
}

func BuildScriptName(packageName string) (string, error) {
	stem, err := common.Normalize(packageName)
	if err != nil {
		return "", err
	}
	return stem + "_build_script", nil
}

func CrateName(name string) (string, error) {
	if name == "" {
		return "", fmt.Errorf("naming: %q is not a Cargo crate name; use ASCII letters, digits, '-' or '_'", name)
	}
	for i := 0; i < len(name); i++ {
		b := name[i]
		if !(b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' || b >= '0' && b <= '9' || b == '-' || b == '_') {
			return "", fmt.Errorf("naming: %q is not a Cargo crate name; use ASCII letters, digits, '-' or '_'", name)
		}
	}
	return strings.ReplaceAll(name, "-", "_"), nil
}
