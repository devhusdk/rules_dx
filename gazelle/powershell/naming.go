package powershell

import (
	"path"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const (
	LibraryKind  = "powershell_library"
	TestKind     = "powershell_test"
	ModuleExt    = ".psm1"
	ManifestExt  = ".psd1"
	TestSuffix   = ".Tests.ps1"
	languageName = "powershell"
)

func IsTestSource(name string) bool {
	return strings.HasSuffix(name, TestSuffix) && len(path.Base(name)) > len(TestSuffix)
}

func IsModuleSource(name string, present map[string]bool) bool {
	switch {
	case strings.HasSuffix(name, ModuleExt):
		return true
	case strings.HasSuffix(name, ManifestExt):
		return present[strings.TrimSuffix(name, ManifestExt)+ModuleExt]
	}
	return false
}

func ModuleName(src string) string {
	base := src
	if i := strings.LastIndexAny(base, "/:"); i >= 0 {
		base = base[i+1:]
	}
	base = strings.TrimSuffix(base, ModuleExt)
	return strings.TrimSuffix(base, ManifestExt)
}

func TestTargetName(src string) (string, error) {
	stem := strings.TrimSuffix(path.Base(src), TestSuffix)
	name, err := common.Normalize(strings.ToLower(stem))
	if err != nil {
		return "", err
	}
	return name + "_test", nil
}

func isLocalSrc(src string) bool { return src != "" && !strings.ContainsAny(src, "/:\\") }

func Owns(r *rule.Rule) bool {
	srcs := r.AttrStrings("srcs")
	switch r.Kind() {
	case LibraryKind:
		if len(srcs) == 0 {
			return false
		}
		for _, src := range srcs {
			if !isLocalSrc(src) || !(strings.HasSuffix(src, ModuleExt) || strings.HasSuffix(src, ManifestExt)) {
				return false
			}
		}
		return true
	case TestKind:
		return len(srcs) == 1 && isLocalSrc(srcs[0]) && IsTestSource(srcs[0])
	}
	return false
}
