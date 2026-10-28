package powershell

import (
	"path/filepath"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

// UnresolvedHint tells users how to settle an import no rule owns.
const UnresolvedHint = "add a pwsh_library that owns that module, an exact # gazelle:resolve powershell mapping; module names match case-sensitively"

var pwshKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
	TestKind: {
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"deps": true,
			"srcs": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type pwshLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language {
	return &pwshLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		ExtraKinds:     []string{TestKind},
		Kinds:          pwshKinds,
		DefsPackage:    "powershell/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       ModuleName,
		IsImportSource: isModuleFile,
		IsStdLib:       IsStdLib,
		UnresolvedHint: UnresolvedHint,
		Generate:       generateRules,
	})}
}

func isModuleFile(src string) bool {
	return strings.HasSuffix(src, ModuleExt) || strings.HasSuffix(src, ManifestExt)
}

// Imports indexes a pwsh_library once per module, so Greet.psd1 and
// Greet.psm1 together export a single Greet.
func (l *pwshLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	seen := make(map[string]bool)
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !isModuleFile(src) {
			continue
		}
		name := ModuleName(src)
		if name == "" || seen[name] {
			continue
		}
		seen[name] = true
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: name})
	}
	return specs
}

func generateRules(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	present := make(map[string]bool, len(args.RegularFiles))
	for _, name := range args.RegularFiles {
		present[name] = true
	}
	var modules, tests []string
	for _, name := range args.RegularFiles {
		switch {
		case IsTestSource(name):
			tests = append(tests, name)
		case IsModuleSource(name, present):
			modules = append(modules, name)
		}
	}
	sort.Strings(modules)
	sort.Strings(tests)
	if len(modules) == 0 && len(tests) == 0 {
		return common.MergeStaleOwned(args.File, language.GenerateResult{}, kinds, Owns)
	}
	var claimants []common.Claimant
	name := ""
	if len(modules) > 0 {
		var err error
		if name, err = common.PackageTargetName(args); err != nil {
			rep.Fail(common.InPackage(args.Rel), "powershell: %s: %v", args.Rel, err)
			return language.GenerateResult{}
		}
		claimants = append(claimants, common.Claimant{Name: name, Source: args.Rel, Kind: LibraryKind})
	}
	testNames := make([]string, len(tests))
	for i, src := range tests {
		testName, err := TestTargetName(src)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "powershell: %s: %s: %v", args.Rel, src, err)
			return language.GenerateResult{}
		}
		testNames[i] = testName
		claimants = append(claimants, common.Claimant{Name: testName, Source: filepath.ToSlash(filepath.Join(args.Rel, src)), Kind: TestKind})
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, nil); err != nil {
		rep.Fail(common.InPackage(args.Rel), "powershell: %s: %v; rename the handwritten rule or add # gazelle:exclude for this directory", args.Rel, err)
		return language.GenerateResult{}
	}
	own := make(map[string]bool, len(modules))
	for _, src := range modules {
		own[ModuleName(src)] = true
	}
	var result language.GenerateResult
	if len(modules) > 0 {
		imports, ok := readImports(args, modules, own, rep)
		if !ok {
			return language.GenerateResult{}
		}
		r := rule.NewRule(LibraryKind, name)
		r.SetAttr("srcs", modules)
		if common.IsFixturePath(args.Rel) {
			r.SetAttr("testonly", true)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(imports))
	}
	for i, src := range tests {
		imports, ok := readImports(args, []string{src}, nil, rep)
		if !ok {
			return language.GenerateResult{}
		}
		r := rule.NewRule(TestKind, testNames[i])
		r.SetAttr("srcs", []string{src})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapImportSet(imports))
	}
	return common.MergeStaleOwned(args.File, result, kinds, Owns)
}

func readImports(args language.GenerateArgs, files []string, own map[string]bool, rep common.Reporter) ([]string, bool) {
	set := make(map[string]bool)
	for _, file := range files {
		content, err := common.ReadSource(filepath.Join(args.Dir, file))
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "powershell: %s: read %s: %v", args.Rel, file, err)
			return nil, false
		}
		for _, name := range ParseImports(content, strings.HasSuffix(file, ManifestExt)) {
			if !own[name] {
				set[name] = true
			}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out, true
}
