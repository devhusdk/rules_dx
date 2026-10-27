package cc

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"path"
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

func HeaderIdentity(name string) string { return path.Base(name) }

func IsCudaSource(name string) bool { return common.HasExt(path.Base(name), SupportedCudaSrcExts) }

func IsCudaHeader(name string) bool { return common.HasExt(path.Base(name), SupportedCudaHdrExts) }
