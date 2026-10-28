package common

import (
	"fmt"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

// BuildFilePath returns the workspace-relative BUILD file path gazelle uses for pkg.
func BuildFilePath(c *config.Config, pkg string) string {
	name := "BUILD.bazel"
	if c != nil && len(c.ValidBuildFileNames) > 0 {
		name = c.ValidBuildFileNames[0]
	}
	return manifestPath(pkg, name)
}

// UnresolvedImport formats an unresolved import error with its hint and the ignore directive remedy.
func UnresolvedImport(c *config.Config, lang string, from label.Label, name, hint string) string {
	return fmt.Sprintf("%s: %s: unresolved import %q; %s; or add \"# gazelle:dx_ignore_import %s %s\" to %s",
		lang, from, name, hint, lang, name, BuildFilePath(c, from.Pkg))
}

// AmbiguousImport formats an ambiguous import error with the resolve directive remedy.
func AmbiguousImport(c *config.Config, lang string, from label.Label, name, matches string) string {
	return fmt.Sprintf("%s: %s: ambiguous import %q resolves to %s; add \"# gazelle:resolve %s %s <label>\" to %s",
		lang, from, name, matches, lang, name, BuildFilePath(c, from.Pkg))
}

// ClaimError formats a claim conflict with a remedy. buildFile is the workspace-relative BUILD file holding the
// handwritten rule have; it is empty when have comes from another generator's output, which the user cannot edit.
func ClaimError(name, kind, source, have, buildFile string) string {
	if buildFile == "" {
		return fmt.Sprintf("target name %q is claimed by generated %s(%s) and existing %s; rename one of the sources that generate it",
			name, kind, source, have)
	}
	return fmt.Sprintf("target name %q is claimed by generated %s(%s) and existing %s; rename or delete existing %s %q in %s, or change it to %s",
		name, kind, source, have, have, name, buildFile, kind)
}
