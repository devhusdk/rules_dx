package ruby

import (
	"path"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".rb"}

var TestSourceSuffixes = []string{"_spec", "_test"}

const LibraryKind = "ruby_library"

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ClassIdentity(name string) string { return common.ClassIdentity(name, SupportedExts) }

// LoadPath returns the string a Ruby file is required by: its path after the
// last lib segment, or its package-relative path when no lib segment exists.
func LoadPath(pkg, src string) string {
	dir := path.Dir(src)
	stem := common.Stem(path.Base(src), SupportedExts)
	if dir == "." {
		dir = ""
	}
	full := path.Join(pkg, dir)
	segs := strings.Split(full, "/")
	for i := len(segs) - 1; i >= 0; i-- {
		if segs[i] == "lib" {
			return path.Join(strings.Join(segs[i+1:], "/"), stem)
		}
	}
	return path.Join(dir, stem)
}
