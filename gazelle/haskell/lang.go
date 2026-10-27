package haskell

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "haskell"

var haskellKinds = map[string]rule.KindInfo{
	LibraryKind: {
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type haskellLang struct {
	*common.SingleLang
}

// NewLanguage returns the Haskell generator. It emits one haskell_library per
// directory and resolves imports by module name against sibling libraries.
func NewLanguage() language.Language {
	return &haskellLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          haskellKinds,
		DefsPackage:    "haskell/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       Identity,
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a haskell_library with a matching module or an exact # gazelle:resolve haskell mapping",
		Generate:       generateRules,
	})}
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(file string) bool { return isSupported(file) && !IsTestSource(file) },
		DirName:      DirTargetName,
		ParsePackage: func([]byte) (string, error) { return "", nil },
		DefinesMain:  DefinesMain,
		MainRule:     "haskell_binary",
		MainPhrase:   "main modules",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
	}, rep)
}
