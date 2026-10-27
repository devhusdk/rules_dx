package elixir

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func writeTree(t *testing.T, dir string, files map[string]string) []string {
	t.Helper()
	var regular []string
	for name, content := range files {
		p := filepath.Join(dir, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(name, "/") {
			regular = append(regular, name)
		}
	}
	return regular
}

func generate(t *testing.T, lang language.Language, rel string, file *rule.File, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, filepath.FromSlash(rel))
	regular := writeTree(t, dir, files)
	return lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          rel,
		File:         file,
		RegularFiles: regular,
	})
}

func buildFile(t *testing.T, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData("apps/app/BUILD.bazel", "apps/app", []byte(content))
	if err != nil {
		t.Fatal(err)
	}
	return f
}

func errorsText(lang language.Language) string {
	lang.(*elixirLang).AfterResolvingDeps(context.Background())
	return strings.Join(common.Messages(common.Errors.Take()), "\n")
}

var appTree = map[string]string{
	"mix.exs":               "defmodule App.MixProject do\n  def project, do: [app: :app, version: \"1.0.0\"]\nend\n",
	"lib/app.ex":            "defmodule App do\n  def f, do: Shared.g() && App.Inner.h()\n  defmodule Inner do\n    def h, do: true\n  end\nend\n",
	"test/app_test.exs":     "defmodule AppTest do\n  use ExUnit.Case\n  test \"f\", do: assert App.f()\nend\n",
	"test/test_helper.exs":  "ExUnit.start()\n",
	"test/support/case.exs": "defmodule Support do\nend\n",
}

func summary(result language.GenerateResult) string {
	var out []string
	for _, r := range result.Gen {
		out = append(out, r.Kind()+":"+r.Name()+":"+strings.Join(r.AttrStrings("srcs"), "+"))
	}
	return strings.Join(out, ",")
}

func TestGenerateAppAndTest(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, "apps/app", nil, appTree)
	if lang.(*elixirLang).Failed() {
		t.Fatal(errorsText(lang))
	}
	want := "elixir_app:app:lib/app.ex,ex_unit_test:app_test:test/app_test.exs+test/test_helper.exs"
	if got := summary(result); got != want {
		t.Fatalf("rules = %s, want %s", got, want)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "Shared" {
		t.Errorf("app imports = %v, want own modules dropped", got)
	}
	if got := result.Imports[1].(common.ImportSet).Names; strings.Join(got, ",") != "App,ExUnit,ExUnit.Case" {
		t.Errorf("test imports = %v", got)
	}
	if app := result.Gen[0]; app.AttrString("app_name") != "app" || app.AttrString("app_version") != "1.0.0" || app.Attr("app_module") != nil {
		t.Errorf("app attrs = %s %s", app.AttrString("app_name"), app.AttrString("app_version"))
	}
}

func TestGenerateWithoutMixLeavesRulesAlone(t *testing.T) {
	file := buildFile(t, `elixir_app(name = "app", srcs = ["lib/app.ex"], app_name = "app")
`)
	tree := map[string]string{"lib/app.ex": "defmodule App do\nend\n"}
	result := generate(t, NewLanguage(), "apps/app", file, tree)
	if len(result.Gen) != 0 || len(result.Empty) != 0 {
		t.Fatalf("gen = %d empty = %d, want nothing", len(result.Gen), len(result.Empty))
	}
}

func TestGenerateSkipsMixDepsAndBuild(t *testing.T) {
	for _, rel := range []string{"deps/jason", "_build/dev/lib/jason", "apps/app/deps/jason"} {
		root := t.TempDir()
		for _, parent := range []string{"", "apps/app"} {
			writeTree(t, filepath.Join(root, filepath.FromSlash(parent)), map[string]string{"mix.exs": "defmodule P.MixProject do\nend\n"})
		}
		dir := filepath.Join(root, filepath.FromSlash(rel))
		regular := writeTree(t, dir, map[string]string{
			"mix.exs":      "defmodule Jason.MixProject do\n  def project, do: [app: :jason]\nend\n",
			"lib/jason.ex": "defmodule Jason do\n  def f, do: Decimal.new(1)\nend\n",
		})
		lang := NewLanguage()
		result := lang.GenerateRules(language.GenerateArgs{
			Config:       &config.Config{RepoRoot: root},
			Dir:          dir,
			Rel:          rel,
			RegularFiles: regular,
		})
		if len(result.Gen) != 0 || lang.(*elixirLang).Failed() {
			t.Errorf("%s: gen = %s, want nothing", rel, summary(result))
		}
	}
	if inMixOutput(t.TempDir(), "deps/jason") {
		t.Error("deps/ without a sibling mix.exs must still generate")
	}
}

func TestGenerateReusesNamedApp(t *testing.T) {
	file := buildFile(t, `elixir_app(name = "erlang_app", app_name = "app", app_env = "[]")

ex_unit_test(name = "app_test_old", srcs = ["test/app_test.exs"])
`)
	lang := NewLanguage()
	result := generate(t, lang, "apps/app", file, appTree)
	if lang.(*elixirLang).Failed() {
		t.Fatal(errorsText(lang))
	}
	want := "elixir_app:erlang_app:lib/app.ex,ex_unit_test:app_test_old:test/app_test.exs+test/test_helper.exs"
	if got := summary(result); got != want || len(result.Empty) != 0 {
		t.Fatalf("rules = %s (empty %d), want %s", got, len(result.Empty), want)
	}
}

