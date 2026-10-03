package python

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

const (
	languageName = "python"
	libraryKind  = "python_library"
	testKind     = "python_test"
	binaryKind   = "python_binary"
	importsAttr  = "."
)

var pythonKinds = map[string]rule.KindInfo{
	libraryKind: kindInfo(),
	testKind:    kindInfo(),
	binaryKind:  binaryKindInfo(),
}

func kindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs:    []string{"srcs"},
		NonEmptyAttrs: map[string]bool{"srcs": true},
		MergeableAttrs: map[string]bool{
			"srcs":    true,
			"imports": true,
			"deps":    true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

func binaryKindInfo() rule.KindInfo {
	return rule.KindInfo{
		MatchAttrs: []string{"main"},
		MergeableAttrs: map[string]bool{
			"imports": true,
			"deps":    true,
		},
		ResolveAttrs: map[string]bool{"deps": true},
	}
}

type pythonLang struct {
	language.BaseLang
	errors  []string
	ignores []*common.IgnoreEntry
}

type targetImports struct {
	imports []string
}

func NewLanguage() language.Language { return &pythonLang{} }

func (l *pythonLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *pythonLang) DoneGeneratingRules() {}

func (l *pythonLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *pythonLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*pythonLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *pythonLang) Configure(c *config.Config, rel string, f *rule.File) {
	declared, errs := common.DeclaredIgnores(languageName, rel, f)
	for _, msg := range errs {
		l.fail("%s", msg)
	}
	l.ignores = append(l.ignores, declared...)
	c.Exts[languageName] = &common.IgnoreConfig{Ignores: common.MergeIgnores(common.InheritedIgnores(c, languageName), declared)}
}

func (l *pythonLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *pythonLang) AfterResolvingDeps(context.Context) {
	for _, msg := range common.StaleIgnores(languageName, l.ignores) {
		l.fail("%s", msg)
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("Python generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*pythonLang) Name() string { return languageName }

func (*pythonLang) Kinds() map[string]rule.KindInfo { return pythonKinds }

func (*pythonLang) Loads() []rule.LoadInfo {
	return pythonLoads("rules_dx")
}

func (l *pythonLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return pythonLoads(repoName)
}

func pythonLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//python/rules:defs.bzl", Symbols: []string{libraryKind, testKind, binaryKind}},
	}
}

func (*pythonLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != libraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !strings.HasSuffix(src, ".py") {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: ModuleName(src)})
	}
	return specs
}

func (*pythonLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *pythonLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	return l.generateRules(args)
}

func (l *pythonLang) generateRules(args language.GenerateArgs) language.GenerateResult {
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
	if len(sources) == 0 {
		return mergeStale(args.File, language.GenerateResult{})
	}

	type plan struct {
		name    string
		src     string
		stub    string
		test    bool
		entry   bool
		imports []string
	}
	var plans []plan
	for _, src := range sources {
		content, err := os.ReadFile(filepath.Join(args.Dir, src))
		if err != nil {
			l.fail("python: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		name, err := TargetName(src)
		if err != nil {
			l.fail("python: %s: %v", args.Rel, err)
			continue
		}
		p := plan{name: name, src: src, test: IsTestFile(src)}
		p.entry = !p.test && IsEntryFile(src)
		if stub := strings.TrimSuffix(src, ".py") + ".pyi"; stubs[stub] {
			p.stub = stub
		}
		seen := make(map[string]bool)
		for _, root := range ParseImports(content) {
			if IsStdLib(root) || seen[root] {
				continue
			}
			seen[root] = true
			p.imports = append(p.imports, root)
		}
		sort.Strings(p.imports)
		plans = append(plans, p)
	}
	if len(l.errors) > 0 {
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
			claimants = append(claimants, Claimant{Name: EntryBinaryName(p.name), Source: p.src, Kind: binaryKind})
		}
	}
	if err := checkClaims(args.File, args.OtherGen, claimants); err != nil {
		l.fail("python: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	var result language.GenerateResult
	for _, p := range plans {
		kind := libraryKind
		if p.test {
			kind = testKind
		}
		r := rule.NewRule(kind, p.name)
		srcs := []string{p.src}
		if p.stub != "" {
			srcs = append(srcs, p.stub)
		}
		r.SetAttr("srcs", srcs)
		r.SetAttr("imports", []string{importsAttr})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, targetImports{imports: append([]string(nil), p.imports...)})
		if p.entry {
			bin := rule.NewRule(binaryKind, EntryBinaryName(p.name))
			bin.SetAttr("main", p.src)
			bin.SetAttr("imports", []string{importsAttr})
			bin.SetAttr("deps", []string{":" + p.name})
			result.Gen = append(result.Gen, bin)
			result.Imports = append(result.Imports, targetImports{})
		}
	}
	if isFixturePath(args.Rel) {
		for _, r := range result.Gen {
			r.SetAttr("testonly", true)
		}
	}
	return mergeStale(args.File, result)
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
	return common.CheckClaimsMulti(file, other, claimants, claimKind)
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, pythonKinds)
}

func (l *pythonLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	imports, ok := raw.(targetImports)
	if !ok {
		return
	}
	deps := make(map[string]bool)
	for _, name := range imports.imports {
		if IsStdLib(name) {
			continue
		}
		spec := resolve.ImportSpec{Lang: languageName, Imp: name}
		if override, found := resolve.FindRuleWithOverride(c, spec, languageName); found {
			if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
				ignore.Used = true
				l.fail("python: %s: import %q has both an exact resolve mapping and ignore", from, name)
				continue
			}
			deps[override.Rel(from.Repo, from.Pkg).String()] = true
			continue
		}
		matches := ix.FindRulesByImportWithConfig(c, spec, languageName)
		switch len(matches) {
		case 1:
			if matches[0].Label != from {
				deps[matches[0].Label.Rel(from.Repo, from.Pkg).String()] = true
			}
		case 0:
			if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
				ignore.Used = true
				continue
			}
			l.fail("python: %s: unresolved import %q; add a local one-source library or an exact # gazelle:resolve mapping", from, name)
		default:
			l.fail("python: %s: ambiguous import %q resolves to %s", from, name, formatMatches(matches))
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
	if r.Attr("deps") == nil {
		r.SetAttr("deps", labels)
		return
	}
	r.SetAttr("deps", unionStrings(r.AttrStrings("deps"), labels))
}

func unionStrings(a, b []string) []string { return common.UnionStrings(a, b) }

func formatMatches(matches []resolve.FindResult) string { return common.FormatMatches(matches) }

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }
