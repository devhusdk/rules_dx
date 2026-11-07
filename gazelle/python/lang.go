package python

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"regexp"
	"slices"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	languageName = "python"
	libraryKind  = "python_library"
	testKind     = "python_test"
	binaryKind   = "python_binary"
)

var pythonKinds = map[string]rule.KindInfo{
	libraryKind: kindInfo(),
	testKind:    kindInfo(),
	binaryKind:  binaryKindInfo(),
	ruffKind:    ruffKindInfo(),
}

func kindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":    true,
			"imports":      true,
			"deps":         true,
			"aspect_hints": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func ruffKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:     []string{"src"},
		NonEmptyAttrs:  map[string]bool{"src": true},
		MergeableAttrs: map[string]bool{"src": true, "data": true},
	}
}

func binaryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"main"},
		NonEmptyAttrs: map[string]bool{"deps": true},
		MergeableAttrs: map[string]bool{
			"imports":      true,
			"deps":         true,
			"aspect_hints": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

type pythonLang struct{ *common.SingleLang }

func NewLanguage() language.Language { return newLang() }

func newLang() *pythonLang {
	return &pythonLang{common.NewSingleLang(common.SingleLangSpec{
		LanguageName:   languageName,
		LibraryKind:    libraryKind,
		ExtraKinds:     []string{testKind, binaryKind, ruffKind},
		KindDefs:       map[string]string{ruffKind: nativeDefs},
		ConfigureExt:   configureRuff,
		Kinds:          pythonKinds,
		DefsPackage:    "python/rules:defs.bzl",
		ImportAttr:     "srcs",
		IsImportSource: func(name string) bool { return strings.HasSuffix(name, ".py") },
		IsStdLib:       IsStdLib,
		Find:           findModule,
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		External:       "python",
		Generate:       generateRules,
	})}
}

// Imports indexes each library source under its dotted module name, relative
// to the first entry of the library's imports.
func (l *pythonLang) Imports(_ *config.Config, r *rule.Rule, f *rule.File) []resolve.ImportSpec {
	if r.Kind() != libraryKind {
		return nil
	}
	pkg := ""
	if f != nil {
		pkg = f.Pkg
	}
	root := ""
	if imports := r.AttrStrings("imports"); len(imports) > 0 {
		root = path.Join(pkg, imports[0])
		if root == "." {
			root = ""
		}
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !strings.HasSuffix(src, ".py") || IsTestFile(src) || IsConftest(src) {
			continue
		}
		p := path.Join(pkg, src)
		if root != "" {
			var ok bool
			if p, ok = strings.CutPrefix(p, root+"/"); !ok {
				continue
			}
		}
		module := strings.ReplaceAll(strings.TrimSuffix(p, ".py"), "/", ".")
		if module == "__init__" {
			continue
		}
		module = strings.TrimSuffix(module, ".__init__")
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: module})
	}
	return specs
}

// findModule looks the longest dotted prefix of ref up, overrides first.
func findModule(c *config.Config, ix *resolve.RuleIndex, ref common.ImportRef) common.JSLookup {
	name := ref.Spec
	if name == "" {
		name = ref.Root
	}
	for {
		imp := resolve.ImportSpec{Lang: languageName, Imp: name}
		if override, ok := resolve.FindRuleWithOverride(c, imp, languageName); ok {
			return common.JSLookup{Override: override, HasOverride: true, Matched: name}
		}
		if matches := ix.FindRulesByImportWithConfig(c, imp, languageName); len(matches) > 0 {
			return common.JSLookup{Matches: matches, Matched: name}
		}
		i := strings.LastIndexByte(name, '.')
		if i < 0 {
			return common.JSLookup{Matched: ref.Root}
		}
		name = name[:i]
	}
}

// packageDepth counts the directories from rel upward that hold an
// __init__.py, never leaving the repository.
func packageDepth(root, rel string) int {
	depth := 0
	for dir := rel; dir != "" && dir != "."; dir = path.Dir(dir) {
		if _, err := os.Stat(filepath.Join(root, filepath.FromSlash(dir), "__init__.py")); err != nil {
			break
		}
		depth++
	}
	return depth
}

func lastParts(rel string, n int) string {
	parts := strings.Split(rel, "/")
	return strings.Join(parts[len(parts)-n:], "/")
}

func importsFor(depth int) []string {
	if depth == 0 {
		return []string{"."}
	}
	return []string{strings.TrimSuffix(strings.Repeat("../", depth), "/")}
}

