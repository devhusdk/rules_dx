package vue

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedExts = []string{".vue"}

func Normalize(base string) (string, error) {
	return common.Normalize(base)
}

func stripVueExt(base string) (string, bool) {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(base, ext) {
			return strings.TrimSuffix(base, ext), true
		}
	}
	return "", false
}

func TargetName(name string) (string, error) {
	base := path.Base(name)
	stem, ok := stripVueExt(base)
	if !ok {
		return Normalize(base)
	}
	return Normalize(stem)
}

func ModuleName(name string) string {
	base := path.Base(name)
	if stem, ok := stripVueExt(base); ok {
		return stem
	}
	return base
}

type Claimant = common.Claimant

type CollisionError = common.CollisionError

func CheckCollisions(claimants []Claimant) error {
	return common.CheckCollisions(claimants)
}
