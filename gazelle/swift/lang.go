package swift

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const languageName = "swift"

var swiftKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
	TestKind:    common.TestKindInfo(),
}

type swiftLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &swiftLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          swiftKinds,
		ExtraKinds:     []string{TestKind},
		DefsPackage:    "swift:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: isSupported,
		IsTestSource:   IsTestSource,
		IsStdLib:       IsStdLib,
		UnresolvedHint: "add a swift_library with that name or module_name, or an exact # gazelle:resolve swift mapping",
		Generate:       generateRules,
	})}
}

// Imports indexes a library under its module_name, or its name without one.
func (l *swiftLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	name := r.AttrString("module_name")
	if name == "" {
		name = r.Name()
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: name}}
}

func isSupported(name string) bool { return common.HasExt(name, SupportedExts) }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource: func(file string) bool {
			return isSupported(file) && !IsTestSource(file) && !IsEntryFile(file)
		},
		DirName:      DirTargetName,
		DefinesMain:  DefinesMain,
		MainRule:     "swift_binary",
		MainPhrase:   "@main sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         common.WrapImportSet,
		Tests: &common.TestSpec{
			Kind:     TestKind,
			IsSource: func(file string) bool { return isSupported(file) && IsTestSource(file) },
			Name:     func(src string) (string, error) { return common.SnakeName(src, SupportedExts) },
		},
	}, rep)
}

func init() { common.RegisterLanguage(NewLanguage) }
