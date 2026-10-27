package java

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "java"

const LibraryKind = "java_library"

var SupportedExts = []string{".java"}

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "java_binary",
		DefsPackage:    "java/rules:defs.bzl",
		UnresolvedHint: "add a local library, a maven_install.json artifact, or a # gazelle:resolve mapping on the qualified import or its package",
		Exts:           SupportedExts,
		TestSuffixes:   []string{"Test"},
		ClassIDs:       true,
		PackageIDs:     true,
		ParsePackage:   ParsePackage,
		DefinesMain:    DefinesMain,
		ParseImports:   ParseImports,
		IsStdLib:       IsStdLib,
		Candidates:     common.JVMCandidates,
		External:       "maven",
	})
}
