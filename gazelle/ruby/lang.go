package ruby

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "ruby"

var rubyKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

type rubyLang struct{ *common.SingleLang }

func NewLanguage() language.Language {
	return &rubyLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          rubyKinds,
		DefsPackage:    "ruby/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ClassIdentity,
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		Generate:       generateRules,
	})}
}

// Imports indexes each library source under its Ruby load path.
func (l *rubyLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	pkg := ""
	if f != nil {
		pkg = f.Pkg
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) || IsTestSource(src) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: LoadPath(pkg, src)})
	}
	return specs
}

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
		MainRule:     "ruby_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
	}, rep)
}
