package rego

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

const languageName = "rego"

// UnresolvedHint tells users how to settle a data import no rule declares.
const UnresolvedHint = "add a rego_library that declares that package or an exact # gazelle:resolve rego mapping"

var regoKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
	TestKind:    common.TestKindInfo(),
}

func NewLanguage() language.Language {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		Kinds:          regoKinds,
		ExtraKinds:     []string{TestKind},
		DefsPackage:    "rego:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: func(name string) bool { return isSupported(name) && !IsTestSource(name) },
		IsTestSource:   IsTestSource,
		QualifiedIDs:   declaredPackage,
		Candidates:     Candidates,
		UnresolvedHint: UnresolvedHint,
		Generate:       generateRules,
	})
}

func declaredPackage(path string) []string {
	content, err := common.ReadSource(path)
	if err != nil {
		return nil
	}
	if pkg := ParsePackage(content); pkg != "" {
		return []string{pkg}
	}
	return nil
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	return common.GenerateSingleDir(args, kinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return isSupported(name) && !IsTestSource(name) },
		DirName:      DirTargetName,
		ParsePackage: func([]byte) (string, error) { return "", nil },
		DefinesMain:  func([]byte) bool { return false },
		ParseImports: ParseImports,
		Candidates:   Candidates,
		Wrap:         common.WrapImportSet,
		Tests: &common.TestSpec{
			Kind:     TestKind,
			IsSource: func(name string) bool { return isSupported(name) && IsTestSource(name) },
			Name:     func(src string) (string, error) { return common.SnakeName(src, SupportedExts) },
		},
	}, rep)
}

func init() { common.RegisterLanguage(NewLanguage) }
