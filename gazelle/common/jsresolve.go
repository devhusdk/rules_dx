package common

import (
	"strings"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

// JSLookup is the result of FindJSImport: either an exact override label or the
// rule index matches, plus the candidate name that produced them.
type JSLookup struct {
	Override    label.Label
	HasOverride bool
	Matches     []resolve.FindResult
	Matched     string
}

// FindJSImport looks up a JavaScript or TypeScript import name. A local
// (relative) name tries name, then name + "/index", each as override then rule
// index. A bare name tries an exact override on the name first, then each
// candidate from the nearest tsconfig/jsconfig paths and baseUrl (see
// ConfigureTSPaths) as override, index, then <candidate>/index, and last the
// rule index for the name. New candidate sources belong here so the language
// Resolve functions stay unchanged.
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

// aliasIdentity turns a workspace-relative alias target into the module
// identity languages index, stripping a module or declaration extension.
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

// IsAssetSource reports whether a srcs entry is a plain file name rather than a
// label.
func IsAssetSource(src string) bool {
	if src == "" || strings.HasPrefix(src, ":") || strings.HasPrefix(src, "//") || strings.HasPrefix(src, "@") {
		return false
	}
	return true
}
