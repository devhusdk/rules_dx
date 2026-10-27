package fsharp

import (
	"fmt"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
	bzl "github.com/bazelbuild/buildtools/build"
)

const languageName = "fsharp"

var SupportedExts = []string{".fs", ".fsi"}

const LibraryKind = "fsharp_library"

func NewLanguage() language.Language {
	return common.NewClassLang(common.ClassLangSpec{
		LanguageName:   languageName,
		LibraryKind:    LibraryKind,
		MainRule:       "fsharp_binary",
		DefsPackage:    "fsharp/rules:defs.bzl",
		UnresolvedHint: "add a local one-source library or an exact # gazelle:resolve mapping",
		Exts:           SupportedExts,
		TestSuffixes:   []string{"Test"},
		ClassIDs:       true,
		ParsePackage:   ParsePackage,
		DefinesMain:    DefinesMain,
		ParseImports:   ParseImports,
		IsStdLib:       IsStdLib,
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
	})
}

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
			id := common.ClassIdentity(src, SupportedExts)
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
