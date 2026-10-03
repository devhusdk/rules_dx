package python

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
)

var SupportedExts = []string{".py", ".pyi"}

var TestSourceExts = []string{".py"}

var TestSourceSuffixes = []string{"_test"}

func IsTestFile(name string) bool {
	return common.HasExt(path.Base(name), TestSourceExts) && common.IsTestSource(name, TestSourceExts, TestSourceSuffixes)
}

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

func TargetName(name string) (string, error) { return common.Normalize(ModuleName(name)) }

func IsEntryFile(name string) bool {
	return !IsTestFile(name) && path.Base(name) == "main.py"
}

type Claimant = common.Claimant
