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
	languageName       = "dart"
	LibraryKind        = "dart_library"
	BinaryKind         = "dart_binary"
	TestKind           = "dart_test"
	FlutterLibraryKind = "flutter_library"
	FlutterTestKind    = "flutter_test"
	MetadataKind       = "dart_package_metadata"
	MetadataName       = "pubspec-metadata"
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

var dartKinds = map[string]rule.KindInfo{
	LibraryKind: libraryKind(),
	BinaryKind:  entryKind(),
	TestKind:    entryKind(),

	FlutterLibraryKind: libraryKind(),
	FlutterTestKind:    entryKind(),

	MetadataKind: metadataKind(),
}

type dartLang struct {
	*common.SingleLang
}

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

func (l *dartLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind && r.Kind() != FlutterLibraryKind {
		return nil
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: packageName(r, f)}}
}

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
