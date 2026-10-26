package vue

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const (
	languageName = "vue"
	libraryKind  = "vue_library"
)

var vueKinds = map[string]rule.KindInfo{
	libraryKind: common.LibraryKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		DisplayName:    "Vue",
		LibraryKind:    libraryKind,
		Kinds:          vueKinds,
		DefsPackage:    "vue/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ModuleName,
		PathIdentity:   true,
		IsImportSource: isSupported,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping (relative imports use the workspace path without extension)",
		Generate:       generateRules,
	})
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleFile(args, kinds, common.SingleFileSpec{
		LanguageName: languageName,
		LibraryKind:  libraryKind,
		IsSource:     isSupported,
		TargetName:   TargetName,
		ParseImports: ParseImports,
		IsStdLib:     common.IsNodeBuiltin,
		Wrap:         common.WrapImportSet,
	}, rep)
}
