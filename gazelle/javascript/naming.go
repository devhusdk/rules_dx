package javascript

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".js", ".jsx", ".mjs", ".cjs"}

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

func stripJSExt(base string) (string, bool) {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(base, ext) {
			return strings.TrimSuffix(base, ext), true
		}
	}
	return "", false
}

func IsTestFile(name string) bool {
	stem, ok := stripJSExt(path.Base(name))
	if !ok {
		return false
	}
	return strings.HasSuffix(stem, "_test")
}

func TargetName(name string) (string, error) {
	base := path.Base(name)
	stem, ok := stripJSExt(base)
	if !ok {
		return Normalize(base)
	}
	return Normalize(stem)
}

func ModuleName(name string) string {
	base := path.Base(name)
	if stem, ok := stripJSExt(base); ok {
		return stem
	}
	return base
}

func IsEntryFile(name string) bool {
	if IsTestFile(name) {
		return false
	}
	base := path.Base(name)
	for _, ext := range SupportedExts {
		if base == "main"+ext {
			return true
		}
	}
	return false
}

func EntryBinaryName(lib string) string {
	return lib + "_bin"
}

type Claimant = common.Claimant

type CollisionError = common.CollisionError

func CheckCollisions(claimants []Claimant) error {
	return common.CheckCollisions(claimants)
}
