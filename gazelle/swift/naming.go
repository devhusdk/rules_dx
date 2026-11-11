package swift

import "github.com/devhusdk/rules_dx/gazelle/common"

var SupportedExts = []string{".swift"}

var TestSourceSuffixes = []string{"Tests"}

const (
	LibraryKind = "swift_library"
	TestKind    = "swift_test"
)

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ModuleName(target string) string {
	if target != "" && target[0] >= '0' && target[0] <= '9' {
		return "_" + target
	}
	return target
}
