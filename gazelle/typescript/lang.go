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
	libraryKind     = "typescript_library"
	testKind        = "typescript_test"
	binaryKind      = "typescript_binary"
	configKind      = "ts_config"
	packageKind     = "npm_package"
	configName      = "tsconfig"
	packageName     = "pkg"
	rootNodeModules = "//:node_modules"
	rootPackageJSON = "//:package_json"
)

var typescriptKinds = map[string]rule.KindInfo{
	libraryKind: libraryKindInfo(),
	testKind:    testKindInfo(),
	binaryKind:  binaryKindInfo(),
	configKind: {
		MatchAttrs:     []string{"src"},
		MergeableAttrs: map[string]bool{"deps": true},
	},
	packageKind: {
		MergeableAttrs: map[string]bool{"srcs": true},
	},
}

func libraryKindInfo() rule.KindInfo {
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
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs": true,
			"deps": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

type typescriptLang struct {
	*common.SingleLang
}

func NewLanguage() language.Language { return newLang() }

func newLang() *typescriptLang {
	return &typescriptLang{SingleLang: common.NewSingleLang(common.SingleLangSpec{
		LanguageName: languageName,
		LibraryKind:  libraryKind,
		ExtraKinds:   []string{testKind, binaryKind, configKind, packageKind},
		Kinds:        typescriptKinds,
		DefsPackage:  "typescript:defs.bzl",
		KindDefs: map[string]string{
			configKind:  "@aspect_rules_ts//ts:defs.bzl",
			packageKind: "@aspect_rules_js//npm:defs.bzl",
		},
		ImportAttr: "srcs",
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
		PathIdentity:        true,
		IsStdLib:            common.IsNodeBuiltin,
		Find:                common.JSFind(languageName),
		ConfigureExt:        common.ConfigureTSPathsExt,
		MissingExternalHint: common.NpmMissingHint,
		UnresolvedHint:      "add a local one-source library or declaration file, or an exact # gazelle:resolve mapping, or a tsconfig/jsconfig paths alias (relative imports use the workspace path, with the module extension removed, or <path>/index)",
		External:            "npm",
		Generate:            generateRules,
	})}
}

// Imports adds the modules of plain declaration_srcs to the project's index.
func (l *typescriptLang) Imports(c *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	specs := l.SingleLang.Imports(c, r, f)
	if r.Kind() != libraryKind {
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
		case libraryKind, testKind:
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

type plan struct {
	name  string
	src   string
	test  bool
	entry bool
	decl  bool
	refs  []common.ImportRef
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
	repoRoot := ""
	if args.Config != nil {
		repoRoot = args.Config.RepoRoot
	}
	if args.Dir != "" && fileExists(filepath.Join(args.Dir, "tsconfig.json")) {
		parents, err := common.TSConfigParents(repoRoot, args.Rel)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "typescript: %s: read tsconfig.json: %v", args.Rel, err)
			return language.GenerateResult{}
		}
		cfg := rule.NewRule(configKind, configName)
		cfg.SetAttr("src", "tsconfig.json")
		if len(parents) > 0 {
			cfg.SetAttr("deps", parents)
		}
		cfg.SetAttr("visibility", []string{"//visibility:public"})
		result.Gen = append(result.Gen, cfg)
		result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
	}
	if len(sources) == 0 {
		return mergeStale(args.File, result)
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
		p.entry = !p.test && !p.decl && IsEntryFile(src) && !strings.HasSuffix(src, ".tsx")
		p.refs = common.JSImportRefs(common.ParseSourceImportRefs(src, content, path.Join(args.Rel, path.Dir(src))))
		plans = append(plans, p)
	}
	if rep.Failed() {
		return language.GenerateResult{}
	}

	imported := importedEntries(args.Rel, plans)
	claimants := make([]Claimant, 0, len(plans)*2)
	for _, p := range plans {
		kind := libraryKind
		if p.test {
			kind = testKind
		}
		if !p.entry || imported[p.src] {
			claimants = append(claimants, Claimant{Name: p.name, Source: p.src, Kind: kind})
		}
		if p.entry {
			claimants = append(claimants, Claimant{Name: common.BinaryName(p.name), Source: p.src, Kind: binaryKind})
		}
	}
	if err := common.CheckClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(common.InPackage(args.Rel), "typescript: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	plain := common.CustomRunnerTests(args.File, args.OtherGen, testKind, "jest")
	hasRootPackageJSON := rootPackageJSONExists(repoRoot)
	tsconfig := common.NearestTSConfig(repoRoot, args.Rel)
	if tsconfig == "" && !allHaveTSConfig(args.File, plans) {
		rep.Fail(common.InPackage(args.Rel), "typescript: %s: no tsconfig.json here or above; add tsconfig.json at the repo root", args.Rel)
		return language.GenerateResult{}
	}
	esm := common.IsESMPackage(repoRoot, args.Rel)
	var libraries []string
	for _, p := range plans {
		if p.test {
			if plain[p.name] {
				continue
			}
			if msg := common.JestPreflight(repoRoot); msg != "" {
				rep.Fail(common.InPackage(args.Rel), "typescript: %s: %s generates a jest test; %s", args.Rel, p.src, msg)
				return language.GenerateResult{}
			}
			r := rule.NewRule(testKind, p.name)
			r.SetAttr("srcs", []string{p.src})
			setTSConfig(r, tsconfig)
			r.SetAttr("node_modules", rootNodeModules)
			if hasRootPackageJSON {
				r.SetAttr("package_json", rootPackageJSON)
			}
			if esm {
				r.SetAttr("node_options", []string{"--experimental-vm-modules"})
			}
			result.Gen = append(result.Gen, r)
			result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
			continue
		}
		if p.entry {
			if !common.CoveredBinary(args.File, binaryKind, common.BinaryName(p.name), servesEntry(p.src)) {
				bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
				bin.SetAttr("srcs", []string{p.src})
				bin.SetAttr("main", p.src)
				if tsconfig != "" {
					bin.SetAttr("tsconfig", tsconfig)
				}
				result.Gen = append(result.Gen, bin)
				result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
			}
			if !imported[p.src] {
				continue
			}
		}
		r := rule.NewRule(libraryKind, p.name)
		if p.decl {
			r.SetAttr("declaration_srcs", []string{p.src})
		} else {
			r.SetAttr("srcs", []string{p.src})
			carryDeclarationSrcs(args.File, r)
		}
		setTSConfig(r, tsconfig)
		libraries = append(libraries, ":"+p.name)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapRefImportSet(p.refs))
	}
	if common.IsPnpmWorkspaceMember(repoRoot, args.Rel) {
		pkg := rule.NewRule(packageKind, packageName)
		pkg.SetAttr("srcs", append([]string{"package.json"}, libraries...))
		pkg.SetAttr("visibility", []string{"//visibility:public"})
		result.Gen = append(result.Gen, pkg)
		result.Imports = append(result.Imports, common.WrapRefImportSet(nil))
	}
	if common.FixtureTestonly(args.Config, args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

// setTSConfig points a generated rule at its tsconfig and turns declaration
// output on so tests and dependents can import it.
func setTSConfig(r *rule.Rule, tsconfig string) {
	if tsconfig != "" {
		r.SetAttr("tsconfig", tsconfig)
	}
	r.SetAttr("declaration", true)
}

// allHaveTSConfig reports whether every planned rule already exists with a
// hand-set tsconfig.
func allHaveTSConfig(file *rule.File, plans []plan) bool {
	set := map[string]bool{}
	if file != nil {
		for _, r := range file.Rules {
			if r.Attr("tsconfig") != nil {
				set[r.Name()] = true
			}
		}
	}
	for _, p := range plans {
		if !set[p.name] {
			return false
		}
	}
	return true
}

// servesEntry reports whether a binary runs entry.
func servesEntry(entry string) func(*rule.Rule) bool {
	return func(r *rule.Rule) bool {
		main := r.AttrString("main")
		if main == "" {
			if srcs := r.AttrStrings("srcs"); len(srcs) == 1 {
				main = srcs[0]
			}
		}
		return main == entry
	}
}

// importedEntries maps each entry source that another source in the package
// imports.
func importedEntries(rel string, plans []plan) map[string]bool {
	roots := map[string]string{}
	for _, p := range plans {
		if p.entry {
			roots[common.ModuleIdentity(rel, ModuleName(p.src))] = p.src
		}
	}
	out := map[string]bool{}
	for _, p := range plans {
		for _, ref := range p.refs {
			if src, ok := roots[ref.Root]; ok && src != p.src {
				out[src] = true
			}
		}
	}
	return out
}

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStaleOwned(file, result, typescriptKinds, func(r *rule.Rule) bool {
		switch r.Kind() {
		case configKind:
			return r.Name() == configName
		case packageKind:
			return r.Name() == packageName
		case testKind:
			return r.AttrString("runner") == ""
		case binaryKind:
			return generatedBinary(r)
		}
		return true
	})
}

// generatedBinary reports whether r is the _bin binary gazelle derives from
// an entry source.
func generatedBinary(r *rule.Rule) bool {
	target, ok := strings.CutSuffix(r.Name(), common.BinaryName(""))
	if !ok || target == "" {
		return false
	}
	srcs := r.AttrStrings("srcs")
	return len(srcs) == 1 && IsEntryFile(srcs[0]) && ModuleName(srcs[0]) == target
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
