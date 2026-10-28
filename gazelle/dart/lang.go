package dart

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const (
	languageName = "dart"
	// LibraryKind is the rule kind generated over lib/.
	LibraryKind = "dart_library"
	// BinaryKind is the rule kind generated per bin/*.dart.
	BinaryKind = "dart_binary"
	// TestKind is the rule kind generated per test/**/*_test.dart.
	TestKind = "dart_test"
)

func entryKind() rule.KindInfo {
	merge := map[string]bool{"deps": true, "main": true, "srcs": true}
	return rule.KindInfo{
		MatchAttrs:     []string{"main"},
		NonEmptyAttrs:  map[string]bool{"main": true},
		MergeableAttrs: merge,
		ResolveAttrs:   map[string]bool{"deps": true},
	}
}

// The kinds leave data, defines, aspect_hints and tags unmergeable so they stay
// user-owned.
var dartKinds = map[string]rule.KindInfo{
	LibraryKind: {
		MatchAttrs:     []string{"package_name"},
		NonEmptyAttrs:  map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{"deps": true, "package_name": true, "srcs": true},
		ResolveAttrs:   map[string]bool{"deps": true},
	},
	BinaryKind: entryKind(),
	TestKind:   entryKind(),
}

type dartLang struct {
	*common.SingleLang
}

// NewLanguage returns the Dart language extension.
func NewLanguage() language.Language {
	return &dartLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          dartKinds,
		ExtraKinds:     []string{BinaryKind, TestKind},
		DefsPackage:    "dart/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: func(name string) bool { return common.HasExt(name, []string{".dart"}) },
		External:       "pub",
		UnresolvedHint: "declare the package in pubspec.yaml, run pub get, and point # gazelle:dx_external pub at the pubspec.lock; or add an exact # gazelle:resolve dart mapping",
		Generate:       generateRules,
	})}
}

// Imports indexes a library under its pub package name.
func (l *dartLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	name := r.AttrString("package_name")
	if name == "" {
		name = r.Name()
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: name}}
}
