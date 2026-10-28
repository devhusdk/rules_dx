package julia

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestSourceHelpers(t *testing.T) {
	for src, want := range map[string]bool{"src/A.jl": true, "src/x/B.jl": true, "A.jl": false, "src/A.txt": false, "src/../A.jl": false, ":gen": false, "test/a.jl": false} {
		if got := IsSrc(src); got != want {
			t.Errorf("IsSrc(%q) = %v, want %v", src, got, want)
		}
	}
	for src, want := range map[string]bool{"test/runtests.jl": true, "test/x/a.jl": true, "src/A.jl": false, "/test/a.jl": false} {
		if got := IsTestSrc(src); got != want {
			t.Errorf("IsTestSrc(%q) = %v, want %v", src, got, want)
		}
	}
}

func TestOwns(t *testing.T) {
	mk := func(kind, name string, srcs ...string) *rule.Rule {
		r := rule.NewRule(kind, name)
		if srcs != nil {
			r.SetAttr("srcs", srcs)
		}
		return r
	}
	for _, tc := range []struct {
		r    *rule.Rule
		want bool
	}{
		{mk(LibraryKind, "A", "src/A.jl"), true},
		{mk(LibraryKind, "A"), false},
		{mk(LibraryKind, "A", "main.jl"), false},
		{mk(LibraryKind, "A", "test/runtests.jl"), false},
		{mk(TestKind, "A_test", "test/runtests.jl"), true},
		{mk(TestKind, "A_test", "src/A.jl"), false},
		{mk("julia_binary", "bin", "src/A.jl"), false},
	} {
		if got := Owns(tc.r); got != tc.want {
			t.Errorf("Owns(%s %s %v) = %v, want %v", tc.r.Kind(), tc.r.Name(), tc.r.AttrStrings("srcs"), got, tc.want)
		}
	}
}
