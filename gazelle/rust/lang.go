package rust

import (
	"context"
	"flag"
	"fmt"
	"os"
	"path"
	"path/filepath"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

const (
	languageName  = "rust"
	libraryKind   = "rust_library"
	binaryKind    = "rust_binary"
	testKind      = "rust_test"
	procMacroKind = "rust_proc_macro"
	sharedKind    = "rust_shared_library"
	staticKind    = "rust_static_library"
	scriptKind    = "cargo_build_script"
	dxCrateKind   = "dx_rust_crate"
)

var rustKinds = map[string]rule.KindInfo{
	libraryKind:   kindInfo(),
	binaryKind:    kindInfo(),
	testKind:      kindInfo(),
	procMacroKind: kindInfo(),
	sharedKind:    kindInfo(),
	staticKind:    kindInfo(),
	scriptKind:    kindInfo(),
}

func isLibraryKind(kind string) bool {
	switch kind {
	case libraryKind, procMacroKind, sharedKind, staticKind:
		return true
	}
	return false
}

func shouldSetVisibility(args language.GenerateArgs) bool {
	if args.File != nil && args.File.HasDefaultVisibility() {
		return false
	}
	for _, r := range args.OtherGen {
		if r.Kind() == "package" && r.Attr("default_visibility") != nil {
			return false
		}
	}
	return true
}

func init() {
	for kind, info := range nativeConfigKinds() {
		rustKinds[kind] = info
	}
	for kind, info := range corpusKinds() {
		rustKinds[kind] = info
	}
}

func kindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs: []string{"crate_root"},
		NonEmptyAttrs: map[string]bool{
			"crate": true,
			"srcs":  true,
		},
		SubstituteAttrs: map[string]bool{"crate": true},
		MergeableAttrs: map[string]bool{
			"aliases":      true,
			"aspect_hints": true,
			"crate":        true,
			"crate_name":   true,
			"crate_root":   true,
			"srcs":         true,
		},
		ResolveAttrs: map[string]bool{"aliases": true, "deps": true},
	}
}

type rustLang struct {
	language.BaseLang
	errors         []common.GenerationError
	ignores        []*common.IgnoreEntry
	generated      common.GeneratedRels
	workspaceRoots map[string]*cargoManifest
}

type rustConfig struct {
	common.IgnoreConfig
	tools  []string
	corpus bool
}

type targetImports struct {
	production  []string
	test        []string
	mirrorPaths []string
	siblingLib  string
	scriptDep   string
	renames     map[string]string
	gated       map[string]bool
	gateTarget  map[string]string
	// inheritedDeps are dependency labels of the existing crate rule that a
	// self-contained unit test no longer inherits through crate=.
	inheritedDeps []string
}

func NewLanguage() language.Language { return &rustLang{} }

func (l *rustLang) Before(context.Context) {
	l.errors = nil
	l.ignores = nil
	l.generated = common.GeneratedRels{}
	common.Errors.Reset()
	l.workspaceRoots = nil
}

func (*rustLang) DoneGeneratingRules() {}

func (l *rustLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *rustLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*rustLang) KnownDirectives() []string {
	return []string{"dx_ignore_import", nativeToolsDirective, corpusDirective}
}

func (l *rustLang) Configure(c *config.Config, rel string, file *rule.File) {
	tools := defaultNativeTools()
	corpus := false
	if raw, ok := c.Exts[languageName]; ok {
		parent := raw.(*rustConfig)
		if parent.tools != nil {
			tools = parent.tools
		}
		corpus = parent.corpus
	}
	if file != nil {
		for _, directive := range file.Directives {
			if directive.Key == corpusDirective {
				enabled, err := parseCorpusDirective(directive.Value)
				if err != nil {
					l.fail(common.InPackage(rel), "%v", err)
					continue
				}
				corpus = enabled
				continue
			}
			if directive.Key != nativeToolsDirective {
				continue
			}
			selected, err := parseNativeToolsDirective(directive.Value)
			if err != nil {
				l.fail(common.InPackage(rel), "%v", err)
				continue
			}
			tools = selected
		}
	}
	declared, errs := common.DeclaredIgnores(languageName, rel, file)
	for _, msg := range errs {
		l.fail(common.InPackage(rel), "%s", msg)
	}
	l.ignores = append(l.ignores, declared...)
	inherited := common.MergeIgnores(common.InheritedIgnores(c, languageName), declared)
	c.Exts[languageName] = &rustConfig{IgnoreConfig: common.IgnoreConfig{Ignores: inherited}, tools: tools, corpus: corpus}
}

