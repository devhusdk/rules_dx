package erlang

import (
	"path"
	"regexp"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const (
	AppKind      = "erlang_app"
	NifKind      = "erlang_nif"
	AppTarget    = "erlang_app"
	RebarConfig  = "rebar.config"
	SrcDir       = "src"
	IncludeDir   = "include"
	NifDir       = "c_src"
	languageName = "erlang"
)

var atomPattern = regexp.MustCompile(`^[a-z][A-Za-z0-9_@]*$`)

// ModuleName returns the Erlang module an .erl source defines: its base name.
func ModuleName(src string) string {
	return strings.TrimSuffix(path.Base(src), ".erl")
}

// IsErlSource reports whether src is an .erl file.
func IsErlSource(src string) bool { return strings.HasSuffix(src, ".erl") }

func isLocal(src string) bool {
	return src != "" && !strings.ContainsAny(src, ":\\") && !strings.HasPrefix(src, "/") && !strings.Contains("/"+src+"/", "/../")
}

// IsAppSource reports whether src is a src/**/*.erl or src/**/*.hrl file.
func IsAppSource(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, SrcDir+"/") && (strings.HasSuffix(src, ".erl") || strings.HasSuffix(src, ".hrl"))
}

// IsAppHeader reports whether src is an include/**/*.hrl file.
func IsAppHeader(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, IncludeDir+"/") && strings.HasSuffix(src, ".hrl")
}

// IsNifSource reports whether src is a C or C++ source under c_src.
func IsNifSource(src string) bool {
	if !isLocal(src) || !strings.HasPrefix(src, NifDir+"/") {
		return false
	}
	switch path.Ext(src) {
	case ".c", ".cc", ".cpp":
		return true
	}
	return false
}

// IsNifHeader reports whether src is a C or C++ header under c_src.
func IsNifHeader(src string) bool {
	if !isLocal(src) || !strings.HasPrefix(src, NifDir+"/") {
		return false
	}
	switch path.Ext(src) {
	case ".h", ".hpp":
		return true
	}
	return false
}

// Owns reports whether generation could have produced r, so a stale r may be
// removed. The generator asks only in directories that hold an Erlang app.
func Owns(r *rule.Rule) bool {
	if r.Name() == "" {
		return false
	}
	srcs := r.AttrStrings("srcs")
	if len(srcs) == 0 {
		return false
	}
	switch r.Kind() {
	case AppKind:
		for _, src := range srcs {
			if !IsAppSource(src) {
				return false
			}
		}
		return true
	case NifKind:
		for _, src := range srcs {
			if !IsNifSource(src) {
				return false
			}
		}
		return true
	}
	return false
}
