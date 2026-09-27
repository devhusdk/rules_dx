package fsharp

import (
	"context"
	"flag"
	"fmt"
	"github.com/ralvik/rules_dx/gazelle/common"
	"sort"
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/repo"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"
)

const languageName = "fsharp"

var fsharpKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

type fsharpLang struct {
	language.BaseLang
	errors  []string
	ignores []*ignoreEntry
}

type fsharpConfig struct {
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

func NewLanguage() language.Language { return &fsharpLang{} }

func (l *fsharpLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *fsharpLang) DoneGeneratingRules() {}

func (l *fsharpLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *fsharpLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*fsharpLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *fsharpLang) Configure(c *config.Config, rel string, f *rule.File) {
	var inherited []*ignoreEntry
	if raw, ok := c.Exts[languageName]; ok {
		inherited = append(inherited, raw.(*fsharpConfig).ignores...)
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
				l.fail("fsharp: //%s: malformed # gazelle:dx_ignore_import %s", rel, directive.Value)
			}
		}
	}
	c.Exts[languageName] = &fsharpConfig{ignores: inherited}
}

func (l *fsharpLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *fsharpLang) AfterResolvingDeps(context.Context) {
	for _, ignore := range l.ignores {
		if !ignore.used {
			l.fail("fsharp: //%s: stale # gazelle:dx_ignore_import fsharp %s matches no literal reference", ignore.path, ignore.value)
		}
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("FSharp generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*fsharpLang) Name() string { return languageName }

func (*fsharpLang) Kinds() map[string]rule.KindInfo { return fsharpKinds }

func (*fsharpLang) Loads() []rule.LoadInfo {
	return fsharpLoads("rules_dx")
}

func (l *fsharpLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return fsharpLoads(repoName)
}

func fsharpLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//fsharp/rules:defs.bzl", Symbols: []string{LibraryKind}},
	}
}

func (*fsharpLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
	if r.Kind() != LibraryKind {
		return nil
	}
	var specs []resolve.ImportSpec
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) || IsTestSource(src) {
			continue
		}
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: ClassIdentity(src)})
	}
	return specs
}

func (*fsharpLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *fsharpLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
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

func (l *fsharpLang) generateRules(args language.GenerateArgs) language.GenerateResult {
	return common.GenerateSingleDir(args, fsharpKinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return isSupported(name) && !IsTestSource(name) },
		DirName:      DirTargetName,
		CheckPackage: true,
		ParsePackage: ParsePackage,
		DefinesMain:  DefinesMain,
		MainRule:     "fsharp_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		OrderSrcs:    orderSourcesByDependency,
		MakeRule: func(name string, sources []string, _ []string) *rule.Rule {
			r := rule.NewRule(LibraryKind, name)
			r.SetSortedAttrs([]string{"deps"})
			r.SetAttr("srcs", rule.UnsortedStrings(sources))
			if comments := r.AttrComments("srcs"); comments != nil {
				comments.Before = append(comments.Before, bzl.Comment{Token: "# do not sort: F# compile order, dependencies first"})
			}
			return r
		},
		Wrap: func(imports []string) any { return targetImports{imports: imports} },
	}, l)
}

func (l *fsharpLang) Fail(format string, args ...interface{}) { l.fail(format, args...) }

func (l *fsharpLang) Failed() bool { return len(l.errors) > 0 }

func orderSourcesByDependency(sources []string, contents map[string][]byte) ([]string, error) {
	identityToSrc := make(map[string]string, len(sources))
	for _, src := range sources {
		id := ClassIdentity(src)
		if _, ok := identityToSrc[id]; !ok {
			identityToSrc[id] = src
		}
	}
	deps := make(map[string]map[string]bool, len(sources))
	for _, src := range sources {
		deps[src] = make(map[string]bool)
		content, ok := contents[src]
		if !ok {
			continue
		}
		for _, imp := range ParseImports(content) {
			if IsStdLib(imp) {
				continue
			}
			if depSrc, ok := identityToSrc[imp]; ok && depSrc != src {
				deps[src][depSrc] = true
			}
		}
	}
	remaining := make(map[string]bool, len(sources))
	for _, src := range sources {
		remaining[src] = true
	}
	emitted := make(map[string]bool, len(sources))
	ordered := make([]string, 0, len(sources))
	for len(remaining) > 0 {
		var ready []string
		for src := range remaining {
			blocked := false
			for dep := range deps[src] {
				if !emitted[dep] {
					blocked = true
					break
				}
			}
			if !blocked {
				ready = append(ready, src)
			}
		}
		if len(ready) == 0 {
			cycle := make([]string, 0, len(remaining))
			for src := range remaining {
				cycle = append(cycle, src)
			}
			sort.Strings(cycle)
			return nil, fmt.Errorf("cyclic F# compile order among %s; split the directory or order sources by hand", strings.Join(cycle, ", "))
		}
		sort.Strings(ready)
		next := ready[0]
		ordered = append(ordered, next)
		emitted[next] = true
		delete(remaining, next)
	}
	return ordered, nil
}

func (l *fsharpLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
	common.ResolveSingle(c, ix, r, raw, from, common.ResolveSpec{
		LanguageName: languageName,
		WantKind:     LibraryKind,
		Attr:         "deps",
		Unwrap: func(raw any) ([]string, bool) {
			imports, ok := raw.(targetImports)
			if !ok {
				return nil, false
			}
			return imports.imports, true
		},
		IsStdLib: IsStdLib,
		MarkUsed: func(name string) bool {
			if ignore := matchingIgnore(c, name); ignore != nil {
				ignore.used = true
				return true
			}
			return false
		},
		FailConflict: func(from label.Label, name string) {
			l.fail("fsharp: %s: import %q has both an exact resolve mapping and ignore", from, name)
		},
		FailUnresolved: func(from label.Label, name string) {
			l.fail("fsharp: %s: unresolved import %q; add a local one-source library or an exact # gazelle:resolve mapping", from, name)
		},
		FailAmbiguous: func(from label.Label, name string, matches string) {
			l.fail("fsharp: %s: ambiguous import %q resolves to %s", from, name, matches)
		},
	})
}

func matchingIgnore(c *config.Config, name string) *ignoreEntry {
	raw, ok := c.Exts[languageName]
	if !ok {
		return nil
	}
	for i := len(raw.(*fsharpConfig).ignores) - 1; i >= 0; i-- {
		if entry := raw.(*fsharpConfig).ignores[i]; entry.value == name {
			return entry
		}
	}
	return nil
}

func CollectUsedIgnores(c *config.Config) [][2]string {
	raw, ok := c.Exts[languageName]
	if !ok || raw == nil {
		return nil
	}
	cfg, ok := raw.(*fsharpConfig)
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
