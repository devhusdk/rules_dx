package javascript

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	languageName    = "javascript"
	libraryKind     = "javascript_library"
	testKind        = "javascript_test"
	binaryKind      = "javascript_binary"
	linkKind        = "npm_link_all_packages"
	jsLibraryKind   = "js_library"
	packageKind     = "npm_package"
	packageJSONName = "package_json"
	packageName     = "pkg"
	linkDefs        = "@npm//:defs.bzl"
	rootNodeModules = "//:node_modules"
	rootPackageJSON = "//:package_json"
)

var javascriptKinds = map[string]rule.KindInfo{
	libraryKind: libraryKindInfo(),
	testKind:    testKindInfo(),
	binaryKind:  binaryKindInfo(),
	linkKind:    {},
	jsLibraryKind: {
		MergeableAttrs: map[string]bool{"srcs": true},
	},
	packageKind: {
		MergeableAttrs: map[string]bool{"srcs": true},
	},
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
		MatchAttrs:    []string{"entry_point"},
		NonEmptyAttrs: map[string]bool{"data": true},
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
		ExtraKinds:   []string{testKind, binaryKind, linkKind, jsLibraryKind, packageKind},
		KindDefs: map[string]string{
			linkKind:      linkDefs,
			jsLibraryKind: "@aspect_rules_js//js:defs.bzl",
			packageKind:   "@aspect_rules_js//npm:defs.bzl",
		},
		Kinds:       javascriptKinds,
		DefsPackage: "javascript/rules:defs.bzl",
		ImportAttr:  "srcs",
		IsImportSource: func(src string) bool {
			return isSupported(src) || common.IsAssetSource(src)
		},
		Identity: func(src string) string {
			if isSupported(src) {
				return ModuleName(src)
			}
			return src
		},
		PathIdentity:        true,
		IsStdLib:            common.IsNodeBuiltin,
		Find:                common.JSFind(languageName),
		ConfigureExt:        configureExt,
		MissingExternalHint: common.NpmMissingHint,
		UnresolvedHint:      "add a local one-source library or an exact # gazelle:resolve mapping, or a tsconfig/jsconfig paths alias (relative imports use the workspace path, with the module extension removed, or <path>/index)",
		External:            "npm",
		Generate:            generateRules,
	})
}

const npmHubKey = "javascript:npm_hub"

func configureExt(c *config.Config, rel string, f *rule.File, rep common.Reporter) {
	common.ConfigureTSPathsExt(c, rel, f, rep)
	if !fileExists(filepath.Join(c.RepoRoot, filepath.FromSlash(rel), "pnpm-lock.yaml")) {
		return
	}
	ours := c.Exts[npmHubKey] == true
	if _, set := c.KindMap[linkKind]; set && !ours {
		return
	}
	hub := common.NpmHub(c.RepoRoot, path.Join(rel, "pnpm-lock.yaml"))
	if hub == "npm" {
		if ours {
			delete(c.KindMap, linkKind)
			c.Exts[npmHubKey] = false
		}
		return
	}
	if c.KindMap == nil {
		c.KindMap = map[string]config.MappedKind{}
	}
	c.KindMap[linkKind] = config.MappedKind{FromKind: linkKind, KindName: linkKind, KindLoad: "@" + hub + "//:defs.bzl"}
	c.Exts[npmHubKey] = true
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
	var result language.GenerateResult
	member := args.Config != nil && common.IsPnpmWorkspaceMember(args.Config.RepoRoot, args.Rel)
	if fileExists(filepath.Join(args.Dir, "pnpm-lock.yaml")) || member {
		link := rule.NewRule(linkKind, "node_modules")
		result.Gen = append(result.Gen, link)
		result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
	}
	if args.Rel == "" && fileExists(filepath.Join(args.Dir, "package.json")) {
		pj := rule.NewRule(jsLibraryKind, packageJSONName)
		pj.SetAttr("srcs", []string{"package.json"})
		pj.SetAttr("visibility", []string{"//visibility:public"})
		result.Gen = append(result.Gen, pj)
		result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
	}
	// TypeScript writes the package target when the package has TS sources.
	member = member && !hasTypeScript(args.RegularFiles)
	if len(sources) == 0 {
		if member {
			result = addPackage(result, nil)
		}
		return mergeStale(args.File, result)
	}

	type plan struct {
		name  string
		src   string
		test  bool
		entry bool
		refs  []common.ImportRef
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
	if err := common.CheckClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(common.InPackage(args.Rel), "javascript: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	plain := common.CustomRunnerTests(args.File, args.OtherGen, testKind, "jest")
	hasRootPackageJSON := rootPackageJSONExists(args.Config.RepoRoot)
	var libraries []string
	for _, p := range plans {
		if p.test {
			if plain[p.name] {
				continue
			}
			if msg := common.JestPreflight(args.Config.RepoRoot); msg != "" {
				rep.Fail(common.InPackage(args.Rel), "javascript: %s: %s generates a jest test; %s", args.Rel, p.src, msg)
				return language.GenerateResult{}
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
		libraries = append(libraries, ":"+p.name)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
		if p.entry && !common.CoveredBinary(args.File, binaryKind, common.BinaryName(p.name), servesEntry(p.src)) {
			bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
			bin.SetAttr("entry_point", p.src)
			bin.SetAttr("data", []string{":" + p.name})
			result.Gen = append(result.Gen, bin)
			result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
		}
	}
	if member {
		result = addPackage(result, libraries)
	}
	if common.FixtureTestonly(args.Config, args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

// addPackage adds the npm_package a pnpm workspace link resolves to.
func addPackage(result language.GenerateResult, libraries []string) language.GenerateResult {
	pkg := rule.NewRule(packageKind, packageName)
	pkg.SetAttr("srcs", append([]string{"package.json"}, libraries...))
	pkg.SetAttr("visibility", []string{"//visibility:public"})
	result.Gen = append(result.Gen, pkg)
	result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
	return result
}

func hasTypeScript(files []string) bool {
	for _, f := range files {
		if strings.HasSuffix(f, ".d.ts") || strings.HasSuffix(f, ".d.mts") || strings.HasSuffix(f, ".d.cts") {
			continue
		}
		if common.HasExt(f, []string{".ts", ".tsx", ".mts", ".cts"}) {
			return true
		}
	}
	return false
}

// servesEntry reports whether a binary runs entry.
func servesEntry(entry string) func(*rule.Rule) bool {
	return func(r *rule.Rule) bool { return r.AttrString("entry_point") == entry }
}

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStaleOwned(file, result, javascriptKinds, func(r *rule.Rule) bool {
		switch r.Kind() {
		case linkKind:
			return false
		case jsLibraryKind:
			return r.Name() == packageJSONName
		case packageKind:
			return r.Name() == packageName
		case testKind:
			return r.AttrString("runner") == ""
		case binaryKind:
			return common.GeneratedBinary(r, "data")
		}
		return true
	})
}

// rootPackageJSONExists reports whether the repo root has a package.json file.
func rootPackageJSONExists(repoRoot string) bool {
	return fileExists(filepath.Join(repoRoot, "package.json"))
}

func fileExists(name string) bool {
	info, err := os.Stat(name)
	return err == nil && !info.IsDir()
}

func init() { common.RegisterLanguage(NewLanguage) }
