package scala

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/jvmsyntax"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "scala"

var scalaKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "Scala",
		LibraryKind:    LibraryKind,
		Kinds:          scalaKinds,
		DefsPackage:    "scala/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		QualifiedIDs:   qualifiedIDs,
		Candidates:     jvmsyntax.Candidates,
		Generate:       generateRules,
	})
}

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return isSupported(name) && !IsTestSource(name) },
		DirName:      DirTargetName,
		CheckPackage: true,
		ParseSource:  ParseSource,
		MainRule:     "scala_binary",
		MainPhrase:   "main-bearing sources",
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
	}, rep)
}