// importRefs turns the imports of one module in package pkg (dotted) into
// refs. Relative imports become absolute dotted names.
func importRefs(imps []Import, pkg string) ([]common.ImportRef, error) {
	var refs []common.ImportRef
	for _, imp := range imps {
		module := imp.Module
		if imp.Level > 0 {
			parts := []string{}
			if pkg != "" {
				parts = strings.Split(pkg, ".")
			}
			if imp.Level > len(parts) {
				return nil, fmt.Errorf("relative import %q goes beyond the top-level package", strings.Repeat(".", imp.Level)+imp.Module)
			}
			parts = parts[:len(parts)-imp.Level+1]
			if imp.Module != "" {
				parts = append(parts, imp.Module)
			}
			module = strings.Join(parts, ".")
		}
		specs := []string{module}
		if len(imp.Names) > 0 {
			specs = specs[:0]
			for _, name := range imp.Names {
				specs = append(specs, module+"."+name)
			}
		}
		root, _, _ := strings.Cut(module, ".")
		if imp.Level == 0 && IsStdLib(root) {
			continue
		}
		for _, spec := range specs {
			refs = append(refs, common.ImportRef{Spec: spec, Root: root, Relative: imp.Level > 0})
		}
	}
	return refs, nil
}

func mergeRefs(refs []common.ImportRef) []common.ImportRef {
	sort.Slice(refs, func(i, j int) bool { return refs[i].Spec < refs[j].Spec })
	return slices.CompactFunc(refs, func(a, b common.ImportRef) bool { return a == b })
}

