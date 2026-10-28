package javascript

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	languageName    = "javascript"
	libraryKind     = "javascript_library"
	testKind        = "javascript_test"
	plainTestKind   = "javascript_js_test"
	binaryKind      = "javascript_binary"
	rootNodeModules = "//:node_modules"
	rootPackageJSON = "//:package_json"
)

// claimAliases lets a handwritten plain-runner test stand in for the generated test of the same name.
var claimAliases = map[string]string{plainTestKind: testKind}

var javascriptKinds = map[string]rule.KindInfo{
	libraryKind: libraryKindInfo(),
	testKind:    testKindInfo(),
	binaryKind:  binaryKindInfo(),
}

func libraryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func testKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":         true,
			"data":         true,
			"node_modules": true,
		},
		ResolveAttrs: map[string]bool{"data": true},
	}
}

func binaryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs: []string{"entry_point"},
		MergeableAttrs: map[string]bool{
			"data":         true,
			"node_modules": true,
		},
		ResolveAttrs: map[string]bool{"data": true},
	}
}

func NewLanguage() language.Language { return newLang() }

func newLang() *common.SingleLang {
	return common.NewSingleLang(common.SingleLangSpec{
		LanguageName: languageName,
		LibraryKind:  libraryKind,
		ExtraKinds:   []string{testKind, binaryKind},
		Kinds:        javascriptKinds,
		DefsPackage:  "javascript/rules:defs.bzl",
		ImportAttr:   "srcs",
		IsImportSource: func(src string) bool {
			return isSupported(src) || common.IsAssetSource(src)
		},
		Identity: func(src string) string {
			if isSupported(src) {
				return ModuleName(src)
			}
			return src
		},
		PathIdentity:   true,
		IsStdLib:       common.IsNodeBuiltin,
		Find:           common.JSFind(languageName),
		ConfigureExt:   common.ConfigureTSPathsExt,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping, or a tsconfig/jsconfig paths alias (relative imports use the workspace path, with the module extension removed, or <path>/index)",
		External:       "npm",
		Generate:       generateRules,
	})
}

func isSupported(name string) bool {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(name, ext) {
			return true
		}
	}
	return false
}

func generateRules(args language.GenerateArgs, _ map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	var sources []string
	for _, name := range args.RegularFiles {
		if isSupported(name) {
			sources = append(sources, name)
		}
	}
	sort.Strings(sources)
	if len(sources) == 0 {
		return mergeStale(args.File, language.GenerateResult{})
	}

	type plan struct {
		name    string
		src     string
		test    bool
		entry   bool
		refs    []common.ImportRef
	}
	var plans []plan
	for _, src := range sources {
		content, err := common.ReadSource(filepath.Join(args.Dir, src))
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "javascript: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		name, err := TargetName(src)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "javascript: %s: %v", args.Rel, err)
			continue
		}
		p := plan{name: name, src: src, test: IsTestFile(src)}
		p.entry = !p.test && IsEntryFile(src)
		p.refs = common.JSImportRefs(common.ParseSourceImportRefs(src, content, path.Join(args.Rel, path.Dir(src))))
		plans = append(plans, p)
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}

	claimants := make([]Claimant, 0, len(plans)*2)
	for _, p := range plans {
		kind := libraryKind
		if p.test {
			kind = testKind
		}
		claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: kind})
		if p.entry {
			claimants = append(claimants, Claimant{Name: common.BinaryName(p.name), Source: p.src, Kind: binaryKind})
		}
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, claimAliases); err != nil {
		rep.Fail(common.InPackage(args.Rel), "javascript: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	plain := common.RuleNamesOfKind(args.File, args.OtherGen, plainTestKind)
	hasRootPackageJSON := rootPackageJSONExists(args.Config.RepoRoot)
	var result language.GenerateResult
	for _, p := range plans {
		if p.test {
			if plain[p.name] {
				continue
			}
			r := rule.NewRule(testKind, p.name)
			r.SetAttr("srcs", []string{p.src})
			r.SetAttr("node_modules", rootNodeModules)
			if hasRootPackageJSON {
				r.SetAttr("package_json", rootPackageJSON)
			}
			result.Gen = append(result.Gen, r)
			result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
			continue
		}
		r := rule.NewRule(libraryKind, p.name)
		r.SetAttr("srcs", []string{p.src})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
		if p.entry {
			bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
			bin.SetAttr("entry_point", p.src)
			bin.SetAttr("data", []string{":" + p.name})
			result.Gen = append(result.Gen, bin)
			result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
		}
	}
	if isFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, javascriptKinds)
}

// rootPackageJSONExists reports whether the repo root has a package.json file.
func rootPackageJSONExists(repoRoot string) bool {
	info, err := os.Stat(filepath.Join(repoRoot, "package.json"))
	return err == nil && !info.IsDir()
}
