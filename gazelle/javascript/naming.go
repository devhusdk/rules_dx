package javascript

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
)

var SupportedExts = []string{".js", ".jsx", ".mjs", ".cjs"}

var TestSourceSuffixes = []string{"_test"}

func IsTestFile(name string) bool {
	return common.HasExt(path.Base(name), SupportedExts) && common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

func TargetName(name string) (string, error) { return common.Normalize(ModuleName(name)) }

func IsEntryFile(name string) bool {
	return !IsTestFile(name) && common.IsEntryFile(name, SupportedExts)
}

type Claimant = common.Claimant
