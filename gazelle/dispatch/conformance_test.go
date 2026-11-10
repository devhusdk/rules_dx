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
		kind:  "astro_library", target: "demo", defs: "astro:defs.bzl",
		symbols: []string{"astro_library"},
	},
	"cc": {
		files: map[string]string{"demo.cc": "#include <string>\n"},
		kind:  "cc_library", target: "demo", defs: "cc/rules:defs.bzl",
		symbols:    []string{"cc_library", "cc_test"},
		extraLoads: map[string][]string{"@rules_dx_cuda//rules:defs.bzl": {"cuda_library"}},
	},
	"csharp": {
		files: map[string]string{"Demo.cs": "namespace Demo;\n\nclass Demo\n"},
		kind:  "csharp_library", target: "demo", defs: "@rules_dx_dotnet//csharp/rules:defs.bzl",
		symbols: []string{"csharp_library", "csharp_test"},
	},
	"d": {
		files: map[string]string{"demo.d": "module demo;\n"},
		inert: []string{"notes.txt"},
		kind:  "d_library", target: "demo", defs: "d/rules:defs.bzl",
		symbols: []string{"d_library", "d_binary", "d_test"},
	},
	"dart": {
		rel: "pkg/demo",
		files: map[string]string{
			"pubspec.yaml":  "name: demo\n",
			"lib/demo.dart": "int answer() => 42;\n",
		},
		kind: "dart_library", target: "demo", defs: "dart/rules:defs.bzl",
		symbols:    []string{"dart_library", "dart_binary", "dart_test"},
		extraLoads: map[string][]string{"@rules_dx//flutter/rules:defs.bzl": {"flutter_library", "flutter_test"}},
	},
	"elixir": {
		files: map[string]string{
			"mix.exs":     "defmodule Demo.MixProject do\n  def project, do: [app: :demo, version: \"1.0.0\"]\nend\n",
			"lib/demo.ex": "defmodule Demo do\n  def f, do: true\nend\n",
		},
		kind: "elixir_app", target: "demo", defs: "elixir/rules:defs.bzl",
		symbols: []string{"elixir_app", "elixir_test"},
		oldSrcs: []string{"lib/old.ex"},
	},
	"erlang": {
		files: map[string]string{
			"src/demo.app.src": "{application, demo, [{vsn, \"1.0.0\"}]}.\n",
			"src/demo.erl":     "-module(demo).\n-export([f/0]).\nf() -> true.\n",
		},
		kind: "erlang_app", target: "erlang_app", defs: "erlang/rules:defs.bzl",
		symbols: []string{"erlang_app", "erlang_nif"},
		oldSrcs: []string{"src/old.erl"},
	},
	"fsharp": {
		files: map[string]string{"Demo.fs": "namespace Demo\n\ntype Demo = class end\n"},
		kind:  "fsharp_library", target: "demo", defs: "@rules_dx_dotnet//fsharp/rules:defs.bzl",
		symbols: []string{"fsharp_library", "fsharp_test"},
	},
	"go": {
		files: map[string]string{"demo.go": "package demo\n"},
		inert: []string{"notes.txt"},
		kind:  "go_library", target: "demo", defs: "go/rules:defs.bzl",
		symbols: []string{"go_library", "go_binary", "go_test", "go_bench"},
	},
	"haskell": {
		files: map[string]string{"Demo.hs": "module Demo where\n\nimport Data.List (sort)\n"},
		kind:  "haskell_library", target: "demo", defs: "haskell/rules:defs.bzl",
		symbols: []string{"haskell_library"},
	},
	"java": {
		files: map[string]string{"Demo.java": "package demo;\n\npublic class Demo {}\n"},
		kind:  "java_library", target: "demo", defs: "java:defs.bzl",
		symbols:    []string{"java_library", "java_test"},
		extraLoads: map[string][]string{"@rules_dx_android//rules:defs.bzl": {"java_android_library"}},
	},
	"javascript": {
		files: map[string]string{"demo.js": "export const x = 1;\n"},
		kind:  "javascript_library", target: "demo", defs: "javascript:defs.bzl",
		symbols:    []string{"javascript_library", "javascript_test", "javascript_binary"},
		extraLoads: map[string][]string{
			"@npm//:defs.bzl":                {"npm_link_all_packages"},
			"@aspect_rules_js//js:defs.bzl":  {"js_library"},
			"@aspect_rules_js//npm:defs.bzl": {"npm_package"},
		},
		inert: []string{"notes.txt", "widget.d.ts"},
	},
	"julia": {
		files: map[string]string{
			"Project.toml": "name = \"Demo\"\nuuid = \"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa\"\n",
			"src/Demo.jl":  "module Demo\nend\n",
		},
		kind: "julia_library", target: "Demo", defs: "julia/rules:defs.bzl",
		symbols: []string{"julia_library", "julia_test"},
		oldSrcs: []string{"src/Old.jl"},
	},
	"kotlin": {
		files: map[string]string{"Demo.kt": "package demo\n\nclass Demo\n"},
		kind:  "kotlin_library", target: "demo", defs: "kotlin:defs.bzl",
		symbols:    []string{"kotlin_library", "kotlin_test"},
		extraLoads: map[string][]string{"@rules_dx_android//rules:defs.bzl": {"kotlin_android_library"}},
	},
	"lua": {
		files: map[string]string{"demo.lua": "return {}\n"},
		kind:  "lua_library", target: "demo", defs: "lua/rules:defs.bzl",
		symbols: []string{"lua_library", "lua_test"},
		inert:   []string{"demo.txt"},
	},
	"mdx": {
		files: map[string]string{"demo.mdx": webScript + "\n# Demo\n\nContent.\n"},
		kind:  "mdx_library", target: "demo", defs: "mdx:defs.bzl",
		symbols: []string{"mdx_library"},
	},
	"ownership": {
		files: map[string]string{"Dockerfile": "FROM scratch\n", "CMakeLists.txt": "project(demo)\n", "main.bicep": "targetScope = 'subscription'\n", "guide.adoc": "= Guide\n"},
		kind:  "dockerfile_library", target: "dockerfile", defs: "dockerfile/rules:defs.bzl",
		symbols: []string{"dockerfile_library"},
		extraLoads: map[string][]string{
			"@rules_dx//cmake/rules:defs.bzl":    {"cmake_library"},
			"@rules_dx//bicep/rules:defs.bzl":    {"bicep_library"},
			"@rules_dx//asciidoc/rules:defs.bzl": {"asciidoc_library"},
		},
	},
	"perl": {
		files: map[string]string{"Demo.pm": "package Demo;\n1;\n"},
		kind:  "perl_library", target: "demo", defs: "perl/rules:defs.bzl",
		symbols: []string{"perl_library", "perl_test"},
		inert:   []string{"script.pl"},
	},
	"php": {
		files: map[string]string{"Demo.php": "<?php\nclass Demo {}\n"},
		kind:  "php_library", target: "demo", defs: "php/rules:defs.bzl",
		symbols: []string{"php_library", "php_test"},
		inert:   []string{"bin/x.php"},
	},
	"powershell": {
		files: map[string]string{"Demo.psm1": "function Get-Greeting { 'hi' }\n"},
		kind:  "powershell_library", target: "demo", defs: "powershell/rules:defs.bzl",
		symbols: []string{"powershell_library", "powershell_test"},
		oldSrcs: []string{"Old.psm1"},
	},
	"protobuf": {
		files: map[string]string{"demo.proto": "syntax = \"proto3\";\n"},
		kind:  "proto_library", target: "demo_proto", defs: "@protobuf//bazel:proto_library.bzl",
		symbols:    []string{"proto_library"},
		extraLoads: map[string][]string{"@rules_rust_prost//:defs.bzl": {"rust_prost_library"}, "@rules_dx//typescript:defs.bzl": {"typescript_proto_library"}, "@rules_dx_dotnet//csharp/rules:defs.bzl": {"csharp_proto_library"}, "@grpc-java//:java_grpc_library.bzl": {"java_grpc_library"}, "@grpc//bazel:cc_grpc_library.bzl": {"cc_grpc_library"}, "@protobuf//bazel:cc_proto_library.bzl": {"cc_proto_library"}, "@protobuf//bazel:java_proto_library.bzl": {"java_proto_library"}, "@protobuf//bazel:py_proto_library.bzl": {"py_proto_library"}, "@rules_dx//python/rules:defs.bzl": {"python_grpc_library"}, "@rules_go//proto:def.bzl": {"go_proto_library"}},
	},
	"avro": {
		files: map[string]string{"demo.avdl": "protocol Demo {\n}\n"},
		kind:  "avro_library", target: "demo", defs: "avro/rules:defs.bzl",
		symbols: []string{"avro_library"},
		extraLoads: map[string][]string{
			"@rules_dx//avro/rules:codegen.bzl": {"avro_cc_library", "avro_java_library"},
		},
	},
	"capnproto": {
		files: map[string]string{"demo.capnp": "@0xdbb9ad1f14bf0b36;\n"},
		kind:  "capnp_library", target: "demo", defs: "capnproto/rules:defs.bzl",
		symbols: []string{"capnp_library"},
		extraLoads: map[string][]string{
			"@rules_dx//capnproto/rules:codegen.bzl": {"capnp_cc_library", "capnp_java_library"},
		},
	},
	"flatbuffers": {
		files: map[string]string{"demo.fbs": "table Demo {}\n"},
		kind:  "flatbuffers_library", target: "demo", defs: "flatbuffers/rules:defs.bzl",
		symbols: []string{"flatbuffers_library"},
		extraLoads: map[string][]string{
			"@rules_dx//flatbuffers/rules:codegen.bzl": {"flatbuffers_cc_library", "flatbuffers_java_library", "flatbuffers_rust_library"},
		},
	},
	"thrift": {
		files: map[string]string{"demo.thrift": "struct Demo {}\n"},
		kind:  "thrift_library", target: "demo", defs: "thrift/rules:defs.bzl",
		symbols: []string{"thrift_library"},
		extraLoads: map[string][]string{
			"@rules_dx//thrift/rules:codegen.bzl": {"thrift_cc_library", "thrift_java_library", "thrift_rust_library"},
		},
	},
	"python": {
		files: map[string]string{"demo.py": "x = 1\n"},
		kind:  "python_library", target: "demo", defs: "python/rules:defs.bzl",
		symbols: []string{"python_library", "python_test", "python_binary"},
		inert:   []string{"notes.txt", "orphan.pyi"},
		extraLoads: map[string][]string{
			"@rules_dx//quality:native_config.bzl": {"ruff_config"},
		},
	},
	"ruby": {
		files: map[string]string{"demo.rb": "module Demo\nend\n"},
		kind:  "ruby_library", target: "demo", defs: "ruby/rules:defs.bzl",
		symbols: []string{"ruby_library", "ruby_test"},
	},
	"rust": {
		rel:   "crates/demo",
		files: map[string]string{"src/lib.rs": "pub fn demo() {}\n"},
		kind:  "rust_library", target: "demo", defs: "rust/rules:defs.bzl",
		symbols: []string{"rust_binary", "rust_library", "rust_test", "rust_bench", "rust_proc_macro", "rust_shared_library", "rust_static_library"},
		extraLoads: map[string][]string{
			"@rules_rust//cargo:defs.bzl":          {"cargo_build_script"},
			"@crates//:crates.bzl":                 {"aliases", "all_crate_deps"},
			"@rules_dx//quality:fixtures.bzl":      {"real_source_target"},
		},
	},
	"scala": {
		files: map[string]string{"Demo.scala": "package demo\n\nclass Demo\n"},
		kind:  "scala_library", target: "demo", defs: "scala:defs.bzl",
		symbols: []string{"scala_library", "scala_test"},
	},
	"svelte": {
		files: map[string]string{"demo.svelte": "<script>\n" + webScript + "</script>\n\n<div>hello</div>\n"},
		kind:  "svelte_library", target: "demo", defs: "svelte:defs.bzl",
		symbols: []string{"svelte_library"},
	},
	"ocaml": {
		files: map[string]string{"demo.ml": "let x = 1\n"},
		kind:  "ocaml_module", target: "demo", defs: "ocaml/rules:defs.bzl",
		symbols: []string{"ocaml_module"},
	},
	"swift": {
		files: map[string]string{"Demo.swift": "import Foundation\n"},
		kind:  "swift_library", target: "demo", defs: "swift/rules:defs.bzl",
		symbols: []string{"swift_library", "swift_test"},
	},
	"typescript": {
		files: map[string]string{"demo.ts": "export const x = 1;\n", "tsconfig.json": "{}\n"},
		kind:  "typescript_library", target: "demo", defs: "typescript:defs.bzl",
		symbols:    []string{"typescript_library", "typescript_test", "typescript_binary"},
		extraLoads: map[string][]string{
			"@aspect_rules_js//npm:defs.bzl": {"npm_package"},
			"@aspect_rules_ts//ts:defs.bzl":  {"ts_config"},
		},
		inert: []string{"notes.txt", "widget.d.ts", "orphan.d.mts"},
	},
	"vue": {
		files: map[string]string{"demo.vue": "<template>\n  <div>hello</div>\n</template>\n\n<script>\nexport default {};\n</script>\n"},
		kind:  "vue_library", target: "demo", defs: "vue:defs.bzl",
		symbols: []string{"vue_library"},
	},
	"zig": {
		files: map[string]string{"demo.zig": "const std = @import(\"std\");\n"},
		kind:  "zig_library", target: "demo", defs: "zig/rules:defs.bzl",
		symbols: []string{"zig_library", "zig_test"},
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

func TestGazelleLanguagePackagesAreRegistered(t *testing.T) {
	src, err := filepath.EvalSymlinks("lang.go")
	if err != nil {
		t.Fatal(err)
	}
	entries, err := os.ReadDir(filepath.Dir(filepath.Dir(src)))
	if err != nil {
		t.Fatal(err)
	}
	registered := map[string]bool{}
	for _, name := range registeredPackages {
		registered[name] = true
	}
	for _, name := range supportPackages {
		registered[name] = true
	}
	found := map[string]bool{}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		found[entry.Name()] = true
		if !registered[entry.Name()] {
			t.Errorf("gazelle/%s is not in GAZELLE_LANGUAGES or GAZELLE_SUPPORT_PACKAGES in gazelle/languages.bzl", entry.Name())
		}
	}
	for _, name := range registeredPackages {
		if !found[name] {
			t.Errorf("GAZELLE_LANGUAGES lists %s but gazelle/%s does not exist", name, name)
		}
		if _, ok := conformanceRows[name]; !ok {
			t.Errorf("GAZELLE_LANGUAGES lists %s but conformanceRows has no row", name)
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
	defsLabel := "@rules_dx//" + row.defs
	if strings.HasPrefix(row.defs, "@") {
		defsLabel = row.defs
	}
	wantLoads := map[string][]string{defsLabel: row.symbols}
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
	lang.Configure(c, "app", ignoreDirectives(name+" Dropped", name+" Second", "other Foreign", ""))
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
	lm.Before(context.Background())
	lang.Configure(freshConfig(t), "app", ignoreDirectives(name+" "+name+" x"))
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
