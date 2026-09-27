package golang

import (
	"context"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/ralvik/rules_dx/gazelle/common"
)

const languageName = "go"

var goKinds = map[string]rule.KindInfo{
	LibraryKind: rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":       true,
			"deps":       true,
			"importpath": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
	TestKind: rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":  true,
			"deps":  true,
			"embed": true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	},
}

type goLang struct {
	language.BaseLang
	errors  []string
	ignores []*ignoreEntry
}

type goConfig struct {
	ignores []*ignoreEntry
}

type ignoreEntry struct {
	value string
	path  string
	used  bool
}

type targetImports struct {
	imports []string
}

type testTargetImports struct {
	imports []string
}

func NewLanguage() language.Language { return &goLang{} }

func (l *goLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *goLang) DoneGeneratingRules() {}

func (l *goLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *goLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*goLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *goLang) Configure(c *config.Config, rel string, f *rule.File) {
	var inherited []*ignoreEntry
	if raw, ok := c.Exts[languageName]; ok {
		inherited = append(inherited, raw.(*goConfig).ignores...)
	}
	if f != nil {
		for _, directive := range f.Directives {
			if directive.Key != "dx_ignore_import" {
				continue
			}
			fields := strings.Fields(directive.Value)
			if len(fields) == 2 && fields[0] == languageName {
				entry := &ignoreEntry{value: fields[1], path: rel}
				inherited = append(inherited, entry)
				l.ignores = append(l.ignores, entry)
			} else if len(fields) == 3 && fields[0] == languageName && fields[1] == languageName {
				entry := &ignoreEntry{value: fields[2], path: rel}
				inherited = append(inherited, entry)
				l.ignores = append(l.ignores, entry)
			} else if len(fields) > 0 && fields[0] == languageName {
				l.fail("go: //%s: malformed # gazelle:dx_ignore_import %s", rel, directive.Value)
			}
		}
	}
	c.Exts[languageName] = &goConfig{ignores: inherited}
}

func (l *goLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *goLang) AfterResolvingDeps(context.Context) {
	for _, ignore := range l.ignores {
		if !ignore.used {
			l.fail("go: //%s: stale # gazelle:dx_ignore_import go %s matches no literal reference", ignore.path, ignore.value)
		}
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("Go generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*goLang) Name() string { return languageName }

func (*goLang) Kinds() map[string]rule.KindInfo { return goKinds }

func (*goLang) Loads() []rule.LoadInfo {
	return goLoads("rules_dx")
}

func (l *goLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return goLoads(repoName)
}

func goLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//go/rules:defs.bzl", Symbols: []string{LibraryKind, TestKind}},
	}
}

func (*goLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) || IsTestSource(src) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: ModuleName(src)})
	}
	return specs
}

func (*goLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *goLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	return l.generateRules(args)
}

func isSupported(name string) bool {
	for _, ext := range SupportedExts {
		if strings.HasSuffix(name, ext) {
			return true
		}
	}
	return false
}

