package elixir

import (
	"path"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	AppKind      = "elixir_app"
	TestKind     = "elixir_test"
	LibDir       = "lib"
	TestDir      = "test"
	TestHelper   = "test_helper.exs"
	languageName = "elixir"
)

var MixFile = common.ManifestName("hex")

func IsLibSource(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, LibDir+"/") && strings.HasSuffix(src, ".ex")
}

func IsTestSource(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, TestDir+"/") && strings.HasSuffix(src, "_test.exs")
}

func isTestHelper(src string) bool {
	return isLocal(src) && strings.HasPrefix(src, TestDir+"/") && path.Base(src) == TestHelper
}

func isLocal(src string) bool {
	return src != "" && !strings.ContainsAny(src, ":\\") && !strings.HasPrefix(src, "/") && !strings.Contains("/"+src+"/", "/../")
}

type Mix struct {
	App      string
	Version  string
	Mod      string
	Umbrella bool
}

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
