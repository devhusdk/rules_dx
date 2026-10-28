package perl

import (
	"path"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".pm"}

const (
	LibraryKind = "perl_library"
	TestKind    = "perl_test"
	libDir      = "lib"
	testDir     = "t"
	testExt     = ".t"
)

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

// ModuleName returns the module a source is loaded as. The source is joined
// to its package path and everything up to the nearest lib segment is dropped;
// without a lib segment the path is taken relative to the package root.
func ModuleName(pkg, src string) string {
	full := strings.Split(path.Join(pkg, src), "/")
	start := -1
	for i := len(full) - 2; i >= 0; i-- {
		if full[i] == libDir {
			start = i + 1
			break
		}
	}
	rel := path.Clean(src)
	if start >= 0 {
		rel = strings.Join(full[start:], "/")
	}
	rel = strings.TrimSuffix(rel, path.Ext(rel))
	return strings.ReplaceAll(rel, "/", "::")
}

// TestTargetName names the perl_test of a t/*.t source.
func TestTargetName(src string) (string, error) {
	return common.Normalize(strings.TrimSuffix(path.Base(src), testExt) + "_test")
}
