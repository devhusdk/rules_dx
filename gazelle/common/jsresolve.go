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

// FindJSImport looks up a JavaScript or TypeScript import name. It tries the
// exact name first (override, then rule index). Only for a local (relative)
// import it then tries name + "/index". New candidate sources belong here so
// the language Resolve functions stay unchanged.
func FindJSImport(c *config.Config, ix *resolve.RuleIndex, lang, name string, local bool) JSLookup {
	candidates := []string{name}
	if local {
		candidates = append(candidates, name+"/index")
	}
	for _, cand := range candidates {
		spec := resolve.ImportSpec{Lang: lang, Imp: cand}
		if override, found := resolve.FindRuleWithOverride(c, spec, lang); found {
			return JSLookup{Override: override, HasOverride: true, Matched: cand}
		}
		if matches := ix.FindRulesByImportWithConfig(c, spec, lang); len(matches) > 0 {
			return JSLookup{Matches: matches, Matched: cand}
		}
	}
	return JSLookup{Matched: name}
}

// IsAssetSource reports whether a srcs entry is a plain file name rather than a
// label.
func IsAssetSource(src string) bool {
	if src == "" || strings.HasPrefix(src, ":") || strings.HasPrefix(src, "//") || strings.HasPrefix(src, "@") {
		return false
	}
	return true
}