func generateRules(args language.GenerateArgs, _ map[string]rule.KindInfo, rep common.Reporter) language.GenerateResult {
	if args.Rel == ".dx" || strings.HasPrefix(args.Rel, ".dx/") {
		// dx init owns these packages, including the root ruff config.
		return language.GenerateResult{}
	}
	var sources []string
	stubs := make(map[string]bool)
	for _, name := range args.RegularFiles {
		switch {
		case strings.HasSuffix(name, ".py"):
			sources = append(sources, name)
		case strings.HasSuffix(name, ".pyi"):
			stubs[name] = true
		}
	}
	sort.Strings(sources)
	ruff := ruffAt(args.Config)
	var result language.GenerateResult
	if ruff != nil && ruff.rel == args.Rel && !ruff.initOwned {
		result.Gen = append(result.Gen, ruffRule(ruff, args.Rel))
		result.Imports = append(result.Imports, common.WrapImportSet(nil))
	}
	if len(sources) == 0 {
		return mergeStale(args.File, result)
	}

	depth := packageDepth(args.Config.RepoRoot, args.Rel)
	pkgDotted := ""
	if depth > 0 {
		pkgDotted = strings.ReplaceAll(lastParts(args.Rel, depth), "/", ".")
	}
	inPackage := func(src string) bool { return depth > 0 && !IsTestFile(src) && !IsConftest(src) }

	type plan struct {
		name  string
		srcs  []string
		main  string
		test  bool
		entry bool
		plain bool
		refs  []common.ImportRef
	}
	var plans []plan
	pkgIdx := -1
	for _, src := range sources {
		content, err := common.ReadSource(filepath.Join(args.Dir, src))
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "python: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		refs, err := importRefs(ParseImports(content), pkgDotted)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "python: %s: %s: %v", args.Rel, src, err)
			continue
		}
		srcs := []string{src}
		if stub := strings.TrimSuffix(src, ".py") + ".pyi"; stubs[stub] {
			srcs = append(srcs, stub)
		}
		entry := !IsTestFile(src) && IsEntryFile(src)
		if inPackage(src) {
			if pkgIdx < 0 {
				name, err := common.Normalize(path.Base(args.Rel))
				if err != nil {
					rep.Fail(common.InPackage(args.Rel), "python: %s: %v", args.Rel, err)
					continue
				}
				plans = append(plans, plan{name: name})
				pkgIdx = len(plans) - 1
			}
			pkg := &plans[pkgIdx]
			pkg.srcs = append(pkg.srcs, srcs...)
			pkg.refs = append(pkg.refs, refs...)
			if entry {
				pkg.entry, pkg.main = true, src
			}
			continue
		}
		name, err := TargetName(src)
		if err != nil {
			rep.Fail(common.InPackage(args.Rel), "python: %s: %v", args.Rel, err)
			continue
		}
		test := IsTestFile(src)
		plans = append(plans, plan{name: name, srcs: srcs, main: src, test: test, entry: entry, plain: test && unittestOnly(content), refs: refs})
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
		claimants = append(claimants, Claimant{Name: p.name, Source: p.srcs[0], Kind: kind})
		if p.entry {
			claimants = append(claimants, Claimant{Name: common.BinaryName(p.name), Source: p.main, Kind: binaryKind})
		}
	}
	if err := checkClaims(args.File, args.OtherGen, claimants); err != nil {
		rep.Fail(common.InPackage(args.Rel), "python: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	conftestDeps := conftestLabels(args.Config.RepoRoot, args.Rel, sources)
	plain := common.CustomRunnerTests(args.File, args.OtherGen, testKind, "pytest")
	imports := importsFor(depth)
	for _, p := range plans {
		kind := libraryKind
		p.plain = p.plain && len(conftestDeps) == 0
		refs := p.refs
		if p.test {
			if plain[p.name] && !p.plain {
				continue
			}
			kind = testKind
			if !p.plain {
				refs = append(slices.Clone(refs), common.ImportRef{Spec: "pytest", Root: "pytest"})
			}
		}
		r := rule.NewRule(kind, p.name)
		r.SetAttr("srcs", p.srcs)
		r.SetAttr("imports", imports)
		if p.plain {
			r.SetAttr("runner", "plain")
		}
		if p.test && len(conftestDeps) > 0 {
			r.SetAttr("deps", conftestDeps)
		}
		if IsConftest(p.main) {
			r.SetAttr("testonly", true)
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, common.WrapRefImportSet(mergeRefs(refs)))
		if p.entry && !common.CoveredBinary(args.File, binaryKind, common.BinaryName(p.name), servesMain(p.main)) {
			bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
			bin.SetAttr("main", p.main)
			bin.SetAttr("imports", imports)
			bin.SetAttr("deps", []string{":" + p.name})
			result.Gen = append(result.Gen, bin)
			result.Imports = append(result.Imports, common.WrapImportSet(nil))
		}
	}
	for _, r := range result.Gen {
		if r.Kind() == ruffKind {
			continue
		}
		if hints := withRuffHint(args.Config, existingRule(args.File, r.Kind(), r.Name()), ruff, args.Rel); len(hints) > 0 {
			r.SetAttr("aspect_hints", hints)
		}
		if common.FixtureTestonly(args.Config, args.Rel) {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

var bareTestFunc = regexp.MustCompile(`(?m)^(?:async\s+)?def\s+test`)

// unittestOnly reports whether a test file is plain unittest: it imports
// unittest and nothing of pytest, and defines no bare test functions.
func unittestOnly(content []byte) bool {
	usesUnittest := false
	for _, imp := range ParseImports(content) {
		root, _, _ := strings.Cut(imp.Module, ".")
		if imp.Level > 0 {
			continue
		}
		switch root {
		case "pytest", "_pytest":
			return false
		case "unittest":
			usesUnittest = true
		}
	}
	return usesUnittest && !bareTestFunc.Match(content)
}

func conftestLabels(root, rel string, sources []string) []string {
	var labels []string
	if slices.ContainsFunc(sources, IsConftest) {
		labels = append(labels, ":conftest")
	}
	for dir := rel; dir != ""; {
		dir = path.Dir(dir)
		if dir == "." {
			dir = ""
		}
		if _, err := os.Stat(filepath.Join(root, dir, "conftest.py")); err == nil {
			labels = append(labels, "//"+dir+":conftest")
		}
		if dir == "" {
			break
		}
	}
	return labels
}

func claimKind(c Claimant) string {
	if c.Kind != "" {
		return c.Kind
	}
	if IsTestFile(c.Source) {
		return testKind
	}
	return libraryKind
}

func checkClaims(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	filled := make([]Claimant, len(claimants))
	for i, c := range claimants {
		c.Kind = claimKind(c)
		filled[i] = c
	}
	return common.CheckClaims(file, other, filled, nil)
}

// servesMain reports whether a binary runs main: its main attr names it, or it
// has no main attr and lists it in srcs.
func servesMain(main string) func(*rule.Rule) bool {
	return func(r *rule.Rule) bool {
		if m := r.AttrString("main"); m != "" {
			return m == main
		}
		return slices.Contains(r.AttrStrings("srcs"), main)
	}
}

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStaleOwned(file, result, pythonKinds, func(r *rule.Rule) bool {
		switch r.Kind() {
		case testKind:
			return r.AttrString("runner") == ""
		case binaryKind:
			return common.GeneratedBinary(r, "deps")
		}
		return true
	})
}

func init() { common.RegisterLanguage(NewLanguage) }
