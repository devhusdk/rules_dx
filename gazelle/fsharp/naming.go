package fsharp

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".fs"}

const LibraryKind = "fsharp_library"

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

func IsTestSource(name string) bool {
	base := path.Base(name)
	stem := strings.TrimSuffix(base, ".fs")
	return strings.HasSuffix(stem, "Test")
}

func DirTargetName(dir string) (string, error) {
	return Normalize(path.Base(dir))
}

func TargetName(name string) (string, error) {
	base := path.Base(name)
	stem := strings.TrimSuffix(base, ".fs")
	return Normalize(stem)
}

func ClassIdentity(name string) string {
	base := path.Base(name)
	stem := strings.TrimSuffix(base, ".fs")
	if i := strings.LastIndex(stem, "."); i >= 0 {
		return stem[i+1:]
	}
	return stem
}

type Claimant = common.Claimant

type CollisionError = common.CollisionError

func CheckCollisions(claimants []Claimant) error {
	return common.CheckCollisions(claimants)
}
