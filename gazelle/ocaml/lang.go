package ocaml

import (
	"path"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const (
	languageName = "ocaml"
	// ModuleKind is the only kind this generator emits.
	ModuleKind = "ocaml_module"
)

var ocamlKinds = map[string]rule.KindInfo{
	ModuleKind: {
		MatchAttrs:     []string{"struct"},
		NonEmptyAttrs:  map[string]bool{"struct": true},
		MergeableAttrs: map[string]bool{"sig": true, "struct": true},
	},
}

type ocamlLang struct {
	*common.SingleLang
}

// NewLanguage returns a narrow OCaml generator. It emits one ocaml_module per
// .ml file (with its sibling .mli as sig) and states only facts readable from
// file names. It does not infer module dependencies: OCaml module references
// are not resolvable without ocamldep and the dune/opam layout, so deps stay
// handwritten.
func NewLanguage() language.Language {
	return &ocamlLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName: languageName,
		LibraryKind:  ModuleKind,
		Kinds:        ocamlKinds,
		DefsPackage:  "ocaml/rules:defs.bzl",
		ImportAttr:   "struct",
		Generate:     generateRules,
	})}
}

func (l *ocamlLang) Imports(*config.Config, *rule.Rule, *rule.File) []resolve.ImportSpec {
	return nil
}

func (l *ocamlLang) Resolve(*config.Config, *resolve.RuleIndex, *repo.RemoteCache, *rule.Rule, interface{}, label.Label) {
}

// moduleName maps foo_bar.ml to the target name foo_bar.
func moduleName(file string) string { return strings.TrimSuffix(file, ".ml") }

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	present := make(map[string]bool, len(args.RegularFiles))
	var impls []string
	for _, f := range args.RegularFiles {
		present[f] = true
		if strings.HasSuffix(f, ".ml") {
			impls = append(impls, f)
		}
	}
	sort.Strings(impls)
	var claimants []common.Claimant
	for _, f := range impls {
		claimants = append(claimants, common.Claimant{Name: moduleName(f), Source: path.Join(args.Rel, f), Kind: ModuleKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		rep.Fail(common.InPackage(args.Rel), "ocaml: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	result := language.GenerateResult{}
	for _, f := range impls {
		r := rule.NewRule(ModuleKind, moduleName(f))
		r.SetAttr("struct", f)
		if sig := f + "i"; present[sig] {
			r.SetAttr("sig", sig)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, nil)
	}
	owns := func(r *rule.Rule) bool { return !present[r.AttrString("struct")] }
	return common.MergeStaleOwned(args.File, result, kinds, owns)
}
