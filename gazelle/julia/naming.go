package julia

import (
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	LibraryKind  = "julia_library"
	TestKind     = "julia_test"
	SrcDir       = "src"
	TestDir      = "test"
	TestEntry    = "test/runtests.jl"
	languageName = "julia"
)

var ProjectFile = common.ManifestName("julia")

func IsSrc(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, SrcDir+"/") && strings.HasSuffix(src, ".jl")
}

func IsTestSrc(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, TestDir+"/") && strings.HasSuffix(src, ".jl")
}

func isLocal(src string) bool {
	return src != "" && !strings.ContainsAny(src, ":\\") && !strings.HasPrefix(src, "/") && !strings.Contains("/"+src+"/", "/../")
}

func Owns(r *rule.Rule) bool {
	srcs := r.AttrStrings("srcs")
	if len(srcs) == 0 || r.Name() == "" {
		return false
	}
	var ok func(string) bool
	switch r.Kind() {
	case LibraryKind:
		ok = IsSrc
	case TestKind:
		ok = IsTestSrc
	default:
		return false
	}
	for _, src := range srcs {
		if !ok(src) {
			return false
		}
	}
	return true
}
