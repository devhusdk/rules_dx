package java

import (
	"github.com/ralvik/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "java"

var javaKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "Java",
		LibraryKind:    LibraryKind,
		Kinds:          javaKinds,
		DefsPackage:    "java/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ClassIdentity,
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
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
		ParsePackage: ParsePackage,
		DefinesMain:  DefinesMain,
		MainRule:     "java_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
	}, rep)
}
