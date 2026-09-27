package typescript

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".ts", ".tsx", ".mts", ".cts"}

func IsDeclaration(name string) bool {
	base := path.Base(name)
	return strings.HasSuffix(base, ".d.ts") || strings.HasSuffix(base, ".d.mts") || strings.HasSuffix(base, ".d.cts")
}

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

func stripTSExt(base string) (string, bool) {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(base, ext) {
			return strings.TrimSuffix(base, ext), true
		}
	}
	return "", false
}

func IsTestFile(name string) bool {
	if IsDeclaration(name) {
		return false
	}
	stem, ok := stripTSExt(path.Base(name))
	if !ok {
		return false
	}
	return strings.HasSuffix(stem, "_test")
}

func TargetName(name string) (string, error) {
	base := path.Base(name)
	stem, ok := stripTSExt(base)
	if !ok {
		return Normalize(base)
	}
	return Normalize(stem)
}

func ModuleName(name string) string {
	base := path.Base(name)
	if stem, ok := stripTSExt(base); ok {
		return stem
	}
	return base
}

func IsEntryFile(name string) bool {
	if IsTestFile(name) || IsDeclaration(name) {
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

func EntryPointName(src string) string {
	base := path.Base(src)
	if strings.HasSuffix(base, ".mts") {
		return strings.TrimSuffix(base, ".mts") + ".mjs"
	}
	if strings.HasSuffix(base, ".cts") {
		return strings.TrimSuffix(base, ".cts") + ".cjs"
	}
	for _, ext := range []string{".ts", ".tsx"} {
		if strings.HasSuffix(base, ext) {
			return strings.TrimSuffix(base, ext) + ".js"
		}
	}
	return base
}

type Claimant = common.Claimant

type CollisionError = common.CollisionError

func CheckCollisions(claimants []Claimant) error {
	return common.CheckCollisions(claimants)
}