func TestGenerateRemovesOnlyOwnedRules(t *testing.T) {
	file := buildFile(t, `elixir_app(name = "old", srcs = ["lib/old.ex"], app_name = "old")

elixir_app(name = "gen", srcs = [":gen_src"], app_name = "gen")

ex_unit_test(name = "a_test", srcs = ["test/a_test.exs"])

ex_unit_test(name = "b_test", srcs = ["test/b_test.exs", "test/support/x.ex"])
`)
	lang := NewLanguage()
	result := generate(t, lang, "apps/app", file, appTree)
	var empty []string
	for _, r := range result.Empty {
		empty = append(empty, r.Kind()+":"+r.Name())
	}
	if got := strings.Join(empty, ","); got != "elixir_app:old,ex_unit_test:a_test" {
		t.Errorf("removed = %s", got)
	}
}

func TestGenerateFailsOnUnnamedApp(t *testing.T) {
	file := buildFile(t, "elixir_app(app_name = \"app\")\n")
	lang := NewLanguage()
	result := generate(t, lang, "apps/app", file, appTree)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %s, want none", summary(result))
	}
	if msg := errorsText(lang); !strings.Contains(msg, "unnamed elixir_app") || !strings.Contains(msg, `name = "erlang_app"`) {
		t.Errorf("errors = %q", msg)
	}
}

func TestGenerateUmbrellaRootIsEmpty(t *testing.T) {
	tree := map[string]string{
		"mix.exs":         "defmodule U.MixProject do\n  def project, do: [apps_path: \"apps\"]\nend\n",
		"apps/a/mix.exs":  "defmodule A.MixProject do\n  def project, do: [app: :a]\nend\n",
		"apps/a/lib/a.ex": "defmodule A do\nend\n",
	}
	if result := generate(t, NewLanguage(), "", nil, tree); len(result.Gen) != 0 {
		t.Fatalf("generated %s, want none", summary(result))
	}
}

func TestGenerateRequiresLiteralApp(t *testing.T) {
	tree := map[string]string{
		"mix.exs":  "defmodule A.MixProject do\n  def project, do: [app: @app]\nend\n",
		"lib/a.ex": "defmodule A do\nend\n",
	}
	lang := NewLanguage()
	generate(t, lang, "a", nil, tree)
	if msg := errorsText(lang); !strings.Contains(msg, "no literal app: atom") {
		t.Errorf("errors = %q", msg)
	}
}

func TestImports(t *testing.T) {
	dir := t.TempDir()
	writeTree(t, dir, map[string]string{
		"lib/a.ex": "defmodule A do\n  defmodule B do\n  end\nend\n",
		"lib/c.ex": "defmodule A do\nend\ndefmodule C do\nend\n",
	})
	f := rule.EmptyFile(filepath.Join(dir, "BUILD.bazel"), "pkg")
	app := rule.NewRule(AppKind, "pkg")
	app.SetAttr("srcs", []string{"lib/a.ex", "lib/c.ex", ":gen"})
	var got []string
	for _, spec := range NewLanguage().Imports(nil, app, f) {
		got = append(got, spec.Imp)
	}
	if strings.Join(got, ",") != "A,A.B,C" {
		t.Errorf("imports = %v", got)
	}
	unnamed := rule.NewRule(AppKind, "")
	unnamed.SetAttr("srcs", []string{"lib/a.ex"})
	if specs := NewLanguage().Imports(nil, unnamed, f); len(specs) != 0 {
		t.Errorf("unnamed imports = %v, want none", specs)
	}
}

func TestResolvePrefixAndStdlib(t *testing.T) {
	lang := NewLanguage().(*elixirLang)
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	f, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:resolve elixir Jason @jason//:erlang_app\n"))
	if err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "", f)
	lang.Configure(cfg, "", f)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	index.Finish()
	app := rule.NewRule(AppKind, "app")
	lang.Resolve(cfg, index, nil, app, common.WrapImportSet([]string{"GenServer", "Jason.Encoder", "String.Chars"}), label.New("", "app", "app"))
	if got := strings.Join(app.AttrStrings("deps"), ","); got != "@jason//:erlang_app" || lang.Failed() {
		t.Fatalf("deps = %s (failed=%v)", got, lang.Failed())
	}
	lang.Resolve(cfg, index, nil, rule.NewRule(AppKind, "app"), common.WrapImportSet([]string{"String.Utils"}), label.New("", "app", "app"))
	if msg := errorsText(lang); !strings.Contains(msg, `unresolved import "String.Utils"`) || !strings.Contains(msg, UnresolvedHint) || !strings.Contains(msg, `add "# gazelle:dx_ignore_import elixir String.Utils" to app/BUILD.bazel`) {
		t.Errorf("errors = %q", msg)
	}
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*elixirLang)
	loads := lang.Loads()
	if lang.Name() != "elixir" || len(loads) != 1 || loads[0].Name != "@rules_dx//elixir/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "elixir_app,ex_unit_test" {
		t.Errorf("metadata = %s %+v", lang.Name(), loads)
	}
	if !strings.Contains(UnresolvedHint, "# gazelle:resolve elixir Jason @jason//:erlang_app") || strings.Contains(UnresolvedHint, "dx_ignore_import") {
		t.Errorf("hint = %q", UnresolvedHint)
	}
}