func corpusEnabled(c *config.Config) bool {
	if c == nil || c.Exts == nil {
		return false
	}
	cfg, ok := c.Exts[languageName].(*rustConfig)
	return ok && cfg.corpus
}

func (l *rustLang) fail(pkg common.PackageRef, format string, args ...interface{}) {
	l.errors = append(l.errors, common.GenerationError{Language: languageName, Package: pkg, Message: fmt.Sprintf(format, args...)})
}

func (l *rustLang) AfterResolvingDeps(context.Context) {
	l.errors = append(l.errors, common.StaleIgnores(languageName, l.ignores, l.generated)...)
	common.Errors.Add(l.errors...)
}

func (*rustLang) Name() string { return languageName }

func (*rustLang) Kinds() map[string]rule.KindInfo { return rustKinds }

func (*rustLang) Loads() []rule.LoadInfo {
	return rustLoads("rules_dx", "crates", "rules_rust")
}

func (l *rustLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	cratesName := moduleToApparentName("crates")
	if cratesName == "" {
		cratesName = "crates"
	}
	rulesRustName := moduleToApparentName("rules_rust")
	if rulesRustName == "" {
		rulesRustName = "rules_rust"
	}
	return rustLoads(repoName, cratesName, rulesRustName)
}

func rustLoads(rulesRepo, cratesRepo, rulesRustRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//rust/rules:defs.bzl", Symbols: []string{binaryKind, libraryKind, testKind, procMacroKind, sharedKind, staticKind}},
		{Name: "@" + rulesRepo + "//rust/rules:crate.bzl", Symbols: []string{dxCrateKind}},
		{Name: "@" + rulesRustRepo + "//cargo:defs.bzl", Symbols: []string{scriptKind}},
		{Name: "@" + cratesRepo + "//:crates.bzl", Symbols: []string{"aliases", "all_crate_deps", "crate_deps"}},
		nativeConfigLoads(rulesRepo),
		corpusLoads(rulesRepo),
	}
}

func (*rustLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() == dxCrateKind {
		crateName := r.AttrString("crate_name")
		if crateName == "" {
			crateName = r.Name()
		}
		return []resolve.ImportSpec{{Lang: languageName, Imp: crateName}}
	}
	if r.Kind() != libraryKind && r.Kind() != procMacroKind {
		return nil
	}
	crateName := r.AttrString("crate_name")
	if crateName == "" {
		crateName = r.Name()
	}
	return []resolve.ImportSpec{{Lang: languageName, Imp: crateName}}
}

func (*rustLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *rustLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	l.generated.Add(args.Rel)
	return l.generateRules(args)
}

