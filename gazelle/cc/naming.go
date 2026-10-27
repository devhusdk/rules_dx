package cc

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"path"
	"path/filepath"
	"slices"
	"strings"
)

var SupportedSrcExts = []string{".c", ".cc", ".cpp", ".cxx"}

var SupportedHdrExts = []string{".h", ".hh", ".hpp", ".hxx"}

var SupportedCudaSrcExts = []string{".cu"}

var SupportedCudaHdrExts = []string{".cuh"}

const LibraryKind = "cc_library"

const CudaKind = "cuda_library"

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

func HeaderIdentity(name string) string { return path.Clean(name) }

// HeaderBasename returns the bare file name a header is also indexed under.
func HeaderBasename(p string) []string { return []string{filepath.Base(p)} }

// IncludeKeys returns the index keys for a quoted include, package-relative first, then workspace root.
func IncludeKeys(pkg, name string) []string {
	var keys []string
	for _, k := range []string{path.Join(pkg, name), path.Clean(name)} {
		if k == ".." || strings.HasPrefix(k, "../") || slices.Contains(keys, k) {
			continue
		}
		keys = append(keys, k)
	}
	return keys
}

func IsCudaSource(name string) bool { return common.HasExt(path.Base(name), SupportedCudaSrcExts) }

func IsCudaHeader(name string) bool { return common.HasExt(path.Base(name), SupportedCudaHdrExts) }
