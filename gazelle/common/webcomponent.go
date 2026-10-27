package common

import (
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func NewWebComponentLang(name string, exts []string, parseImports func([]byte, string) []ImportRef) *SingleLang {
	kind := name + "_library"
	isSource := func(n string) bool { return HasExt(n, exts) }
	return NewSingleLang(SingleLangSpec{
		LanguageName:   name,
		LibraryKind:    kind,
		Kinds:          map[string]rule.KindInfo{kind: LibraryKindInfo()},
		DefsPackage:    name + "/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       func(n string) string { return Stem(n, exts) },
		PathIdentity:   true,
		IsImportSource: isSource,
		Fallback:       []string{"typescript", "javascript"},
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping (relative imports use the workspace path without extension and also search TypeScript and JavaScript targets)",
		Generate: func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult {
			return GenerateSingleFile(args, kinds, SingleFileSpec{
				LanguageName: name,
				LibraryKind:  kind,
				IsSource:     isSource,
				TargetName:   func(n string) (string, error) { return Normalize(Stem(n, exts)) },
				ParseImports: parseImports,
				IsStdLib:     IsNodeBuiltin,
				Wrap:         WrapRefImportSet,
			}, rep)
		},
	})
}

func ParseScriptImports(content []byte, dir string) []ImportRef {
	set := RefSet{Dir: dir}
	for _, script := range ExtractScripts(content, TopLevel) {
		ScanEmbedded(script, set.Add)
	}
	return set.Refs()
}
