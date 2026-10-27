package csharp

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "csharp"

const LibraryKind = "csharp_library"

var SupportedExts = []string{".cs"}

const unresolvedHint = "add a local library that declares the namespace or a # gazelle:resolve mapping on the full namespace"

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "csharp_binary",
		DefsPackage:    "csharp/rules:defs.bzl",
		UnresolvedHint: unresolvedHint,
		Exts:           SupportedExts,
		TestSuffixes:   []string{"Test"},
		PackageIDs:     true,
		ParsePackage:   ParsePackage,
		DefinesMain:    DefinesMain,
		ParseImports:   ParseImports,
		IsStdLib:       IsStdLib,
		Candidates:     namespaceCandidates,
	})
}
