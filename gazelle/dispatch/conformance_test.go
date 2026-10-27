package dispatch

import (
	"context"
	"flag"
	"os"
	"path/filepath"
	"reflect"
	"slices"
	"sort"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

type conformanceRow struct {
	rel        string
	files      map[string]string
	kind       string
	target     string
	defs       string
	inert      []string
	oldSrcs    []string
	symbols    []string
	extraLoads map[string][]string
}

const webScript = "export const x = 1;\n"

var conformanceRows = map[string]conformanceRow{
	"astro": {
		files: map[string]string{"demo.astro": "---\nconst title = \"hello\";\n---\n\n<script>\n" + webScript + "</script>\n\n<div class=\"hello\">hello</div>\n"},
		kind:  "astro_library", target: "demo", defs: "astro/rules:defs.bzl",
		symbols: []string{"astro_library"},
	},
	"cc": {
		files: map[string]string{"demo.cc": "#include <string>\n"},
		kind:  "cc_library", target: "demo", defs: "cc/rules:defs.bzl",
		symbols:    []string{"cc_library"},
		extraLoads: map[string][]string{"@rules_dx//cuda/rules:defs.bzl": {"cuda_library"}},
	},
	"csharp": {
		files: map[string]string{"Demo.cs": "namespace Demo;\n\nclass Demo\n"},
		kind:  "csharp_library", target: "demo", defs: "csharp/rules:defs.bzl",
		symbols: []string{"csharp_library"},
	},
	"elixir": {
		files: map[string]string{
			"mix.exs":     "defmodule Demo.MixProject do\n  def project, do: [app: :demo, version: \"1.0.0\"]\nend\n",
			"lib/demo.ex": "defmodule Demo do\n  def f, do: true\nend\n",
		},
		kind: "elixir_app", target: "demo", defs: "elixir/rules:defs.bzl",
		symbols: []string{"elixir_app", "ex_unit_test"},
		oldSrcs: []string{"lib/old.ex"},
	},
	"fsharp": {
		files: map[string]string{"Demo.fs": "namespace Demo\n\ntype Demo = class end\n"},
		kind:  "fsharp_library", target: "demo", defs: "fsharp/rules:defs.bzl",
		symbols: []string{"fsharp_library"},
	},
	"gleam": {
		files: map[string]string{
			"gleam.toml":     "name = \"demo\"\n",
			"src/demo.gleam": "import gleam/io\n",
		},
		kind: "gleam_library", target: "demo", defs: "gleam/rules:defs.bzl",
		symbols: []string{"gleam_library", "gleam_test", "gleam_test_library"},
	},
	"go": {
		files: map[string]string{"demo.go": "package demo\n"},
		kind:  "go_library", target: "demo", defs: "go/rules:defs.bzl",
		symbols: []string{"go_library", "go_test"},
	},
	"java": {
		files: map[string]string{"Demo.java": "package demo;\n\npublic class Demo {}\n"},
		kind:  "java_library", target: "demo", defs: "java/rules:defs.bzl",
		symbols: []string{"java_library"},
	},
	"javascript": {
		files: map[string]string{"demo.js": "export const x = 1;\n"},
		kind:  "javascript_library", target: "demo", defs: "javascript/rules:defs.bzl",
		symbols: []string{"javascript_library", "javascript_test", "javascript_binary"},
		inert:   []string{"notes.txt", "widget.d.ts"},
	},
	"kotlin": {
		files: map[string]string{"Demo.kt": "package demo\n\nclass Demo\n"},
		kind:  "kotlin_library", target: "demo", defs: "kotlin/rules:defs.bzl",
		symbols: []string{"kotlin_library"},
	},
	"mdx": {
		files: map[string]string{"demo.mdx": webScript + "\n# Demo\n\nContent.\n"},
		kind:  "mdx_library", target: "demo", defs: "mdx/rules:defs.bzl",
		symbols: []string{"mdx_library"},
	},
	"powershell": {
		files: map[string]string{"Demo.psm1": "function Get-Greeting { 'hi' }\n"},
		kind:  "pwsh_library", target: "demo", defs: "powershell/rules:defs.bzl",
		symbols: []string{"pwsh_library", "pwsh_test"},
		oldSrcs: []string{"Old.psm1"},
	},
	"python": {
		files: map[string]string{"demo.py": "x = 1\n"},
		kind:  "python_library", target: "demo", defs: "python/rules:defs.bzl",
		symbols: []string{"python_library", "python_test", "python_binary"},
		inert:   []string{"notes.txt", "orphan.pyi"},
	},
	"ruby": {
		files: map[string]string{"demo.rb": "module Demo\nend\n"},
		kind:  "ruby_library", target: "demo", defs: "ruby/rules:defs.bzl",
		symbols: []string{"ruby_library"},
	},
	"rust": {
		rel:   "crates/demo",
		files: map[string]string{"src/lib.rs": "pub fn demo() {}\n"},
		kind:  "rust_library", target: "demo", defs: "rust/rules:defs.bzl",
		symbols: []string{"rust_binary", "rust_library", "rust_test", "rust_proc_macro", "rust_shared_library", "rust_static_library"},
		extraLoads: map[string][]string{
			"@rules_dx//rust/rules:crate.bzl":      {"dx_rust_crate"},
			"@rules_rust//cargo:defs.bzl":          {"cargo_build_script"},
			"@crates//:crates.bzl":                 {"aliases", "all_crate_deps", "crate_deps"},
			"@rules_dx//quality:native_config.bzl": {"buildifier_config", "taplo_config", "vale_config", "rustfmt_config"},
			"@rules_dx//quality:fixtures.bzl":      {"real_source_target"},
		},
	},
	"scala": {
		files: map[string]string{"Demo.scala": "package demo\n\nclass Demo\n"},
		kind:  "scala_library", target: "demo", defs: "scala/rules:defs.bzl",
		symbols: []string{"scala_library"},
	},
	"svelte": {
		files: map[string]string{"demo.svelte": "<script>\n" + webScript + "</script>\n\n<div>hello</div>\n"},
		kind:  "svelte_library", target: "demo", defs: "svelte/rules:defs.bzl",
		symbols: []string{"svelte_library"},
	},
	"typescript": {
		files: map[string]string{"demo.ts": "export const x = 1;\n"},
		kind:  "typescript_project", target: "demo", defs: "typescript/rules:defs.bzl",
		symbols:    []string{"typescript_project", "typescript_test"},
		extraLoads: map[string][]string{"@rules_dx//javascript/rules:defs.bzl": {"javascript_binary"}},
		inert:      []string{"notes.txt", "widget.d.ts", "orphan.d.mts"},
	},
	"vue": {
		files: map[string]string{"demo.vue": "<template>\n  <div>hello</div>\n</template>\n\n<script>\nexport default {};\n</script>\n"},
		kind:  "vue_library", target: "demo", defs: "vue/rules:defs.bzl",
		symbols: []string{"vue_library"},
	},
	"zig": {
		files: map[string]string{"demo.zig": "const std = @import(\"std\");\n"},
		kind:  "zig_library", target: "demo", defs: "zig/rules:defs.bzl",
		symbols: []string{"zig_library"},
	},
}

func (r conformanceRow) relPath() string {
	if r.rel == "" {
		return "pkg/demo"
	}
	return r.rel
}

func (r conformanceRow) inertFiles() []string {
	if r.inert == nil {
		return []string{"notes.txt"}
	}
	return r.inert
}

func composedByName() map[string]func() language.Language {
	out := map[string]func() language.Language{}
	for i, lang := range composedLanguages() {
		out[lang.Name()] = func() language.Language { return composedLanguages()[i] }
	}
	return out
}

func TestConformanceRowsMatchComposedLanguages(t *testing.T) {
	composed := composedByName()
	for _, name := range slices.Sorted(mapKeys(composed)) {
		if _, ok := conformanceRows[name]; !ok {
			t.Errorf("%s is composed into dispatch but has no conformance row", name)
		}
	}
	for _, name := range slices.Sorted(mapKeys(conformanceRows)) {
		if _, ok := composed[name]; !ok {
			t.Errorf("%s has a conformance row but is not composed into dispatch", name)
		}
	}
}

func mapKeys[V any](m map[string]V) func(func(string) bool) {
	return func(yield func(string) bool) {
		for key := range m {
			if !yield(key) {
				return
			}
		}
	}
}

func TestLanguageConformance(t *testing.T) {
	composed := composedByName()
	for _, name := range slices.Sorted(mapKeys(conformanceRows)) {
		newLang, ok := composed[name]
		if !ok {
			continue
		}
		row := conformanceRows[name]
		t.Run(name, func(t *testing.T) {
			t.Run("metadata", func(t *testing.T) { checkMetadata(t, name, row, newLang()) })
			t.Run("inert", func(t *testing.T) { checkInert(t, row, newLang()) })
			t.Run("stale", func(t *testing.T) { checkStaleSweep(t, row, newLang()) })
			t.Run("kind_mismatch", func(t *testing.T) { checkKindMismatch(t, name, row, newLang()) })
			t.Run("ignore_naming", func(t *testing.T) { checkIgnoreNaming(t, name, newLang()) })
			t.Run("stale_ignore", func(t *testing.T) { checkStaleIgnore(t, name, row, newLang()) })
		})
	}
}

func lifecycle(t *testing.T, lang language.Language) language.LifecycleManager {
	t.Helper()
	lm, ok := lang.(language.LifecycleManager)
	if !ok {
		t.Fatalf("%s does not implement language.LifecycleManager", lang.Name())
	}
	return lm
}

func freshConfig(t *testing.T) *config.Config {
	t.Helper()
	c := config.New()
	c.RepoRoot = t.TempDir()
	return c
}

func checkMetadata(t *testing.T, name string, row conformanceRow, lang language.Language) {
	if lang.Name() != name {
		t.Errorf("Name() = %q, want %q", lang.Name(), name)
	}
	if _, ok := lang.Kinds()[row.kind]; !ok {
		t.Errorf("Kinds() = %v, want %s among them", lang.Kinds(), row.kind)
	}
	if !slices.Contains(lang.KnownDirectives(), "dx_ignore_import") {
		t.Errorf("KnownDirectives() = %v, want dx_ignore_import", lang.KnownDirectives())
	}
	lm := lifecycle(t, lang)
	lm.Before(context.Background())
	c := freshConfig(t)
	lang.RegisterFlags(flag.NewFlagSet(name, flag.ContinueOnError), "update", c)
	if err := lang.CheckFlags(flag.NewFlagSet(name, flag.ContinueOnError), c); err != nil {
		t.Errorf("CheckFlags() = %v, want nil", err)
	}
	lang.Configure(c, "pkg", nil)
	if done, ok := lang.(language.FinishableLanguage); ok {
		done.DoneGeneratingRules()
	}
	if got := lang.Embeds(nil, label.NoLabel); got != nil {
		t.Errorf("Embeds() = %v, want nil", got)
	}
	lm.AfterResolvingDeps(context.Background())
	if got := common.Errors.Take(); got != nil {
		t.Errorf("Configure without a BUILD file recorded %#v, want none", got)
	}
	loads := lang.Loads()
	wantLoads := map[string][]string{"@rules_dx//" + row.defs: row.symbols}
	for name, symbols := range row.extraLoads {
		wantLoads[name] = symbols
	}
	gotLoads := map[string][]string{}
	for _, load := range loads {
		gotLoads[load.Name] = load.Symbols
	}
	if !reflect.DeepEqual(gotLoads, wantLoads) || !slices.Contains(row.symbols, row.kind) {
		t.Errorf("Loads() = %+v, want %v with %s among them", loads, wantLoads, row.kind)
	}
	aware, ok := lang.(language.ModuleAwareLanguage)
	if !ok {
		t.Fatalf("%s does not implement language.ModuleAwareLanguage", name)
	}
	if got := aware.ApparentLoads(func(string) string { return "" }); !reflect.DeepEqual(got, loads) {
		t.Errorf("ApparentLoads(identity) = %+v, want Loads() %+v", got, loads)
	}
	renamed := aware.ApparentLoads(func(module string) string {
		if module == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	wantRenamed := make([]rule.LoadInfo, len(loads))
	for i, load := range loads {
		wantRenamed[i] = load
		if rest, ok := strings.CutPrefix(load.Name, "@rules_dx//"); ok {
			wantRenamed[i].Name = "@renamed_dx//" + rest
		}
	}
	if !reflect.DeepEqual(renamed, wantRenamed) {
		t.Errorf("ApparentLoads(renamed_dx) = %+v, want %+v", renamed, wantRenamed)
	}
}

func writeRowFiles(t *testing.T, root, rel string, files map[string]string, inert []string) (string, []string) {
	t.Helper()
	dir := filepath.Join(root, filepath.FromSlash(rel))
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	var regular []string
	write := func(name, content string) {
		path := filepath.Join(dir, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(name, "/") {
			regular = append(regular, name)
		}
	}
	for name, content := range files {
		write(name, content)
	}
	for _, name := range inert {
		write(name, "not a source\n")
	}
	sort.Strings(regular)
	return dir, regular
}

func generateRow(t *testing.T, lang language.Language, row conformanceRow, files map[string]string, f *rule.File) language.GenerateResult {
	t.Helper()
	c := freshConfig(t)
	lang.Configure(c, row.relPath(), f)
	dir, regular := writeRowFiles(t, c.RepoRoot, row.relPath(), files, row.inertFiles())
	return lang.GenerateRules(language.GenerateArgs{
		Config:       c,
		Dir:          dir,
		Rel:          row.relPath(),
		File:         f,
		RegularFiles: regular,
	})
}

func staleFile(row conformanceRow, directives ...rule.Directive) *rule.File {
	f := rule.EmptyFile("BUILD.bazel", row.relPath())
	old := rule.NewRule(row.kind, "old")
	if row.oldSrcs != nil {
		old.SetAttr("srcs", row.oldSrcs)
	}
	f.Rules = append(f.Rules,
		old,
		rule.NewRule("filegroup", "keep"),
		rule.NewRule(row.kind, row.target),
	)
	f.Directives = directives
	return f
}

func checkInert(t *testing.T, row conformanceRow, lang language.Language) {
	lm := lifecycle(t, lang)
	lm.Before(context.Background())
	result := generateRow(t, lang, row, nil, nil)
	if len(result.Gen) != 0 || len(result.Empty) != 0 {
		t.Errorf("inert files %v generated %d rules and %d empty rules, want none", row.inertFiles(), len(result.Gen), len(result.Empty))
	}
	lm.AfterResolvingDeps(context.Background())
	if got := common.Errors.Take(); got != nil {
		t.Errorf("inert files recorded %#v, want none", got)
	}
}

func checkStaleSweep(t *testing.T, row conformanceRow, lang language.Language) {
	lm := lifecycle(t, lang)
	lm.Before(context.Background())
	result := generateRow(t, lang, row, row.files, staleFile(row))
	if len(result.Gen) == 0 {
		t.Errorf("generated no rules from %v", slices.Sorted(mapKeys(row.files)))
	}
	var empty []string
	for _, r := range result.Empty {
		empty = append(empty, r.Name())
	}
	if len(empty) == 0 || slices.ContainsFunc(empty, func(name string) bool { return name != "old" }) {
		t.Errorf("empty rules = %v, want only old", empty)
	}
	lm.AfterResolvingDeps(context.Background())
	if got := common.Errors.Take(); got != nil {
		t.Errorf("stale sweep recorded %#v, want none", got)
	}
}

func checkKindMismatch(t *testing.T, name string, row conformanceRow, lang language.Language) {
	lm := lifecycle(t, lang)
	lm.Before(context.Background())
	f := rule.EmptyFile("BUILD.bazel", row.relPath())
	f.Rules = append(f.Rules, rule.NewRule("filegroup", row.target))
	result := generateRow(t, lang, row, row.files, f)
	for _, r := range result.Gen {
		if r.Name() == row.target {
			t.Errorf("generated %s(%s) over an existing filegroup, want it refused", r.Kind(), r.Name())
		}
	}
	lm.AfterResolvingDeps(context.Background())
	wantErrorEntry(t, name, name+": "+row.relPath()+": target name", false)
}

func ignoreDirectives(values ...string) *rule.File {
	f := rule.EmptyFile("BUILD.bazel", "app")
	for _, value := range values {
		f.Directives = append(f.Directives, rule.Directive{Key: "dx_ignore_import", Value: value})
	}
	return f
}

func checkIgnoreNaming(t *testing.T, name string, lang language.Language) {
	lm := lifecycle(t, lang)
	lm.Before(context.Background())
	c := freshConfig(t)
	lang.Configure(c, "app", ignoreDirectives(name+" Dropped", name+" "+name+" Second", "other Foreign", ""))
	lm.AfterResolvingDeps(context.Background())
	if got := common.Errors.Take(); got != nil {
		t.Errorf("a foreign or empty ignore recorded %#v, want none", got)
	}
	var values []string
	for _, entry := range common.InheritedIgnores(c, name) {
		values = append(values, entry.Value)
	}
	if !reflect.DeepEqual(values, []string{"Dropped", "Second"}) {
		t.Errorf("ignores = %v, want [Dropped Second]", values)
	}
	lm.Before(context.Background())
	lang.Configure(freshConfig(t), "app", ignoreDirectives(name))
	lm.AfterResolvingDeps(context.Background())
	wantErrorEntry(t, name, name+": //app: malformed # gazelle:dx_ignore_import", false)
}

func checkStaleIgnore(t *testing.T, name string, row conformanceRow, lang language.Language) {
	lm := lifecycle(t, lang)
	lm.Before(context.Background())
	f := staleFile(row, rule.Directive{Key: "dx_ignore_import", Value: name + " Unused"})
	generateRow(t, lang, row, row.files, f)
	lm.AfterResolvingDeps(context.Background())
	wantErrorEntry(t, name, "stale # gazelle:dx_ignore_import", true)
}

func wantErrorEntry(t *testing.T, name, want string, contains bool) {
	t.Helper()
	got := common.Errors.Take()
	for _, entry := range got {
		if entry.Language != name {
			continue
		}
		if contains && strings.Contains(entry.Message, want) || !contains && strings.HasPrefix(entry.Message, want) {
			return
		}
	}
	t.Errorf("errors = %#v, want a %s entry matching %q", got, name, want)
}
