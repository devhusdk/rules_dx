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

// ModuleName returns the module a package-relative source is loaded as: its
// path after a leading lib segment, with slashes turned into double colons.
func ModuleName(src string) string {
	rel := strings.TrimPrefix(src, libDir+"/")
	rel = strings.TrimSuffix(rel, path.Ext(rel))
	return strings.ReplaceAll(rel, "/", "::")
}

// TestTargetName names the perl_test of a t/*.t source.
func TestTargetName(src string) (string, error) {
	return common.Normalize(strings.TrimSuffix(path.Base(src), testExt) + "_test")
}
