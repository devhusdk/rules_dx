package common

import (
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

// ClassLangSpec describes a one-library-per-directory language named by class.
type ClassLangSpec struct {
	LanguageName string
	LibraryKind  string
	MainRule     string
	// Variant swaps LibraryKind for a platform kind; see KindVariant.
	Variant        *KindVariant
	DefsPackage    string
	UnresolvedHint string
	Exts           []string
	TestSuffixes   []string
	// ClassIDs indexes each source under its bare class identity.
	ClassIDs bool
	// PackageIDs indexes each source under its package and its package-qualified class identity.
	PackageIDs   bool
	ParsePackage func(content []byte) (string, error)
	DefinesMain  func(content []byte) bool
	ParseImports func(content []byte) []string
	IsStdLib     func(name string) bool
	Candidates   func(name string) []string
	OrderSrcs    func(name string, sources []string, contents map[string][]byte) ([]string, error)
	MakeRule     func(name string, sources []string, headers []string) *rule.Rule
	// External is the dx_external ecosystem third-party imports resolve through.
	External string
}

// Spec returns the SingleLangSpec the class language runs on.
func (s ClassLangSpec) Spec() SingleLangSpec {
	isSource := func(name string) bool { return HasExt(name, s.Exts) }
	isTest := func(name string) bool { return IsTestSource(name, s.Exts, s.TestSuffixes) }
	identity := func(name string) string { return ClassIdentity(name, s.Exts) }
	spec := SingleLangSpec{
		LanguageName:   s.LanguageName,
		LibraryKind:    s.LibraryKind,
		Kinds:          map[string]rule.KindInfo{s.LibraryKind: LibraryKindInfo()},
		DefsPackage:    s.DefsPackage,
		ImportAttr:     "srcs",
		IsImportSource: isSource,
		IsTestSource:   isTest,
		IsStdLib:       s.IsStdLib,
		UnresolvedHint: s.UnresolvedHint,
		Candidates:     s.Candidates,
		External:       s.External,
		Generate: func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult {
			return GenerateSingleDir(args, kinds, SingleDirSpec{
				LanguageName: s.LanguageName,
				LibraryKind:  s.LibraryKind,
				IsSource:     func(name string) bool { return isSource(name) && !isTest(name) },
				DirName:      DirTargetName,
				CheckPackage: true,
				ParsePackage: s.ParsePackage,
				DefinesMain:  s.DefinesMain,
				MainRule:     s.MainRule,
				MainPhrase:   "main-bearing sources",
				ParseImports: s.ParseImports,
				IsStdLib:     s.IsStdLib,
				Candidates:   s.Candidates,
				OrderSrcs:    s.OrderSrcs,
				MakeRule:     s.MakeRule,
				Variant:      s.Variant,
				Wrap:         WrapImportSet,
			}, rep)
		},
	}
	if v := s.Variant; v != nil {
		spec.Kinds[v.Kind] = LibraryKindInfo()
		spec.ExtraKinds = []string{v.Kind}
		spec.IndexKinds = []string{v.Kind}
		spec.KindDefs = map[string]string{v.Kind: v.DefsPackage}
	}
	if s.ClassIDs {
		spec.Identity = identity
	}
	if s.PackageIDs {
		spec.QualifiedIDs = func(path string) []string { return JVMQualifiedIDs(path, s.ParsePackage, identity) }
	}
	return spec
}

// NewClassLang returns the language described by s.
func NewClassLang(s ClassLangSpec) *SingleLang { return NewSingleLang(s.Spec()) }
