package golang

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".go"}

const LibraryKind = "go_library"

const TestKind = "go_test"

func IsTestSource(name string) bool {
	return strings.HasSuffix(path.Base(name), "_test.go")
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ModuleName(name string) string { return common.Stem(name, SupportedExts) }

type Claimant = common.Claimant
