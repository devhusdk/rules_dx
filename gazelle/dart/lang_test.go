package dart

import (
	"reflect"
	"testing"

	"github.com/bazelbuild/bazel-gazelle/language"
)

func TestWrappedKindsLoadFromRulesDx(t *testing.T) {
	lang := NewLanguage().(*dartLang)
	loads := lang.ApparentLoads(func(module string) string {
		if module == "rules_dx" {
			return "my_dx"
		}
		return ""
	})
	if loads[0].Name != "@my_dx//dart/rules:defs.bzl" {
		t.Fatalf("first load = %q", loads[0].Name)
	}
	if !reflect.DeepEqual(loads[0].Symbols, wrappedKinds) {
		t.Fatalf("wrapped symbols = %v", loads[0].Symbols)
	}
	owners := map[string]string{}
	for _, info := range loads {
		for _, symbol := range info.Symbols {
			if prev, ok := owners[symbol]; ok {
				t.Fatalf("%s loads from %s and %s", symbol, prev, info.Name)
			}
			owners[symbol] = info.Name
		}
	}
	for _, kind := range []string{"dart_aggregate_codegen", "dart_sqlcodegen"} {
		if owners[kind] != "@rules_dart//dart:defs.bzl" {
			t.Fatalf("%s loads from %q", kind, owners[kind])
		}
	}
	if owners["json_serializable_library"] != "@rules_dart//dart/ext/json_serializable:defs.bzl" {
		t.Fatalf("json_serializable_library loads from %q", owners["json_serializable_library"])
	}
}

func TestUnmappedRepoFallsBackToRulesDx(t *testing.T) {
	lang := NewLanguage().(language.ModuleAwareLanguage)
	loads := lang.ApparentLoads(func(string) string { return "" })
	if loads[0].Name != "@rules_dx//dart/rules:defs.bzl" {
		t.Fatalf("first load = %q", loads[0].Name)
	}
	plain := NewLanguage().Loads()
	if plain[0].Name != "@rules_dx//dart/rules:defs.bzl" {
		t.Fatalf("Loads first = %q", plain[0].Name)
	}
}

func TestUpstreamBehaviorIsKept(t *testing.T) {
	lang := NewLanguage()
	if lang.Name() != "dart" {
		t.Fatalf("name = %q", lang.Name())
	}
	for _, kind := range wrappedKinds {
		if _, ok := lang.Kinds()[kind]; !ok {
			t.Fatalf("missing kind %s", kind)
		}
	}
	want := []string{"dart_package_name", "dart_pub_deps_repo"}
	for _, directive := range want {
		found := false
		for _, known := range lang.KnownDirectives() {
			found = found || known == directive
		}
		if !found {
			t.Fatalf("missing directive %s", directive)
		}
	}
}
