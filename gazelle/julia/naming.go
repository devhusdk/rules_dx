package julia

import (
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const (
	LibraryKind  = "julia_library"
	TestKind     = "julia_test"
	ProjectFile  = "Project.toml"
	SrcDir       = "src"
	TestDir      = "test"
	TestEntry    = "test/runtests.jl"
	languageName = "julia"
)

// IsSrc reports whether src is a src/**/*.jl source of the package.
func IsSrc(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, SrcDir+"/") && strings.HasSuffix(src, ".jl")
}

// IsTestSrc reports whether src is a test/**/*.jl source of the package.
func IsTestSrc(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, TestDir+"/") && strings.HasSuffix(src, ".jl")
}

func isLocal(src string) bool {
	return src != "" && !strings.ContainsAny(src, ":\\") && !strings.HasPrefix(src, "/") && !strings.Contains("/"+src+"/", "/../")
}

// Owns reports whether generation could have produced r, so a stale r may be
// removed. Rules with a source outside src/ and test/, such as a handwritten
// julia_binary over main.jl, are never owned.
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
