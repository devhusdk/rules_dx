package common

import (
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

type JSLookup struct {
	Override    label.Label
	HasOverride bool
	Matches     []resolve.FindResult
	Matched     string
}

func FindJSImport(c *config.Config, ix *resolve.RuleIndex, lang, name string, local bool) JSLookup {
	try := func(cand string) (JSLookup, bool) {
		spec := resolve.ImportSpec{Lang: lang, Imp: cand}
		if override, found := resolve.FindRuleWithOverride(c, spec, lang); found {
			return JSLookup{Override: override, HasOverride: true, Matched: cand}, true
		}
		if matches := ix.FindRulesByImportWithConfig(c, spec, lang); len(matches) > 0 {
			return JSLookup{Matches: matches, Matched: cand}, true
		}
		return JSLookup{}, false
	}
	if local {
		for _, cand := range []string{name, name + "/index"} {
			if found, ok := try(cand); ok {
				return found
			}
		}
		return JSLookup{Matched: name}
	}
	if override, found := resolve.FindRuleWithOverride(c, resolve.ImportSpec{Lang: lang, Imp: name}, lang); found {
		return JSLookup{Override: override, HasOverride: true, Matched: name}
	}
	for _, cand := range tsPathsFor(c).Candidates(name) {
		id := aliasIdentity(cand)
		if id == "" {
			continue
		}
		for _, imp := range []string{id, id + "/index"} {
			if found, ok := try(imp); ok {
				return found
			}
		}
	}
	if found, ok := try(name); ok {
		return found
	}
	return JSLookup{Matched: name}
}

var declarationSuffixes = []string{".d.ts", ".d.mts", ".d.cts"}

func aliasIdentity(cand string) string {
	for _, suffix := range declarationSuffixes {
		if trimmed, ok := strings.CutSuffix(cand, suffix); ok {
			cand = trimmed
			break
		}
	}
	root, _, _ := relativeRef("", "/"+cand, JSModuleExts)
	return root
}

func IsAssetSource(src string) bool {
	if src == "" || strings.HasPrefix(src, ":") || strings.HasPrefix(src, "//") || strings.HasPrefix(src, "@") {
		return false
	}
	return true
}

func JSFind(lang string) func(c *config.Config, ix *resolve.RuleIndex, ref ImportRef) JSLookup {
	return func(c *config.Config, ix *resolve.RuleIndex, ref ImportRef) JSLookup {
		return FindJSImport(c, ix, lang, ref.Root, ref.Relative)
	}
}

func JSImportRefs(refs []ImportRef) []ImportRef {
	var out []ImportRef
	index := make(map[string]int)
	for _, ref := range refs {
		if i, ok := index[ref.Root]; ok {
			out[i].Relative = out[i].Relative || ref.Relative
			if out[i].Ext == "" {
				out[i].Ext = ref.Ext
			}
			continue
		}
		if !ref.Relative && IsNodeBuiltin(ref.Root) {
			continue
		}
		index[ref.Root] = len(out)
		out = append(out, ref)
	}
	SortRefs(out)
	return out
}

func ConfigureTSPathsExt(c *config.Config, rel string, _ *rule.File, rep Reporter) {
	if err := ConfigureTSPaths(c, rel); err != nil {
		rep.Fail(InPackage(rel), "%v", err)
	}
}