func (l *rustLang) generateRules(args language.GenerateArgs) language.GenerateResult {
	plan, err := planNativeConfig(args.Config, args)
	if err != nil {
		l.fail(common.InPackage(args.Rel), "%v", err)
		return language.GenerateResult{}
	}
	files, err := sourceFiles(args.Dir, args.Rel)
	if err != nil {
		l.fail(common.InPackage(args.Rel), "rust: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	for _, name := range args.RegularFiles {
		if name == "Cargo.toml" {
			return l.generateCargo(args, files, plan)
		}
	}
	if len(files) == 0 {
		return l.attachNative(args, language.GenerateResult{}, plan)
	}

	roots := DiscoverCrateRoots(args.Rel, files)
	if !roots.HasRoots() {
		return l.attachNative(args, language.GenerateResult{}, plan)
	}
	read := func(name string) ([]byte, error) {
		return os.ReadFile(filepath.Join(args.Config.RepoRoot, filepath.FromSlash(name)))
	}
	existsSet := make(map[string]bool, len(files))
	for _, name := range files {
		existsSet[name] = true
	}
	exists := func(name string) bool { return existsSet[name] }

	trees := make(map[string]map[string]*FileFacts)
	owners := make(map[string]sourceOwner)
	for _, root := range append(append([]string{}, roots.LibRoot, roots.BinRoot), roots.TestRoots...) {
		if root == "" {
			continue
		}
		tree, loadErr := LoadCrate(root, read, exists)
		if loadErr != nil {
			l.fail(common.InPackage(args.Rel), "%v", loadErr)
			return language.GenerateResult{}
		}
		trees[root] = tree
		if err := claimSources(owners, root, root != roots.LibRoot && root != roots.BinRoot, tree); err != nil {
			l.fail(common.InPackage(args.Rel), "%v", err)
			return language.GenerateResult{}
		}
	}
	shape, err := ShapeCrate(roots, hasUnitTests(trees[roots.LibRoot]), hasUnitTests(trees[roots.BinRoot]))
	if err != nil {
		l.fail(common.InPackage(args.Rel), "%v", err)
		return language.GenerateResult{}
	}

	var result language.GenerateResult
	publicLibs := shouldSetVisibility(args)
	if shape.LibTarget != "" {
		tree := trees[roots.LibRoot]
		split := hasTestOnly(tree)
		r := crateRule(libraryKind, shape.LibTarget, shape.Name, roots.LibRoot, tree, args.Rel, split)
		if publicLibs {
			r.SetAttr("visibility", []string{"//visibility:public"})
		}
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, importsFor(tree))
		if shape.LibUnitTest {
			result.Gen = append(result.Gen, unitTestRule(common.UnitTestName(shape.LibTarget), shape.LibTarget, shape.Name, roots.LibRoot, tree, args.Rel, split))
			result.Imports = append(result.Imports, sourceOnlyTestImports(tree, split))
		}
	}
	if shape.BinTarget != "" {
		tree := trees[roots.BinRoot]
		split := hasTestOnly(tree)
		r := crateRule(binaryKind, shape.BinTarget, shape.Name, roots.BinRoot, tree, args.Rel, split)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, importsFor(tree))
		if shape.BinUnitTest {
			result.Gen = append(result.Gen, unitTestRule(common.UnitTestName(shape.BinTarget), shape.BinTarget, shape.Name, roots.BinRoot, tree, args.Rel, split))
			result.Imports = append(result.Imports, sourceOnlyTestImports(tree, split))
		}
	}
	for _, test := range shape.Tests {
		r := crateRule(testKind, test.Name, strings.TrimSuffix(path.Base(test.Root), ".rs"), test.Root, trees[test.Root], args.Rel, false)
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, importsFor(trees[test.Root]))
	}
	return l.attachNative(args, result, plan)
}

// sourceOnlyTestImports returns the imports of a source-only unit test. A
// self-contained test compiles the whole tree, so it needs the production
// imports too; a crate= wrapper inherits those from its crate.
func sourceOnlyTestImports(tree map[string]*FileFacts, selfContained bool) targetImports {
	imports := importsFor(tree)
	if selfContained {
		return imports
	}
	return targetImports{test: imports.test}
}

