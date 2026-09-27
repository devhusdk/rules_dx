package python

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

func IsTestFile(name string) bool {
	if !strings.HasSuffix(name, ".py") {
		return false
	}
	stem := strings.TrimSuffix(path.Base(name), ".py")
	return strings.HasSuffix(stem, "_test")
}

func TargetName(name string) (string, error) {
	base := path.Base(name)
	if strings.HasSuffix(base, ".py") {
		base = strings.TrimSuffix(base, ".py")
	} else if strings.HasSuffix(base, ".pyi") {
		base = strings.TrimSuffix(base, ".pyi")
	}
	return Normalize(base)
}

func ModuleName(name string) string {
	base := path.Base(name)
	if strings.HasSuffix(base, ".py") {
		return strings.TrimSuffix(base, ".py")
	}
	if strings.HasSuffix(base, ".pyi") {
		return strings.TrimSuffix(base, ".pyi")
	}
	return base
}

func IsEntryFile(name string) bool {
	if IsTestFile(name) {
		return false
	}
	return path.Base(name) == "main.py"
}

func EntryBinaryName(lib string) string {
	return lib + "_bin"
}

type Claimant = common.Claimant

type CollisionError = common.CollisionError

func CheckCollisions(claimants []Claimant) error {
	return common.CheckCollisions(claimants)
}
