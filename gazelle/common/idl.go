package common

import (
	"path"
	"regexp"
	"sort"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/rule"
)

// IDLSpec describes a schema language whose files include or import each
// other by path and whose library groups one directory.
type IDLSpec struct {
	Name        string
	Kind        string
	Defs        string
	Exts        []string
	StdPrefix   string
	AbsoluteDep bool
}

var (
	slashComments = regexp.MustCompile(`(?s)/\*.*?\*/|//[^\n]*`)
	hashComments  = regexp.MustCompile(`(?s)/\*.*?\*/|//[^\n]*|#[^\n]*`)
)

// ParseIDLStatements returns the sorted, deduplicated captures of every match
// of statement in content, with comments removed first. hash also removes
// '#' comments.
func ParseIDLStatements(content []byte, statement *regexp.Regexp, hash bool) []string {
	comments := slashComments
	if hash {
		comments = hashComments
	}
	text := comments.ReplaceAll(content, nil)
	set := make(map[string]bool)
	for _, m := range statement.FindAllSubmatch(text, -1) {
		for _, group := range m[1:] {
			if len(group) > 0 {
				set[string(group)] = true
			}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// NewIDLLanguage builds a language that generates one library per directory
// holding schema files. An include resolves to the library owning the path
// next to the including file, or from the repository root.
func NewIDLLanguage(spec IDLSpec, parse func([]byte) []string, hint string) language.Language {
	isSource := func(name string) bool { return HasExt(name, spec.Exts) }
	return NewSingleLang(SingleLangSpec{
		LanguageName:   spec.Name,
		LibraryKind:    spec.Kind,
		Kinds:          map[string]rule.KindInfo{spec.Kind: LibraryKindInfo()},
		DefsPackage:    spec.Defs,
		ImportAttr:     "srcs",
		Identity:       func(name string) string { return name },
		PathIdentity:   true,
		IsImportSource: isSource,
		IsStdLib: func(name string) bool {
			return spec.StdPrefix != "" && strings.HasPrefix(name, spec.StdPrefix)
		},
		Lookups: func(pkg, name string) []string {
			if spec.AbsoluteDep && strings.HasPrefix(name, "/") {
				return []string{path.Clean(strings.TrimPrefix(name, "/"))}
			}
			return []string{path.Join(pkg, name), path.Clean(name)}
		},
		UnresolvedHint: hint,
		Generate: func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult {
			return GenerateSingleDir(args, map[string]rule.KindInfo{spec.Kind: kinds[spec.Kind]}, SingleDirSpec{
				LanguageName: spec.Name,
				LibraryKind:  spec.Kind,
				IsSource:     isSource,
				IsMainFile:   func(string) bool { return false },
				DirName:      DirTargetName,
				ParseImports: parse,
				MakeRule: func(name string, srcs []string, _ []string) *rule.Rule {
					r := rule.NewRule(spec.Kind, name)
					r.SetAttr("srcs", srcs)
					return r
				},
				Wrap: WrapImportSet,
			}, rep)
		},
	})
}
