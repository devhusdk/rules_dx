package typescript

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	languageName    = "typescript"
	projectKind     = "typescript_project"
	testKind        = "typescript_test"
	plainTestKind   = "typescript_js_test"
	binaryKind      = "javascript_binary"
	rootNodeModules = "//:node_modules"
	rootPackageJSON = "//:package_json"
)

// claimAliases lets a handwritten plain-runner test stand in for the generated test of the same name.
var claimAliases = map[string]string{plainTestKind: testKind}

var typescriptKinds = map[string]rule.KindInfo{
	projectKind: projectKindInfo(),
	testKind:    testKindInfo(),
	binaryKind:  binaryKindInfo(),
}

func projectKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true, "declaration_srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":             true,
			"declaration_srcs": true,
			"deps":             true,
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
			"deps":         true,
			"data":         true,
			"node_modules": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func binaryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs: []string{"entry_point"},
		MergeableAttrs: map[string]bool{
			"data": true,
		},
		ResolveAttrs: map[string]bool{"data": true},
	}
}

type typescriptLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language { return newLang() }

func newLang() *typescriptLang {
	return &typescriptLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName: languageName,
		LibraryKind:  projectKind,
		ExtraKinds:   []string{testKind, binaryKind},
		Kinds:        typescriptKinds,
		DefsPackage:  "typescript/rules:defs.bzl",
		KindDefs:     map[string]string{binaryKind: "javascript/rules:defs.bzl"},
		ImportAttr:   "srcs",
		IsImportSource: func(src string) bool {
			return isSupported(src) || (common.IsAssetSource(src) && !IsDeclaration(src))
		},
		IsTestSource: func(src string) bool { return isSupported(src) && IsTestFile(src) },
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
		UnresolvedHint: "add a local one-source library or declaration file, or an exact # gazelle:resolve mapping, or a tsconfig/jsconfig paths alias (relative imports use the workspace path, with the module extension removed, or <path>/index)",
		External:       "npm",
		Generate:       generateRules,
	})}
}

// Imports adds the modules of plain declaration_srcs to the project's index.
func (l *typescriptLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	specs := l.SingleLang.Imports(c, r, f)
	if r.Kind() != projectKind {
		return specs
	}
	seen := make(map[string]bool, len(specs))
	for _, spec := range specs {
		seen[spec.Imp] = true
	}
	pkg := ""
	if f != nil {
		pkg = f.Pkg
	}
	for _, src := range plainDeclarations(r) {
		imp := common.ModuleIdentity(pkg, ModuleName(src))
		if seen[imp] {
			continue
		}
		seen[imp] = true
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: imp})
	}
	return specs
}

// plainDeclarations returns the declaration files named directly in a rule's
// declaration_srcs. Labels and paths into other packages are skipped.
func plainDeclarations(r *rule.Rule) []string {
	var out []string
	for _, src := range r.AttrStrings("declaration_srcs") {
		src = strings.TrimPrefix(src, ":")
		if src == "" || strings.ContainsAny(src, "/:") || !IsDeclaration(src) {
			continue
		}
		out = append(out, src)
	}
	return out
}

// ownedDeclarations returns the declaration files already listed in
// declaration_srcs by an existing project, test or plain-test rule other than
// the one a generated declaration rule would be named.
func ownedDeclarations(file *rule.File, other []*rule.Rule) map[string]bool {
	owned := make(map[string]bool)
	add := func(r *rule.Rule) {
		switch r.Kind() {
		case projectKind, testKind, plainTestKind:
		default:
			return
		}
		for _, decl := range plainDeclarations(r) {
			if name, err := TargetName(decl); err == nil && name == r.Name() {
				continue
			}
			owned[decl] = true
		}
	}
	if file != nil {
		for _, r := range file.Rules {
			add(r)
		}
	}
	for _, r := range other {
		add(r)
	}
	return owned
}

// carryDeclarationSrcs copies hand-written declaration_srcs from an existing
// rule of the same kind and name onto a regenerated source rule.
func carryDeclarationSrcs(file *rule.File, gen *rule.Rule) {
	if file == nil {
		return
	}
	for _, r := range file.Rules {
		if r.Kind() == gen.Kind() && r.Name() == gen.Name() {
			if expr := r.Attr("declaration_srcs"); expr != nil {
				gen.SetAttr("declaration_srcs", expr)
			}
			return
		}
	}
}

func isSupported(name string) bool {
	if IsDeclaration(name) {
		return false
	}
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
	modules := make(map[string]bool, len(sources))
	for _, src := range sources {
		modules[ModuleName(src)] = true
	}
	owned := ownedDeclarations(args.File, args.OtherGen)
	var decls []string
	for _, name := range args.RegularFiles {
		if IsDeclaration(name) && !modules[ModuleName(name)] && !owned[name] {
			decls = append(decls, name)
		}
	}
	sort.Strings(decls)

	type plan struct {
		name    string
		src     string
		test    bool
		entry   bool
		decl    bool
		refs    []common.ImportRef
	}
	var plans []plan
	all := append(append([]string(nil), sources...), decls...)
	for _, src := range all {
		content, err := common.ReadSource(filepath.Join(args.Dir, src))
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "typescript: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		name, err := TargetName(src)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "typescript: %s: %v", args.Rel, err)
			continue
		}
		p := plan{name: name, src: src, test: IsTestFile(src)}
		p.decl = IsDeclaration(src)
		p.entry = !p.test && !p.decl && IsEntryFile(src)
		p.refs = common.JSImportRefs(common.ParseImportRefs(content, path.Join(args.Rel, path.Dir(src))))
		plans = append(plans, p)
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}

	claimants := make([]Claimant, 0, len(plans)*2)
	for _, p := range plans {
		kind := projectKind
		if p.test {
			kind = testKind
		}
		claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: kind})
		if p.entry {
			claimants = append(claimants, Claimant{Name: common.BinaryName(p.name), Source: p.src, Kind: binaryKind})
		}
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants, claimAliases); err != nil {
		rep.Fail(common.InPackage(args.Rel), "typescript: %s: %v", args.Rel, err)
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
		r := rule.NewRule(projectKind, p.name)
		if p.decl {
			r.SetAttr("declaration_srcs", []string{p.src})
		} else {
			r.SetAttr("srcs", []string{p.src})
			carryDeclarationSrcs(args.File, r)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
		if p.entry {
			bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
			bin.SetAttr("entry_point", EntryPointName(p.src))
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
	return common.MergeStale(file, result, typescriptKinds)
}

// rootPackageJSONExists reports whether the repo root has a package.json file.
func rootPackageJSONExists(repoRoot string) bool {
	info, err := os.Stat(filepath.Join(repoRoot, "package.json"))
	return err == nil && !info.IsDir()
}
