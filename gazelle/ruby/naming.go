package ruby

import (
	"path"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".rb"}

var TestSourceSuffixes = []string{"_spec", "_test"}

const LibraryKind = "ruby_library"

const TestKind = "ruby_test"

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func ClassIdentity(name string) string { return common.ClassIdentity(name, SupportedExts) }

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

func RelativeKey(pkg, src string) string {
	return "//" + strings.TrimSuffix(path.Join(pkg, src), path.Ext(src))
}
