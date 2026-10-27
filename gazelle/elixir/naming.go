package elixir

import (
	"path"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

const (
	AppKind      = "elixir_app"
	TestKind     = "ex_unit_test"
	MixFile      = "mix.exs"
	LibDir       = "lib"
	TestDir      = "test"
	TestHelper   = "test_helper.exs"
	languageName = "elixir"
)

// IsLibSource reports whether src is a lib/**/*.ex source of the app.
func IsLibSource(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, LibDir+"/") && strings.HasSuffix(src, ".ex")
}

// IsTestSource reports whether src is a test/**/*_test.exs ExUnit file.
func IsTestSource(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, TestDir+"/") && strings.HasSuffix(src, "_test.exs")
}

func isTestHelper(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, TestDir+"/") && path.Base(src) == TestHelper
}

func isLocal(src string) bool {
	return src != "" && !strings.ContainsAny(src, ":\\") && !strings.HasPrefix(src, "/") && !strings.Contains("/"+src+"/", "/../")
}

// Mix holds the literal project facts dx generate reads from mix.exs.
type Mix struct {
	App      string
	Version  string
	Mod      string
	Umbrella bool
}

// ParseMix reads app:, version: (a literal or a literal @attribute), mod: and
// apps_path: from mix.exs. Comments, docs and strings never match, and
// anything that is not a literal is left empty.
func ParseMix(content []byte) Mix {
	toks := lex(string(content))
	attrs := make(map[string]string)
	for i := 0; i+2 < len(toks); i++ {
		if toks[i].is(tkPunct, "@") && toks[i+1].kind == tkIdent && toks[i+2].kind == tkString && !toks[i+2].interp {
			if _, seen := attrs[toks[i+1].val]; !seen {
				attrs[toks[i+1].val] = toks[i+2].val
			}
		}
	}
	var m Mix
	for i, t := range toks {
		if t.kind != tkKey {
			continue
		}
		next := func(k int) token {
			if i+k < len(toks) {
				return toks[i+k]
			}
			return token{}
		}
		switch t.val {
		case "app":
			if m.App == "" && next(1).kind == tkAtom {
				m.App = next(1).val
			}
		case "version":
			if m.Version != "" {
				continue
			}
			switch {
			case next(1).kind == tkString && !next(1).interp:
				m.Version = next(1).val
			case next(1).is(tkPunct, "@") && next(2).kind == tkIdent:
				m.Version = attrs[next(2).val]
			}
		case "mod":
			if m.Mod == "" && next(1).is(tkPunct, "{") && next(2).kind == tkAlias && !strings.HasPrefix(next(2).val, moduleKeyword) {
				m.Mod = strings.TrimPrefix(next(2).val, "Elixir.")
			}
		case "apps_path":
			m.Umbrella = true
		}
	}
	return m
}

// Owns reports whether generation could have produced r, so a stale r may be
// removed. The generator asks only in directories that hold a mix.exs.
func Owns(r *rule.Rule) bool {
	srcs := r.AttrStrings("srcs")
	if len(srcs) == 0 {
		return false
	}
	switch r.Kind() {
	case AppKind:
		if r.Name() == "" {
			return false
		}
		for _, src := range srcs {
			if !IsLibSource(src) {
				return false
			}
		}
		return true
	case TestKind:
		for _, src := range srcs {
			if !IsTestSource(src) && !isTestHelper(src) {
				return false
			}
		}
		return true
	}
	return false
}
