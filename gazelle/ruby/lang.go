package ruby

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
)

const languageName = "ruby"

var rubyKinds = map[string]rule.KindInfo{
	LibraryKind: common.LibraryKindInfo(),
}

type rubyLang struct {
	language.BaseLang
	errors  []string
	ignores []*ignoreEntry
}

type rubyConfig struct {
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

func NewLanguage() language.Language { return &rubyLang{} }

func (l *rubyLang) Before(context.Context) { l.errors = nil; l.ignores = nil }

func (l *rubyLang) DoneGeneratingRules() {}

func (l *rubyLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *rubyLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*rubyLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *rubyLang) Configure(c *config.Config, rel string, f *rule.File) {
	var inherited []*ignoreEntry
	if raw, ok := c.Exts[languageName]; ok {
		inherited = append(inherited, raw.(*rubyConfig).ignores...)
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
				l.fail("ruby: //%s: malformed # gazelle:dx_ignore_import %s", rel, directive.Value)
			}
		}
	}
	c.Exts[languageName] = &rubyConfig{ignores: inherited}
}

func (l *rubyLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *rubyLang) AfterResolvingDeps(context.Context) {
	for _, ignore := range l.ignores {
		if !ignore.used {
			l.fail("ruby: //%s: stale # gazelle:dx_ignore_import ruby %s matches no literal reference", ignore.path, ignore.value)
		}
	}
	if len(l.errors) == 0 {
		return
	}
	sort.Strings(l.errors)
	panic("Ruby generation failed:\n" + strings.Join(l.errors, "\n"))
}

func (*rubyLang) Name() string { return languageName }

func (*rubyLang) Kinds() map[string]rule.KindInfo { return rubyKinds }

func (*rubyLang) Loads() []rule.LoadInfo {
	return rubyLoads("rules_dx")
}

func (l *rubyLang) ApparentLoads(moduleToApparentName func(string) string) []rule.LoadInfo {
	repoName := moduleToApparentName("rules_dx")
	if repoName == "" {
		repoName = "rules_dx"
	}
	return rubyLoads(repoName)
}

func rubyLoads(rulesRepo string) []rule.LoadInfo {
	return []rule.LoadInfo{
		{Name: "@" + rulesRepo + "//ruby/rules:defs.bzl", Symbols: []string{LibraryKind}},
	}
}

func (*rubyLang) Imports(_ *config.Config, r *rule.Rule, _ *rule.File) []resolve.ImportSpec {
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

func (*rubyLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *rubyLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
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

func (l *rubyLang) generateRules(args language.GenerateArgs) language.GenerateResult {
	return common.GenerateSingleDir(args, rubyKinds, common.SingleDirSpec{
		LanguageName: languageName,
		LibraryKind:  LibraryKind,
		IsSource:     func(name string) bool { return isSupported(name) && !IsTestSource(name) },
		DirName:      DirTargetName,
		CheckPackage: true,
		ParsePackage: ParsePackage,
		DefinesMain:  DefinesMain,
		MainRule:     "ruby_binary",
		MainPhrase:   "main-bearing sources",
		ParseImports: ParseImports,
		IsStdLib:     IsStdLib,
		Wrap:         func(imports []string) any { return targetImports{imports: imports} },
	}, l)
}

func (l *rubyLang) Fail(format string, args ...interface{}) { l.fail(format, args...) }

func (l *rubyLang) Failed() bool { return len(l.errors) > 0 }

func (l *rubyLang) Resolve(c *config.Config, ix *resolve.RuleIndex, _ *repo.RemoteCache, r *rule.Rule, raw interface{}, from label.Label) {
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
			l.fail("ruby: %s: import %q has both an exact resolve mapping and ignore", from, name)
		},
		FailUnresolved: func(from label.Label, name string) {
			l.fail("ruby: %s: unresolved import %q; add a local one-source library or an exact # gazelle:resolve mapping", from, name)
		},
		FailAmbiguous: func(from label.Label, name string, matches string) {
			l.fail("ruby: %s: ambiguous import %q resolves to %s", from, name, matches)
		},
	})
}

func matchingIgnore(c *config.Config, name string) *ignoreEntry {
	raw, ok := c.Exts[languageName]
	if !ok {
		return nil
	}
	for i := len(raw.(*rubyConfig).ignores) - 1; i >= 0; i-- {
		if entry := raw.(*rubyConfig).ignores[i]; entry.value == name {
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
	cfg, ok := raw.(*rubyConfig)
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
