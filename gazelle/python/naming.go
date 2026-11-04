package python

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".py", ".pyi"}

var TestSourceExts = []string{".py"}

var TestSourceSuffixes = []string{"_test"}

func IsTestFile(name string) bool {
	base := path.Base(name)
	if !common.HasExt(base, TestSourceExts) {
		return false
	}
	if strings.HasPrefix(base, "test_") && base != "test_.py" {
		return true
	}
	return common.IsTestSource(name, TestSourceExts, TestSourceSuffixes)
}

func IsConftest(name string) bool { return path.Base(name) == "conftest.py" }

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

func TargetName(name string) (string, error) { return common.Normalize(ModuleName(name)) }

func IsEntryFile(name string) bool {
	return !IsTestFile(name) && path.Base(name) == "main.py"
}

type Claimant = common.Claimant