func (l *rustLang) attachNative(args language.GenerateArgs, result language.GenerateResult, plan *nativePlan) language.GenerateResult {
	result = filterDxCrateCovered(args.File, result)
	renamed := adoptExistingNames(args.File, result.Gen)
	inherited, err := inheritCrateAttrs(args.File, result.Gen)
	if err != nil {
		l.fail(common.InPackage(args.Rel), "%v", err)
		return language.GenerateResult{}
	}
	for i, r := range result.Gen {
		if labels := inherited[r]; len(labels) > 0 && i < len(result.Imports) {
			if current, ok := result.Imports[i].(targetImports); ok {
				current.inheritedDeps = labels
				result.Imports[i] = current
			}
		}
	}
	for i, raw := range result.Imports {
		current, ok := raw.(targetImports)
		if !ok {
			continue
		}
		if to, ok := renamed[current.siblingLib]; ok {
			current.siblingLib = to
		}
		if to, ok := renamed[current.scriptDep]; ok {
			current.scriptDep = to
		}
		result.Imports[i] = current
	}
	for _, r := range result.Gen {
		applyNativeHints(args.Rel, r, plan)
	}
	result.Gen = append(result.Gen, plan.gen...)
	result.Imports = append(result.Imports, plan.imports...)
	hasOtherGen := len(result.Gen) > 0 || args.File != nil
	corpus := &corpusPlan{}
	if corpusEnabled(args.Config) {
		corpus = planCorpus(args, hasOtherGen)
	}
	result.Gen = append(result.Gen, corpus.gen...)
	for range corpus.gen {
		result.Imports = append(result.Imports, targetImports{})
	}
	if isFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	if err := common.CheckClaims(args.File, args.OtherGen, generatedClaims(args.Rel, result.Gen), rustClaimAliases); err != nil {
		l.fail(common.InPackage(args.Rel), "rust: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	merged := mergeStale(args.File, result)
	merged.Empty = append(merged.Empty, plan.empty...)
	merged.Empty = append(merged.Empty, corpus.empty...)
	return merged
}

func dxCrateNames(file *rule.File) map[string]bool {
	names := make(map[string]bool)
	if file == nil {
		return names
	}
	for _, existing := range file.Rules {
		if existing.Kind() == dxCrateKind {
			names[existing.Name()] = true
		}
	}
	return names
}

func filterDxCrateCovered(file *rule.File, result language.GenerateResult) language.GenerateResult {
	covered := dxCrateNames(file)
	if len(covered) == 0 || len(result.Gen) == 0 {
		return result
	}
	keptGen := result.Gen[:0]
	keptImports := result.Imports[:0]
	for i, r := range result.Gen {
		if r.Kind() == libraryKind && covered[r.Name()] {
			continue
		}
		if of := unitTestOf(r); of != "" && covered[of] && r.Name() == common.UnitTestName(of) {
			continue
		}
		if r.Kind() == testKind && r.Attr("crate") != nil {
			if crate := r.AttrString("crate"); len(crate) > 1 && crate[0] == ':' {
				if covered[crate[1:]] && r.Name() == crate[1:]+"_test" {
					continue
				}
			}
		}
		keptGen = append(keptGen, r)
		if i < len(result.Imports) {
			keptImports = append(keptImports, result.Imports[i])
		}
	}
	result.Gen = keptGen
	if len(result.Gen) == 0 {
		if len(keptImports) > len(keptGen) {
			keptImports = keptImports[:len(keptGen)]
		}
		result.Imports = keptImports
	} else {
		if len(result.Imports) != len(keptImports) {
			result.Imports = keptImports
		}
	}
	return result
}

func adoptExistingNames(file *rule.File, gen []*rule.Rule) map[string]string {
	if file == nil {
		return nil
	}
	taken := make(map[string]bool, len(gen))
	for _, r := range gen {
		taken[r.Name()] = true
	}
	// adopt renames r to the one existing rule of its kind that match
	// selects. It reports whether some existing rule matched at all.
	adopt := func(r *rule.Rule, match func(*rule.Rule) bool) bool {
		var found *rule.Rule
		for _, existing := range file.Rules {
			if existing.Kind() != r.Kind() || !match(existing) {
				continue
			}
			if existing.Name() == r.Name() {
				return true
			}
			if found != nil {
				return true
			}
			found = existing
		}
		if found == nil {
			return false
		}
		if taken[found.Name()] {
			return true
		}
		delete(taken, r.Name())
		taken[found.Name()] = true
		r.SetName(found.Name())
		return true
	}
	renamed := make(map[string]string)
	adopted := make(map[*rule.Rule]bool)
	for _, r := range gen {
		root := r.AttrString("crate_root")
		if root == "" {
			continue
		}
		old := r.Name()
		adopted[r] = adopt(r, func(existing *rule.Rule) bool { return existing.AttrString("crate_root") == root })
		if r.Name() != old {
			renamed[old] = r.Name()
		}
	}
	byName := make(map[string]*rule.Rule, len(gen))
	for _, r := range gen {
		byName[r.Name()] = r
	}
	for _, r := range gen {
		if r.Kind() != testKind {
			continue
		}
		of := unitTestOf(r)
		if target, ok := renamed[of]; ok {
			of = target
			r.SetPrivateAttr(unitTestOfAttr, of)
		}
		if r.AttrString("crate_root") != "" {
			// A self-contained unit test also adopts the crate= wrapper
			// it replaces.
			if of != "" && !adopted[r] {
				crate := ":" + of
				adopt(r, func(existing *rule.Rule) bool {
					return existing.AttrString("crate_root") == "" && existing.AttrString("crate") == crate
				})
			}
			continue
		}
		crate := r.AttrString("crate")
		if len(crate) < 2 || crate[0] != ':' {
			continue
		}
		if target, ok := renamed[crate[1:]]; ok {
			crate = ":" + target
			r.SetAttr("crate", crate)
		}
		if adopt(r, func(existing *rule.Rule) bool { return existing.AttrString("crate") == crate }) {
			continue
		}
		// A crate= unit test also adopts the self-contained test it
		// replaces: same crate root and crate name as its crate target.
		if lib := byName[crate[1:]]; lib != nil && of != "" {
			root, name := lib.AttrString("crate_root"), lib.AttrString("crate_name")
			adopt(r, func(existing *rule.Rule) bool {
				return existing.Attr("crate") == nil && root != "" &&
					existing.AttrString("crate_root") == root && existing.AttrString("crate_name") == name
			})
		}
	}
	return renamed
}

// crateInheritedAttrs are attributes a crate= unit test inherits from its
// crate target. A self-contained unit test has to carry them itself.
var crateInheritedAttrs = []string{"compile_data", "crate_features", "edition", "proc_macro_deps", "rustc_env", "rustc_env_files", "rustc_flags"}

// inheritCrateAttrs copies crateInheritedAttrs from each self-contained unit
// test's existing crate rule, unless the generated test already sets them.
// An existing test with a different value is an error: it would silently
// change what the test compiles. It also returns, per test, the literal deps
// labels of the existing crate rule, which include hand-maintained deps.
func inheritCrateAttrs(file *rule.File, gen []*rule.Rule) (map[*rule.Rule][]string, error) {
	if file == nil {
		return nil, nil
	}
	deps := make(map[*rule.Rule][]string)
	existing := make(map[string]*rule.Rule, len(file.Rules))
	for _, r := range file.Rules {
		existing[r.Name()] = r
	}
	for _, t := range gen {
		of := unitTestOf(t)
		if of == "" || t.AttrString("crate_root") == "" {
			continue
		}
		crate := existing[of]
		if crate == nil || (crate.Kind() != libraryKind && crate.Kind() != binaryKind) {
			continue
		}
		if labels := unmanagedDepsLabels(crate.Attr("deps"), nil); len(labels) > 0 {
			deps[t] = labels
		}
		var old *rule.Rule
		if prev := existing[t.Name()]; prev != nil && prev.Kind() == testKind {
			old = prev
		}
		for _, attr := range crateInheritedAttrs {
			value := crate.Attr(attr)
			if value == nil || t.Attr(attr) != nil {
				continue
			}
			if old != nil {
				if have := old.Attr(attr); have != nil {
					if bzl.FormatString(have) != bzl.FormatString(value) {
						return nil, fmt.Errorf("rust: %s: unit test %s sets %s = %s but its crate %s sets %s; a self-contained unit test no longer inherits it, so make them equal",
							file.Path, t.Name(), attr, bzl.FormatString(have), of, bzl.FormatString(value))
					}
					continue
				}
			}
			t.SetAttr(attr, value)
		}
	}
	return deps, nil
}
