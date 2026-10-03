package cc

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"path"
	"strings"
)

var SupportedSrcExts = []string{".c", ".cc", ".cpp", ".cxx"}

var SupportedHdrExts = []string{".h", ".hh", ".hpp", ".hxx"}

const LibraryKind = "cc_library"

func IsTestSource(name string) bool {
	base := path.Base(name)
	stem := base
	if i := strings.LastIndexByte(base, '.'); i >= 0 {
		stem = base[:i]
	}
	return strings.HasSuffix(stem, "_test")
}

func IsSource(name string) bool { return common.HasExt(path.Base(name), SupportedSrcExts) }

func IsHeader(name string) bool { return common.HasExt(path.Base(name), SupportedHdrExts) }

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func HeaderIdentity(name string) string { return path.Base(name) }
