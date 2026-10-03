package rust

import (
	"github.com/ralvik/rules_dx/gazelle/common"
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