func (l *goLang) generateRules(args language.GenerateArgs) language.GenerateResult {
	var sources []string
	var testSources []string
	for _, name := range args.RegularFiles {
		if !isSupported(name) {
			continue
		}
		if IsTestSource(name) {
			testSources = append(testSources, name)
			continue
		}
		sources = append(sources, name)
	}
	sort.Strings(sources)
	sort.Strings(testSources)
	if len(sources) == 0 && len(testSources) == 0 {
		return mergeStale(args.File, language.GenerateResult{})
	}
	if len(sources) == 0 {
		l.fail("go: %s: test sources %s without non-test sources; add the package sources to this directory or split the tests before adopting generation", args.Rel, strings.Join(testSources, ", "))
		return language.GenerateResult{}
	}

	name, err := DirTargetName(args.Rel)
	if err != nil {
		l.fail("go: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}
	testName := name + "_test"

	packages := make(map[string]bool)
	seen := make(map[string]bool)
	var imports []string
	var libPkg string
	for _, src := range sources {
		content, err := os.ReadFile(filepath.Join(args.Dir, src))
		if err != nil {
			l.fail("go: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		pkg, err := ParsePackage(content)
		if err != nil {
			l.fail("go: %s: parse package %s: %v", args.Rel, src, err)
			continue
		}
		if pkg == "main" {
			l.fail("go: %s: %s declares package main; thin go_binary entries stay handwritten, so split package-main sources into their own directory before adopting generation", args.Rel, src)
			continue
		}
		packages[pkg] = true
		libPkg = pkg
		roots, err := ParseImports(content)
		if err != nil {
			l.fail("go: %s: parse imports %s: %v", args.Rel, src, err)
			continue
		}
		for _, root := range roots {
			if IsCgoImport(root) {
				l.fail("go: %s: %s imports cgo (\"C\"); cgo stays handwritten, so split cgo sources into their own handwritten directory before adopting generation", args.Rel, src)
				continue
			}
			if IsStdLib(root) || seen[root] {
				continue
			}
			seen[root] = true
			imports = append(imports, root)
		}
	}
	if len(l.errors) > 0 {
		return language.GenerateResult{}
	}
	if len(packages) > 1 {
		names := make([]string, 0, len(packages))
		for pkg := range packages {
			names = append(names, pkg)
		}
		sort.Strings(names)
		l.fail("go: %s: mixed packages %s in one directory; split the directory before adopting generation", args.Rel, strings.Join(names, ", "))
		return language.GenerateResult{}
	}
	for pkg := range packages {
		libPkg = pkg
	}
	sort.Strings(imports)

	testSeen := make(map[string]bool)
	var testImports []string
	for _, src := range testSources {
		content, err := os.ReadFile(filepath.Join(args.Dir, src))
		if err != nil {
			l.fail("go: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		pkg, err := ParsePackage(content)
		if err != nil {
			l.fail("go: %s: parse package %s: %v", args.Rel, src, err)
			continue
		}
		if pkg != libPkg && pkg != libPkg+"_test" {
			l.fail("go: %s: %s declares package %s, want %s or %s_test; split the directory before adopting generation", args.Rel, src, pkg, libPkg, libPkg)
			continue
		}
		roots, err := ParseImports(content)
		if err != nil {
			l.fail("go: %s: parse imports %s: %v", args.Rel, src, err)
			continue
		}
		for _, root := range roots {
			if IsCgoImport(root) {
				l.fail("go: %s: %s imports cgo (\"C\"); cgo stays handwritten, so split cgo sources into their own handwritten directory before adopting generation", args.Rel, src)
				continue
			}
			if IsStdLib(root) || seen[root] || testSeen[root] {
				continue
			}
			testSeen[root] = true
			testImports = append(testImports, root)
		}
	}
	if len(l.errors) > 0 {
		return language.GenerateResult{}
	}
	sort.Strings(testImports)

	claimants := []Claimant{{Name: name, Source: args.Rel, Kind: LibraryKind}}
	if len(testSources) > 0 {
		claimants = append(claimants, Claimant{Name: testName, Source: args.Rel, Kind: TestKind})
	}
	if err := checkClaims(args.File, args.OtherGen, claimants); err != nil {
		l.fail("go: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	result := language.GenerateResult{}
	r := rule.NewRule(LibraryKind, name)
	r.SetAttr("srcs", sources)
	if importpath, ok := goImportPath(args.Config.RepoRoot, args.Dir); ok {
		r.SetAttr("importpath", importpath)
	}
	result.Gen = append(result.Gen, r)
	result.Imports = append(result.Imports, targetImports{imports: imports})
	if len(testSources) > 0 {
		t := rule.NewRule(TestKind, testName)
		t.SetAttr("srcs", testSources)
		t.SetAttr("embed", []string{":" + name})
		result.Gen = append(result.Gen, t)
		result.Imports = append(result.Imports, testTargetImports{imports: testImports})
	}
	if isFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
}

func checkClaims(file *rule.File, other []*rule.Rule, claimants []Claimant) error {
	return common.CheckClaimsPerKind(file, other, claimants)
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, goKinds)
}

func goImportPath(repoRoot, dir string) (string, bool) {
	cleanDir := filepath.Clean(dir)
	cleanRoot := filepath.Clean(repoRoot)
	for cur := cleanDir; ; cur = filepath.Dir(cur) {
		candidate := filepath.Join(cur, "go.mod")
		if content, err := os.ReadFile(candidate); err == nil {
			module, ok := parseGoModule(string(content))
			if !ok {
				return "", false
			}
			rel, err := filepath.Rel(cur, cleanDir)
			if err != nil {
				return "", false
			}
			slash := filepath.ToSlash(rel)
			if slash == "." || slash == "" {
				return module, true
			}
			return module + "/" + slash, true
		}
		if cur == cleanRoot || cur == filepath.Dir(cur) {
			return "", false
		}
		if len(cur) < len(cleanRoot) || !strings.HasPrefix(cur, cleanRoot) {
			return "", false
		}
	}
}

func parseGoModule(content string) (string, bool) {
	for _, line := range strings.Split(content, "\n") {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "//") {
			continue
		}
		fields := strings.Fields(trimmed)
		if len(fields) >= 2 && fields[0] == "module" {
			path := strings.Trim(fields[1], "\"'")
			if path != "" {
				return path, true
			}
			return "", false
		}
	}
	return "", false
}

func (l *goLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	var roots []string
	isTest := false
	switch typed := raw.(type) {
	case targetImports:
		if r.Kind() != LibraryKind {
			return
		}
		roots = typed.imports
	case testTargetImports:
		if r.Kind() != TestKind {
			return
		}
		roots = typed.imports
		isTest = true
	default:
		return
	}
	deps := make(map[string]bool)
	for _, name := range roots {
		if IsStdLib(name) {
			continue
		}
		spec := resolve.ImportSpec{Lang: languageName, Imp: name}
		if override, found := resolve.FindRuleWithOverride(c, spec, languageName); found {
			if ignore := matchingIgnore(c, name); ignore != nil {
				ignore.used = true
				l.fail("go: %s: import %q has both an exact resolve mapping and ignore", from, name)
				continue
			}
			if isTest && override.Pkg == from.Pkg {
				continue
			}
			deps[override.Rel(from.Repo, from.Pkg).String()] = true
			continue
		}
		matches := ix.FindRulesByImportWithConfig(c, spec, languageName)
		switch len(matches) {
		case 1:
			if matches[0].Label == from {
				continue
			}
			if isTest && matches[0].Label.Pkg == from.Pkg {
				continue
			}
			deps[matches[0].Label.Rel(from.Repo, from.Pkg).String()] = true
		case 0:
			if ignore := matchingIgnore(c, name); ignore != nil {
				ignore.used = true
				continue
			}
			l.fail("go: %s: unresolved import %q; add a local one-source library or an exact # gazelle:resolve mapping", from, name)
		default:
			l.fail("go: %s: ambiguous import %q resolves to %s", from, name, formatMatches(matches))
		}
	}
	if len(deps) == 0 {
		return
	}
	labels := make([]string, 0, len(deps))
	for dep := range deps {
		labels = append(labels, dep)
	}
	sort.Strings(labels)
	r.SetAttr("deps", unionStrings(r.AttrStrings("deps"), labels))
}

func unionStrings(a, b []string) []string { return common.UnionStrings(a, b) }

func matchingIgnore(c *config.Config, name string) *ignoreEntry {
	raw, ok := c.Exts[languageName]
	if !ok {
		return nil
	}
	for i := len(raw.(*goConfig).ignores) - 1; i >= 0; i-- {
		if entry := raw.(*goConfig).ignores[i]; entry.value == name {
			return entry
		}
	}
	return nil
}

func formatMatches(matches []resolve.FindResult) string { return common.FormatMatches(matches) }

func CollectUsedIgnores(c *config.Config) [][2]string {
	raw, ok := c.Exts[languageName]
	if !ok || raw == nil {
		return nil
	}
	cfg, ok := raw.(*goConfig)
	if !ok || cfg == nil {
		return nil
	}
	seen := map[[2]string]bool{}
	var out [][2]string
	for _, ig := range cfg.ignores {
		if ig == nil || !ig.used {
			continue
		}
		key := [2]string{ig.path, ig.value}
		if seen[key] {
			continue
		}
		seen[key] = true
		out = append(out, key)
	}
	return out
}
