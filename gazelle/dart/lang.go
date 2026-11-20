package dart

import (
	"strings"

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
	// FlutterLibraryKind is the library kind for a package that uses the Flutter SDK.
	FlutterLibraryKind = "flutter_library"
	// FlutterTestKind is the test kind for a package that uses the Flutter SDK.
	FlutterTestKind = "flutter_test"
	// MetadataKind declares the package's pub name; the library names it in `package`.
	MetadataKind = "dart_package_metadata"
	// MetadataName is the name of the generated MetadataKind rule.
	MetadataName = "pubspec-metadata"
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

func libraryKind() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:     []string{"package"},
		NonEmptyAttrs:  map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{"deps": true, "package": true, "srcs": true},
		ResolveAttrs:   map[string]bool{"deps": true},
	}
}

func metadataKind() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:     []string{"package_name"},
		NonEmptyAttrs:  map[string]bool{"package_name": true},
		MergeableAttrs: map[string]bool{"package_name": true},
	}
}

// The kinds leave data, defines, aspect_hints and tags unmergeable so they stay
// user-owned.
var dartKinds = map[string]rule.KindInfo{
	LibraryKind: libraryKind(),
	BinaryKind: entryKind(),
	TestKind:   entryKind(),

	FlutterLibraryKind: libraryKind(),
	FlutterTestKind:    entryKind(),

	MetadataKind: metadataKind(),
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
		ExtraKinds:     []string{BinaryKind, TestKind, FlutterLibraryKind, FlutterTestKind, MetadataKind},
		KindDefs:       map[string]string{FlutterLibraryKind: "flutter:defs.bzl", FlutterTestKind: "flutter:defs.bzl"},
		DefsPackage:    "dart:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: func(name string) bool { return common.HasExt(name, []string{".dart"}) },
		External:       "pub",
		UnresolvedHint: "declare the package in the pub manifest, run pub get, and point # gazelle:dx_external pub at the pub lock; or add an exact # gazelle:resolve dart mapping",
		Generate:       generateRules,
	})}
}

// Imports indexes a library under its pub package name.
func (l *dartLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind && r.Kind() != FlutterLibraryKind {
		return nil
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: packageName(r, f)}}
}

// packageName is the package_name of the same-file dart_package_metadata the
// library names in `package`, else the library's name.
func packageName(r *rule.Rule, f *rule.File) string {
	if ref := strings.TrimPrefix(r.AttrString("package"), ":"); ref != "" && f != nil {
		for _, m := range f.Rules {
			if m.Kind() == MetadataKind && m.Name() == ref {
				if name := m.AttrString("package_name"); name != "" {
					return name
				}
			}
		}
	}
	return r.Name()
}

func init() { common.RegisterLanguage(NewLanguage) }
