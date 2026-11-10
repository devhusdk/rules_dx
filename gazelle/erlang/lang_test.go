package erlang

import (
	"os"
	"path/filepath"
	"slices"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func TestParseAppSrc(t *testing.T) {
	got := ParseAppSrc([]byte(`%% {vsn, "0.0.0"}
{application, kv_store, [
    {description, "Key-value store."},
    {vsn, "1.2.3"},
    {mod, {kv_store_app, []}}
]}.
`))
	want := AppSrc{Name: "kv_store", Version: "1.2.3", Description: "Key-value store.", Module: "kv_store_app"}
	if got != want {
		t.Fatalf("ParseAppSrc = %+v, want %+v", got, want)
	}
}

func TestParseAppSrcPercentInLiterals(t *testing.T) {
	got := ParseAppSrc([]byte("{application, demo, [\n    {description, \"100% Erlang\"}, % note\n    {vsn, \"1.0\"}\n]}.\n"))
	want := AppSrc{Name: "demo", Version: "1.0", Description: "100% Erlang"}
	if got != want {
		t.Fatalf("ParseAppSrc = %+v, want %+v", got, want)
	}
}

func TestRootAppWithoutRepoName(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "checkout")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(filepath.Join(dir, "src"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "src", "demo.erl"), []byte("-module(demo).\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, RebarConfig), nil, 0o644); err != nil {
		t.Fatal(err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: dir},
		Dir:          dir,
		RegularFiles: []string{RebarConfig},
	})
	if len(result.Gen) != 1 || result.Gen[0].AttrString("app_name") != "lib" {
		t.Fatalf("GenerateRules gave %v, want one app named lib", result.Gen)
	}
}

func TestStdLib(t *testing.T) {
	for mod, app := range map[string]string{"lists": "stdlib", "gen_server": "stdlib", "crypto": "crypto", "ssl": "ssl"} {
		if got, ok := OTPApp(mod); !ok || got != app {
			t.Errorf("OTPApp(%q) = %q, %v, want %q", mod, got, ok, app)
		}
	}
	if IsStdLib("thoas") {
		t.Error("thoas is not an OTP module")
	}
	if !IsOTPApp("ssl") || IsOTPApp("thoas") {
		t.Error("IsOTPApp mismatch")
	}
}

func TestOwns(t *testing.T) {
	mk := func(kind, name string, srcs ...string) *rule.Rule {
		r := rule.NewRule(kind, name)
		r.SetAttr("srcs", srcs)
		return r
	}
	if !Owns(mk(AppKind, "erlang_app", "src/a.erl", "src/a.hrl")) {
		t.Error("generated app should be owned")
	}
	if Owns(mk(AppKind, "erlang_app", "src/a.erl", "other.erl")) {
		t.Error("foreign source should not be owned")
	}
	if Owns(mk(AppKind, "", "src/a.erl")) {
		t.Error("unnamed rule should not be owned")
	}
	if !Owns(mk(NifKind, "n", "c_src/a.c")) {
		t.Error("generated nif should be owned")
	}
}

func TestSymlinkedSrcKeepsErlangApp(t *testing.T) {
	root := t.TempDir()
	shared := filepath.Join(root, "shared", "src")
	pkg := filepath.Join(root, "pkg", "demo")
	for _, dir := range []string{shared, pkg} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(shared, "demo.app.src"), []byte(`{application, demo, [{vsn, "1.0.0"}]}.`), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(shared, "demo.erl"), []byte("-module(demo).\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(shared, filepath.Join(pkg, SrcDir)); err != nil {
		t.Skipf("symlinks unavailable: %v", err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: root},
		Dir:    pkg,
		Rel:    "pkg/demo",
	})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != AppKind {
		t.Fatalf("generated %d rules, want one %s", len(result.Gen), AppKind)
	}
	if got := result.Gen[0].AttrStrings("srcs"); !slices.Equal(got, []string{"src/demo.erl"}) {
		t.Fatalf("srcs = %v, want [src/demo.erl]", got)
	}
}

func TestGenerateAppInGlobCharDir(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "app[1]", "demo")
	for name, content := range map[string]string{
		"src/demo.app.src": "{application, demo, []}.\n",
		"src/demo.erl":     "-module(demo).\n",
	} {
		p := filepath.Join(dir, name)
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: filepath.Join(root, "app[1]")},
		Dir:    dir,
		Rel:    "demo",
	})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != AppKind {
		t.Fatalf("GenerateRules gave %d rules, want one %s", len(result.Gen), AppKind)
	}
}
