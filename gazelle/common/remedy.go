package common

import (
	"fmt"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazel-contrib/bazel-gazelle/v2/label"
)

func BuildFilePath(c *config.Config, pkg string) string {
	name := "BUILD.bazel"
	if c != nil && len(c.ValidBuildFileNames) > 0 {
		name = c.ValidBuildFileNames[0]
	}
	return manifestPath(pkg, name)
}

func UnresolvedImport(c *config.Config, lang string, from label.Label, name, hint string) string {
	return UnresolvedImportError(c, lang, from, name, hint).Message
}

func UnresolvedImportError(c *config.Config, lang string, from label.Label, name, hint string) GenerationError {
	file := BuildFilePath(c, from.Pkg)
	directive := fmt.Sprintf("# gazelle:dx_ignore_import %s %s", lang, name)
	return GenerationError{
		Language: lang,
		Package:  InPackage(from.Pkg),
		Message: fmt.Sprintf("%s: %s: unresolved import %q; %s; or add \"%s\" to %s",
			lang, from, name, hint, directive, file),
		Kind:   KindUnresolvedImport,
		Import: name,
		Fix:    Fix{File: file, Directive: directive},
	}
}

func AmbiguousImport(c *config.Config, lang string, from label.Label, name, matches string) string {
	return AmbiguousImportError(c, lang, from, name, matches).Message
}

func AmbiguousImportError(c *config.Config, lang string, from label.Label, name, matches string) GenerationError {
	file := BuildFilePath(c, from.Pkg)
	directive := fmt.Sprintf("# gazelle:resolve %s %s <label>", lang, name)
	return GenerationError{
		Language: lang,
		Package:  InPackage(from.Pkg),
		Message: fmt.Sprintf("%s: %s: ambiguous import %q resolves to %s; add \"%s\" to %s",
			lang, from, name, matches, directive, file),
		Kind:   KindAmbiguousImport,
		Import: name,
		Fix:    Fix{File: file, Directive: directive},
	}
}

func ClaimError(name, kind, source, have, buildFile string) string {
	if buildFile == "" {
		return fmt.Sprintf("target name %q is claimed by generated %s(%s) and existing %s; rename one of the sources that generate it",
			name, kind, source, have)
	}
	return fmt.Sprintf("target name %q is claimed by generated %s(%s) and existing %s; rename or delete existing %s %q in %s, or change it to %s",
		name, kind, source, have, have, name, buildFile, kind)
}
