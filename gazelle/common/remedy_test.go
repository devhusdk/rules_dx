package common

import (
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

func TestBuildFilePath(t *testing.T) {
	cases := []struct {
		name string
		c    *config.Config
		pkg  string
		want string
	}{
		{name: "package", c: &config.Config{ValidBuildFileNames: []string{"BUILD.bazel", "BUILD"}}, pkg: "app", want: "app/BUILD.bazel"},
		{name: "root", c: &config.Config{ValidBuildFileNames: []string{"BUILD.bazel"}}, pkg: "", want: "BUILD.bazel"},
		{name: "custom name", c: &config.Config{ValidBuildFileNames: []string{"BUILD"}}, pkg: "app", want: "app/BUILD"},
		{name: "nil config", c: nil, pkg: "app", want: "app/BUILD.bazel"},
		{name: "empty config", c: &config.Config{}, pkg: "app", want: "app/BUILD.bazel"},
	}
	for _, tc := range cases {
		if got := BuildFilePath(tc.c, tc.pkg); got != tc.want {
			t.Errorf("%s: BuildFilePath = %q, want %q", tc.name, got, tc.want)
		}
	}
}

func TestUnresolvedImportNamesIgnoreDirective(t *testing.T) {
	cases := []struct {
		name string
		c    *config.Config
		from label.Label
		want string
	}{
		{name: "package", c: &config.Config{ValidBuildFileNames: []string{"BUILD.bazel"}}, from: label.New("", "app", "app"),
			want: `py: //app: unresolved import "x"; add a library; or add "# gazelle:dx_ignore_import py x" to app/BUILD.bazel`},
		{name: "root", c: &config.Config{}, from: label.New("", "", "root"),
			want: `py: //:root: unresolved import "x"; add a library; or add "# gazelle:dx_ignore_import py x" to BUILD.bazel`},
		{name: "custom name", c: &config.Config{ValidBuildFileNames: []string{"BUILD"}}, from: label.New("", "app", "app"),
			want: `py: //app: unresolved import "x"; add a library; or add "# gazelle:dx_ignore_import py x" to app/BUILD`},
		{name: "nil config", c: nil, from: label.New("", "app", "app"),
			want: `py: //app: unresolved import "x"; add a library; or add "# gazelle:dx_ignore_import py x" to app/BUILD.bazel`},
	}
	for _, tc := range cases {
		if got := UnresolvedImport(tc.c, "py", tc.from, "x", "add a library"); got != tc.want {
			t.Errorf("%s: UnresolvedImport =\n%s\nwant\n%s", tc.name, got, tc.want)
		}
	}
}

func TestAmbiguousImportNamesResolveDirective(t *testing.T) {
	cases := []struct {
		name string
		c    *config.Config
		from label.Label
		want string
	}{
		{name: "package", c: &config.Config{ValidBuildFileNames: []string{"BUILD.bazel"}}, from: label.New("", "app", "app"),
			want: `py: //app: ambiguous import "x" resolves to //a:x, //b:x; add "# gazelle:resolve py x <label>" to app/BUILD.bazel`},
		{name: "root", c: &config.Config{}, from: label.New("", "", "root"),
			want: `py: //:root: ambiguous import "x" resolves to //a:x, //b:x; add "# gazelle:resolve py x <label>" to BUILD.bazel`},
		{name: "custom name", c: &config.Config{ValidBuildFileNames: []string{"BUILD"}}, from: label.New("", "app", "app"),
			want: `py: //app: ambiguous import "x" resolves to //a:x, //b:x; add "# gazelle:resolve py x <label>" to app/BUILD`},
		{name: "nil config", c: nil, from: label.New("", "app", "app"),
			want: `py: //app: ambiguous import "x" resolves to //a:x, //b:x; add "# gazelle:resolve py x <label>" to app/BUILD.bazel`},
	}
	for _, tc := range cases {
		if got := AmbiguousImport(tc.c, "py", tc.from, "x", "//a:x, //b:x"); got != tc.want {
			t.Errorf("%s: AmbiguousImport =\n%s\nwant\n%s", tc.name, got, tc.want)
		}
	}
}
