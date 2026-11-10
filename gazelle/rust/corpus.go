package rust

import (
	"fmt"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

const corpusKind = "real_source_target"

const corpusDirective = "dx_rust_corpus"

func isGeneratedCorpusName(name string) bool {
	switch name {
	case "corpus_markdown", "corpus_starlark", "corpus_toml", "corpus_json":
		return true
	}
	return false
}

const corpusTag = "corpus"

func corpusKinds() map[string]rule.KindInfo {
	return map[string]rule.KindInfo{
		corpusKind: {
			MatchAttrs: []string{"name"},
			NonEmptyAttrs: map[string]bool{
				"markdown_srcs":     true,
				"starlark_srcs":     true,
				"toml_srcs":         true,
				"json_srcs":         true,
				"markdown_siblings": true,
			},
			MergeableAttrs: map[string]bool{
				"markdown_srcs":     true,
				"starlark_srcs":     true,
				"toml_srcs":         true,
				"json_srcs":         true,
				"markdown_siblings": true,
				"aspect_hints":      true,
				"tags":              true,
			},
		},
	}
}

func corpusLoads(rulesRepo string) rule.LoadInfo {
	return rule.LoadInfo{
		Name:    "@" + rulesRepo + "//core/quality:fixtures.bzl",
		Symbols: []string{"real_source_target"},
	}
}

type corpusPlan struct {
	gen   []*rule.Rule
	errs  []error
}

func planCorpus(args language.GenerateArgs, hasOtherGen bool) *corpusPlan {
	plan := &corpusPlan{}
	rel := args.Rel
	if rel == "node_modules" || strings.Contains(rel, "/node_modules/") || strings.HasPrefix(rel, "node_modules/") {
		return plan
	}
	if args.File == nil && !hasOtherGen {
		return plan
	}
	existing := map[string]*rule.Rule{}
	if args.File != nil {
		for _, r := range args.File.Rules {
			existing[r.Name()] = r
		}
	}
	fixtureOwned := map[string]bool{}
	for _, r := range existing {
		if r.Kind() != corpusKind {
			continue
		}
		name := r.Name()
		if strings.HasPrefix(name, "corpus_") {
			continue
		}
		for _, attr := range []string{"javascript_srcs", "jsx_srcs", "typescript_srcs", "tsx_srcs", "json_srcs", "python_srcs", "python_stub_srcs", "rust_srcs", "starlark_srcs", "toml_srcs", "markdown_srcs"} {
			for _, src := range r.AttrStrings(attr) {
				if src == "" || strings.Contains(src, "*") || strings.HasPrefix(src, "//") || strings.HasPrefix(src, "@") || strings.HasPrefix(src, ":") {
					continue
				}
				fixtureOwned[src] = true
			}
		}
	}
	siblingSet := map[string]bool{}
	if r, ok := existing["corpus_markdown"]; ok && r.Kind() == corpusKind {
		for _, s := range r.AttrStrings("markdown_siblings") {
			siblingSet[s] = true
		}
	}
	localSiblings := map[string]bool{}
	for s := range siblingSet {
		if s == "" || strings.HasPrefix(s, "//") || strings.HasPrefix(s, "@") || strings.HasPrefix(s, ":") || strings.Contains(s, "*") {
			continue
		}
		localSiblings[s] = true
	}
	collected := map[string][]string{
		"markdown": {},
		"starlark": {},
		"toml":     {},
		"json":     {},
	}
	seen := map[string]bool{}
	add := func(typ, pkgRel string) {
		if pkgRel == "" || seen[typ+"\x00"+pkgRel] {
			return
		}
		if fixtureOwned[pkgRel] || localSiblings[pkgRel] {
			return
		}
		seen[typ+"\x00"+pkgRel] = true
		collected[typ] = append(collected[typ], pkgRel)
	}
	root := args.Dir
	_ = filepath.WalkDir(root, func(name string, entry os.DirEntry, err error) error {
		if err != nil {
			plan.errs = append(plan.errs, fmt.Errorf("rust: cannot read corpus directory %s: %w", name, err))
			return nil
		}
		if entry.IsDir() {
			if name == root {
				return nil
			}
			base := entry.Name()
			if strings.HasPrefix(base, ".") || base == "bazel-bin" || base == "bazel-out" || base == "bazel-testlogs" || base == "node_modules" {
				return filepath.SkipDir
			}
			if _, statErr := os.Stat(filepath.Join(name, "BUILD.bazel")); statErr == nil {
				return filepath.SkipDir
			}
			if _, statErr := os.Stat(filepath.Join(name, "BUILD")); statErr == nil {
				return filepath.SkipDir
			}
			return nil
		}
		local, relErr := filepath.Rel(root, name)
		if relErr != nil {
			return nil
		}
		slash := filepath.ToSlash(local)
		if strings.HasPrefix(slash, "paket.") && strings.HasSuffix(slash, ".bzl") {
			return nil
		}
		if strings.HasSuffix(slash, ".lock") {
			return nil
		}
		base := path.Base(slash)
		if strings.HasPrefix(base, ".") {
			return nil
		}
		if strings.Contains(slash, ".local.") {
			return nil
		}
		switch {
		case base == "BUILD.bazel" || base == "MODULE.bazel" || strings.HasSuffix(base, ".bzl"):
			add("starlark", slash)
		case strings.HasSuffix(base, ".md"):
			add("markdown", slash)
		case strings.HasSuffix(base, ".toml"):
			add("toml", slash)
		case strings.HasSuffix(base, ".json"):
			return nil
		}
		return nil
	})
	if r, ok := existing["corpus_json"]; ok && r.Kind() == corpusKind {
		for _, src := range r.AttrStrings("json_srcs") {
			if src == "" || strings.Contains(src, "*") || strings.HasPrefix(src, "//") || strings.HasPrefix(src, "@") || strings.HasPrefix(src, ":") {
				continue
			}
			if fixtureOwned[src] || localSiblings[src] {
				continue
			}
			if !seen["json\x00"+src] {
				seen["json\x00"+src] = true
				collected["json"] = append(collected["json"], src)
			}
		}
	}
	for _, typ := range collected {
		sort.Strings(typ)
	}
	if !seen["starlark\x00BUILD.bazel"] {
		if _, ok := fixtureOwned["BUILD.bazel"]; !ok {
			if _, ok := localSiblings["BUILD.bazel"]; !ok {
				collected["starlark"] = append(collected["starlark"], "BUILD.bazel")
				sort.Strings(collected["starlark"])
			}
		}
	}
	siblings := make([]string, 0, len(siblingSet))
	for s := range siblingSet {
		siblings = append(siblings, s)
	}
	sort.Strings(siblings)
	onlySelf := len(collected["markdown"]) == 0 && len(siblings) == 0 &&
		len(collected["toml"]) == 0 && len(collected["json"]) == 0 &&
		len(collected["starlark"]) == 1 && collected["starlark"][0] == "BUILD.bazel"
	if onlySelf && args.File == nil && !hasOtherGen {
		return plan
	}
	emit := func(typ, attr string, srcs []string) {
		if len(srcs) == 0 {
			return
		}
		r := rule.NewRule(corpusKind, "corpus_"+typ)
		r.SetAttr(attr, srcs)
		r.SetAttr("tags", []string{corpusTag})
		if typ == "markdown" {
			if len(siblings) > 0 {
				r.SetAttr("markdown_siblings", siblings)
			}
		}
		plan.gen = append(plan.gen, r)
	}
	emit("markdown", "markdown_srcs", collected["markdown"])
	if len(collected["markdown"]) == 0 && len(siblings) > 0 {
		r := rule.NewRule(corpusKind, "corpus_markdown")
		r.SetAttr("markdown_siblings", siblings)
		r.SetAttr("tags", []string{corpusTag})
		plan.gen = append(plan.gen, r)
	}
	emit("starlark", "starlark_srcs", collected["starlark"])
	emit("toml", "toml_srcs", collected["toml"])
	emit("json", "json_srcs", collected["json"])
	return plan
}
