package typescript

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".ts", ".tsx", ".mts", ".cts"}

var DeclarationExts = []string{".d.ts", ".d.mts", ".d.cts"}

var TestSourceSuffixes = []string{"_test"}

func IsDeclaration(name string) bool {
	return common.HasExt(path.Base(name), DeclarationExts)
}

func IsTestFile(name string) bool {
	return !IsDeclaration(name) && common.HasExt(path.Base(name), SupportedExts) && common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

func TargetName(name string) (string, error) { return common.Normalize(ModuleName(name)) }

func IsEntryFile(name string) bool {
	return !IsTestFile(name) && common.IsEntryFile(name, SupportedExts)
}

func EntryPointName(src string) string {
	base := path.Base(src)
	for _, pair := range [][2]string{{".mts", ".mjs"}, {".cts", ".cjs"}, {".ts", ".js"}, {".tsx", ".js"}} {
		if strings.HasSuffix(base, pair[0]) {
			return strings.TrimSuffix(base, pair[0]) + pair[1]
		}
	}
	return base
}

type Claimant = common.Claimant
