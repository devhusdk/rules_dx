package fsharp

import (
	"context"
	"flag"
	"fmt"
	"github.com/devhusdk/rules_dx/gazelle/common"
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
	errors    []string
	ignores   []*common.IgnoreEntry
	generated common.GeneratedRels
}

type targetImports struct {
	imports []string
}

func NewLanguage() language.Language { return &fsharpLang{} }

func (l *fsharpLang) Before(context.Context) {
	l.errors = nil
	l.ignores = nil
	l.generated = common.GeneratedRels{}
	common.Errors.Reset()
}

func (l *fsharpLang) DoneGeneratingRules() {}

func (l *fsharpLang) RegisterFlags(*flag.FlagSet, string, *config.Config) {}

func (l *fsharpLang) CheckFlags(*flag.FlagSet, *config.Config) error { return nil }

func (*fsharpLang) KnownDirectives() []string { return []string{"dx_ignore_import"} }

func (l *fsharpLang) Configure(c *config.Config, rel string, f *rule.File) {
	declared, errs := common.DeclaredIgnores(languageName, rel, f)
	for _, msg := range errs {
		l.fail("%s", msg)
	}
	l.ignores = append(l.ignores, declared...)
	c.Exts[languageName] = &common.IgnoreConfig{Ignores: common.MergeIgnores(common.InheritedIgnores(c, languageName), declared)}
}

func (l *fsharpLang) fail(format string, args ...interface{}) {
	l.errors = append(l.errors, fmt.Sprintf(format, args...))
}

func (l *fsharpLang) AfterResolvingDeps(context.Context) {
	for _, msg := range common.StaleIgnores(languageName, l.ignores, l.generated) {
		l.fail("%s", msg)
	}
	common.Errors.Add(languageName, "", l.errors...)
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
	seen := make(map[string]bool)
	for _, src := range r.AttrStrings("srcs") {
		if !isSupported(src) || IsTestSource(src) {
			continue
		}
		id := ClassIdentity(src)
		if seen[id] {
			continue
		}
		seen[id] = true
		specs = append(specs, resolve.ImportSpec{Lang: languageName, Imp: id})
	}
	return specs
}

func (*fsharpLang) Embeds(*rule.Rule, label.Label) []label.Label { return nil }

func (l *fsharpLang) GenerateRules(args language.GenerateArgs) language.GenerateResult {
	l.generated.Add(args.Rel)
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
		OrderSrcs: func(_ string, sources []string, contents map[string][]byte) ([]string, error) {
			return orderSourcesByDependency(sources, contents)
		},
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

type sourceNode struct {
	stem     string
	files    []string
	declared map[string]bool
	deps     map[*sourceNode]string
}

func (n *sourceNode) first() string { return n.files[0] }

func orderSourcesByDependency(sources []string, contents map[string][]byte) ([]string, error) {
	byStem := make(map[string]*sourceNode, len(sources))
	var nodes []*sourceNode
	owner := make(map[string]*sourceNode, len(sources))
	for _, src := range sources {
		stem := common.Stem(src, SupportedExts)
		n, ok := byStem[stem]
		if !ok {
			id := ClassIdentity(src)
			n = &sourceNode{stem: stem, declared: map[string]bool{id: true}, deps: make(map[*sourceNode]string)}
			byStem[stem] = n
			nodes = append(nodes, n)
			if _, taken := owner[id]; !taken {
				owner[id] = n
			}
		}
		n.files = append(n.files, src)
	}
	for _, n := range nodes {
		sort.SliceStable(n.files, func(i, j int) bool {
			return strings.HasSuffix(n.files[i], ".fsi") && !strings.HasSuffix(n.files[j], ".fsi")
		})
		if last := n.files[len(n.files)-1]; strings.HasSuffix(last, ".fsi") {
			return nil, fmt.Errorf("%s has no implementation %s.fs", last, n.stem)
		}
	}
	declaredBy := make(map[string][]*sourceNode)
	for _, n := range nodes {
		for _, src := range n.files {
			for _, name := range ParseDeclaredNames(contents[src]) {
				if n.declared[name] {
					continue
				}
				n.declared[name] = true
				declaredBy[name] = append(declaredBy[name], n)
			}
		}
	}
	for _, n := range nodes {
		for _, src := range n.files {
			if full := TopModuleName(contents[src]); strings.Contains(full, ".") {
				if _, taken := owner[full]; !taken {
					owner[full] = n
				}
				n.declared[full] = true
			}
		}
	}
	for name, decl := range declaredBy {
		if _, ok := owner[name]; !ok && len(decl) == 1 {
			owner[name] = decl[0]
		}
	}
	for _, n := range nodes {
		for _, src := range n.files {
			content, ok := contents[src]
			if !ok {
				continue
			}
			pkg, _ := ParsePackage(content)
			names := ParseReferencedNames(content, pkg)
			for _, path := range ParseOpenPaths(content) {
				names = append(names, localOpenKeys(path, pkg)...)
			}
			sort.Strings(names)
			for _, name := range names {
				dep, ok := owner[name]
				if !ok || dep == n || n.declared[name] {
					continue
				}
				if _, seen := n.deps[dep]; !seen {
					n.deps[dep] = name
				}
			}
		}
	}
	sort.Slice(nodes, func(i, j int) bool { return nodes[i].first() < nodes[j].first() })
	emitted := make(map[*sourceNode]bool, len(nodes))
	ordered := make([]string, 0, len(sources))
	for len(ordered) < len(sources) {
		var next *sourceNode
		for _, n := range nodes {
			if emitted[n] {
				continue
			}
			blocked := false
			for dep := range n.deps {
				if !emitted[dep] {
					blocked = true
					break
				}
			}
			if !blocked {
				next = n
				break
			}
		}
		if next == nil {
			return nil, cycleError(nodes, emitted)
		}
		ordered = append(ordered, next.files...)
		emitted[next] = true
	}
	return ordered, nil
}

func cycleError(nodes []*sourceNode, emitted map[*sourceNode]bool) error {
	var cycle []string
	edge := ""
	for _, n := range nodes {
		if emitted[n] {
			continue
		}
		cycle = append(cycle, n.files...)
		if edge != "" {
			continue
		}
		var blockers []*sourceNode
		for dep := range n.deps {
			if !emitted[dep] {
				blockers = append(blockers, dep)
			}
		}
		sort.Slice(blockers, func(i, j int) bool { return blockers[i].first() < blockers[j].first() })
		dep := blockers[0]
		edge = fmt.Sprintf("%s -> %s (%s)", n.first(), dep.first(), n.deps[dep])
	}
	sort.Strings(cycle)
	return fmt.Errorf("cyclic F# compile order among %s, first edge %s; split the directory or order sources by hand", strings.Join(cycle, ", "), edge)
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
			if ignore := common.MatchingIgnore(c, languageName, name); ignore != nil {
				ignore.MarkUsed(from.Pkg)
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
