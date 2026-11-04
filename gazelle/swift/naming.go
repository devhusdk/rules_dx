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
