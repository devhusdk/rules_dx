package javascript

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
	languageName    = "javascript"
	libraryKind     = "javascript_library"
	testKind        = "javascript_test"
	plainTestKind   = "javascript_js_test"
	binaryKind      = "javascript_binary"
	rootNodeModules = "//:node_modules"
)

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

type javascriptLang struct {
	language.BaseLang
	errors  []string
	ignores []*common.IgnoreEntry
}

type targetImports struct {
	imports []string
	local   map[string]bool
}

func NewLanguage() language.Language { return &javascriptLang{} }

func (l *javascriptLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *javascriptLang) DoneGeneratingRules() {}

func (l *javascriptLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *javascriptLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*javascriptLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *javascriptLang) Configure(c *config.Config, rel string, f *rule.File) {
	declared, errs := common.DeclaredIgnores(languageName, rel, f)
	for _, msg := range errs {
		l.fail("%s", msg)
	}
	l.ignores = append(l.ignores, declared...)
	c.Exts[languageName] = &common.IgnoreConfig{Ignores: common.MergeIgnores(common.InheritedIgnores(c, languageName), declared)}
}

func (l *javascriptLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *javascriptLang) AfterResolvingDeps(context.Context) {
	for _, msg := range common.StaleIgnores(languageName, l.ignores) {
		l.fail("%s", msg)
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("JavaScript generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*javascriptLang) Name() string { return languageName }

func (*javascriptLang) Kinds() map[string]rule.KindInfo { return javascriptKinds }

func (*javascriptLang) Loads() []rule.LoadInfo {
	return javascriptLoads("rules_dx")
}

func (l *javascriptLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return javascriptLoads(repoName)
}

func javascriptLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//javascript/rules:defs.bzl", Symbols: []string{libraryKind, testKind, binaryKind}},
	}
}

func (*javascriptLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != libraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: ModuleName(src)})
	}
	return specs
}

func (*javascriptLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *javascriptLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
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

func (l *javascriptLang) generateRules(args language.GenerateArgs) language.GenerateResult {
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
		imports []string
		local   map[string]bool
	}
	var plans []plan
	for _, src := range sources {
		content, err := os.ReadFile(filepath.Join(args.Dir, src))
		if err != nil {
			l.fail("javascript: %s: read %s: %v", args.Rel, src, err)
			continue
		}
		name, err := TargetName(src)
		if err != nil {
			l.fail("javascript: %s: %v", args.Rel, err)
			continue
		}
		p := plan{name: name, src: src, test: IsTestFile(src)}
		p.entry = !p.test && IsEntryFile(src)
		seen := make(map[string]bool)
		p.local = make(map[string]bool)
		for _, ref := range common.ParseImportRefs(content) {
			if seen[ref.Root] {
				if ref.Relative {
					p.local[ref.Root] = true
				}
				continue
			}
			if !ref.Relative && common.IsNodeBuiltin(ref.Root) {
				continue
			}
			seen[ref.Root] = true
			if ref.Relative {
				p.local[ref.Root] = true
			}
			p.imports = append(p.imports, ref.Root)
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
			claimants = append(claimants, Claimant{Name: common.BinaryName(p.name), Source: p.src, Kind: binaryKind})
		}
	}
	if err := checkClaims(args.File, args.OtherGen, claimants); err != nil {
		l.fail("javascript: %s: %v", args.Rel, err)
		return language.GenerateResult{}
	}

	plain := handPlainTests(args.File, args.OtherGen)
	var result language.GenerateResult
	for _, p := range plans {
		if p.test {
			if plain[p.name] {
				continue
			}
			r := rule.NewRule(testKind, p.name)
			r.SetAttr("srcs", []string{p.src})
			r.SetAttr("node_modules", rootNodeModules)
			result.Gen = append(result.Gen, r)
			result.Imports = append(result.Imports, targetImports{imports: append([]string(nil), p.imports...), local: p.local})
			continue
		}
		r := rule.NewRule(libraryKind, p.name)
		r.SetAttr("srcs", []string{p.src})
		result.Gen = append(result.Gen, r)
		result.Imports = append(result.Imports, targetImports{imports: append([]string(nil), p.imports...), local: p.local})
		if p.entry {
			bin := rule.NewRule(binaryKind, common.BinaryName(p.name))
			bin.SetAttr("entry_point", p.src)
			bin.SetAttr("data", []string{":" + p.name})
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
	return common.CheckClaimsMulti(plainTestsAsTests(file), plainTestsAsTestsOther(other), claimants, claimKind)
}

func handPlainTests(file *rule.File, other []*rule.Rule) map[string]bool {
	owned := make(map[string]bool)
	if file != nil {
		for _, r := range file.Rules {
			if r.Kind() == plainTestKind {
				owned[r.Name()] = true
			}
		}
	}
	for _, r := range other {
		if r.Kind() == plainTestKind {
			owned[r.Name()] = true
		}
	}
	return owned
}

func plainTestsAsTests(file *rule.File) *rule.File {
	if file == nil {
		return nil
	}
	view := rule.EmptyFile(file.Path, file.Pkg)
	for _, r := range file.Rules {
		kind := r.Kind()
		if kind == plainTestKind {
			kind = testKind
		}
		view.Rules = append(view.Rules, rule.NewRule(kind, r.Name()))
	}
	return view
}

func plainTestsAsTestsOther(other []*rule.Rule) []*rule.Rule {
	out := make([]*rule.Rule, 0, len(other))
	for _, r := range other {
		kind := r.Kind()
		if kind == plainTestKind {
			kind = testKind
		}
		out = append(out, rule.NewRule(kind, r.Name()))
	}
	return out
}

func isFixturePath(rel string) bool { return common.IsFixturePath(rel) }

func mergeStale(file *rule.File, result language.GenerateResult) language.GenerateResult {
	return common.MergeStale(file, result, javascriptKinds)
}

func (l *javascriptLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	imports, ok := raw.(targetImports)
	if !ok {
		return
	}
	if r.Kind() == binaryKind {
		return
	}
	resolveAttr := "deps"
	if r.Kind() == testKind {
		resolveAttr = "data"
	}
	deps := make(map[string]bool)
	for _, name := range imports.imports {
		if !imports.local[name] && common.IsNodeBuiltin(name) {
			continue
		}
		spec := resolve.ImportSpec{Lang: languageName, Imp: name}
		if override, found := resolve.FindRuleWithOverride(c, spec, languageName); found {
			if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
				ignore.Used = true
				l.fail("javascript: %s: import %q has both an exact resolve mapping and ignore", from, name)
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
			l.fail("javascript: %s: unresolved import %q; add a local one-source library or an exact # gazelle:resolve mapping", from, name)
		default:
			l.fail("javascript: %s: ambiguous import %q resolves to %s", from, name, formatMatches(matches))
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
	if r.Attr(resolveAttr) == nil {
		r.SetAttr(resolveAttr, unionStrings(r.AttrStrings(resolveAttr), labels))
		return
	}
	r.SetAttr(resolveAttr, unionStrings(r.AttrStrings(resolveAttr), labels))
}

func unionStrings(a, b []string) []string { return common.UnionStrings(a, b) }

func formatMatches(matches []resolve.FindResult) string { return common.FormatMatches(matches) }

func CollectUsedIgnores(c *config.Config) [][2]string { return common.UsedIgnores(c, languageName) }
