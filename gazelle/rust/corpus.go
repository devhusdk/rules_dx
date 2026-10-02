package rust

import (
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

const corpusKind = "real_source_target"

const corpusValeHint = "//quality:corpus_vale_config"

const corpusBuildifierHint = "//:buildifier_config"

const corpusBiomeHint = "//:biome_config"

func rootBuildifierConfig(repoRoot string) bool {
	if repoRoot == "" {
		return false
	}
	_, err := os.Stat(filepath.Join(repoRoot, ".buildifier.json"))
	return err == nil
}

func rootBiomeConfig(repoRoot string) bool {
	if repoRoot == "" {
		return false
	}
	_, err := os.Stat(filepath.Join(repoRoot, "biome.json"))
	return err == nil
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
				"aspect_hints":      true,
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
		Name:    "@" + rulesRepo + "//quality:fixtures.bzl",
		Symbols: []string{"real_source_target"},
	}
}

type corpusPlan struct {
	gen   []*rule.Rule
	empty []*rule.Rule
}

func planCorpus(args language.GenerateArgs, hasOtherGen bool) *corpusPlan {
	plan := &corpusPlan{}
	rel := args.Rel
	if rel == ".opencode" || strings.HasPrefix(rel, ".opencode/") {
		return plan
	}
	if rel == "node_modules" || strings.Contains(rel, "/node_modules/") || strings.HasPrefix(rel, "node_modules/") {
		return plan
	}
	if rel == "tools/depcheck/testdata" || strings.HasPrefix(rel, "tools/depcheck/testdata/") {
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
		if name == "corpus" || strings.HasPrefix(name, "corpus_") {
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
	for _, name := range []string{"corpus", "corpus_markdown"} {
		if r, ok := existing[name]; ok && r.Kind() == corpusKind {
			for _, s := range r.AttrStrings("markdown_siblings") {
				siblingSet[s] = true
			}
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
		if slash == "paket.main.bzl" || slash == "paket.main_extension.bzl" {
			if rel == "third_party/dotnet/deps" {
				return nil
			}
		}
		if strings.HasSuffix(slash, ".lock") {
			return nil
		}
		base := path.Base(slash)
		if strings.HasPrefix(base, ".") && base != ".gitleaks.toml" {
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
	for _, name := range []string{"corpus", "corpus_json"} {
		if r, ok := existing[name]; ok && r.Kind() == corpusKind {
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
	bindBuildifier := rootBuildifierConfig(args.Config.RepoRoot)
	bindBiome := rootBiomeConfig(args.Config.RepoRoot)
	emit := func(typ, attr string, srcs []string) {
		if len(srcs) == 0 {
			return
		}
		r := rule.NewRule(corpusKind, "corpus_"+typ)
		r.SetAttr(attr, srcs)
		r.SetAttr("tags", []string{corpusTag})
		if typ == "markdown" {
			r.SetAttr("aspect_hints", []string{corpusValeHint})
			if len(siblings) > 0 {
				r.SetAttr("markdown_siblings", siblings)
			}
		} else if typ == "starlark" && bindBuildifier {
			r.SetAttr("aspect_hints", []string{corpusBuildifierHint})
		} else if typ == "json" && bindBiome {
			r.SetAttr("aspect_hints", []string{corpusBiomeHint})
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
	if _, ok := existing["corpus"]; ok {
		if r, ok := existing["corpus"]; ok && r.Kind() == corpusKind {
			plan.empty = append(plan.empty, rule.NewRule(corpusKind, "corpus"))
		}
	}
	return plan
}
