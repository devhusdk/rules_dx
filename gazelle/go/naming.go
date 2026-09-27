package golang

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".go"}

const LibraryKind = "go_library"

const TestKind = "go_test"

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

func IsTestSource(name string) bool {
	base := path.Base(name)
	return strings.HasSuffix(base, "_test.go")
}

func DirTargetName(dir string) (string, error) {
	return Normalize(path.Base(dir))
}

func TargetName(name string) (string, error) {
	base := path.Base(name)
	stem := strings.TrimSuffix(base, ".go")
	return Normalize(stem)
}

func ModuleName(name string) string {
	base := path.Base(name)
	return strings.TrimSuffix(base, ".go")
}

type Claimant = common.Claimant

type CollisionError = common.CollisionError

func CheckCollisions(claimants []Claimant) error {
	return common.CheckCollisions(claimants)
}
