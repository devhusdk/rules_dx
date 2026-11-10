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
	Bindings    []IDLBinding
}

// IDLBinding is a code generation rule wrapping the library of a schema
// language. It is generated when the root MODULE.bazel depends on Module.
type IDLBinding struct {
	Kind   string
	Infix  string
	Module string
	// Attr is the attribute that names the library: "deps" (a list) or
	// "library" (one label).
	Attr string
	// OneSchema leaves the binding out for libraries with other than one schema.
	OneSchema bool
}

// IDLBindings returns the cc, java and rust bindings of a schema language
// whose rules are named <prefix>_cc_library, <prefix>_java_library and
// <prefix>_rust_library. An empty rustAttr leaves out the rust binding.
// oneSchema limits the rust binding to libraries with a single schema.
func IDLBindings(prefix, rustAttr string, oneSchema bool) []IDLBinding {
	all := []IDLBinding{
		{prefix + "_cc_library", "_cc", "rules_cc", "deps", false},
		{prefix + "_java_library", "_java", "rules_java", "deps", false},
		{prefix + "_rust_library", "_rust", "rules_rust", rustAttr, oneSchema},
	}
	var out []IDLBinding
	for _, b := range all {
		if rustAttr == "" && b.Infix == "_rust" {
			continue
		}
		out = append(out, b)
	}
	return out
}

func idlBindingKinds(spec IDLSpec) map[string]rule.KindInfo {
	kinds := map[string]rule.KindInfo{spec.Kind: LibraryKindInfo()}
	for _, b := range spec.Bindings {
		kinds[b.Kind] = rule.KindInfo{
			MatchAttrs:     []string{b.Attr},
			NonEmptyAttrs:  map[string]bool{b.Attr: true},
			MergeableAttrs: map[string]bool{b.Attr: true, "visibility": true, "testonly": true},
		}
	}
	return kinds
}

// addIDLBindings appends the binding rules of each generated library and
// empties the ones whose library is gone.
func addIDLBindings(spec IDLSpec, args language.GenerateArgs, result language.GenerateResult) language.GenerateResult {
	deps := ModuleDeps(args.Config.RepoRoot)
	var active []IDLBinding
	for _, b := range spec.Bindings {
		if deps[b.Module] {
			active = append(active, b)
		}
	}
	if len(active) == 0 {
		return result
	}
	names := make(map[string]bool)
	for _, lib := range result.Gen {
		names[lib.Name()] = true
	}
	for _, lib := range result.Empty {
		names[lib.Name()] = true
	}
	owned := make(map[string]bool)
	for name := range names {
		for _, b := range active {
			owned[b.Kind+"\x00"+name+b.Infix] = true
		}
	}
	generated := make(map[string]bool)
	for _, lib := range append([]*rule.Rule{}, result.Gen...) {
		if lib.Kind() != spec.Kind {
			continue
		}
		for _, b := range active {
			if b.OneSchema && len(lib.AttrStrings("srcs")) != 1 {
				continue
			}
			r := rule.NewRule(b.Kind, lib.Name()+b.Infix)
			if b.Attr == "library" {
				r.SetAttr("library", ":"+lib.Name())
			} else {
				r.SetAttr("deps", []string{":" + lib.Name()})
			}
			if lib.AttrBool("testonly") {
				r.SetAttr("testonly", true)
			}
			r.SetAttr("visibility", []string{"//visibility:public"})
			result.Gen = append(result.Gen, r)
			result.Imports = append(result.Imports, nil)
			generated[b.Kind+"\x00"+r.Name()] = true
		}
	}
	if args.File == nil {
		return result
	}
	for _, existing := range args.File.Rules {
		key := existing.Kind() + "\x00" + existing.Name()
		if owned[key] && !generated[key] {
			result.Empty = append(result.Empty, rule.NewRule(existing.Kind(), existing.Name()))
		}
	}
	return result
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
		Kinds:          idlBindingKinds(spec),
		ExtraKinds:     bindingKindNames(spec),
		DefsPackage:    spec.Defs,
		KindDefs:       bindingKindDefs(spec),
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
			result := GenerateSingleDir(args, map[string]rule.KindInfo{spec.Kind: kinds[spec.Kind]}, SingleDirSpec{
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
			return addIDLBindings(spec, args, result)
		},
	})
}

func bindingKindNames(spec IDLSpec) []string {
	var kinds []string
	for _, b := range spec.Bindings {
		kinds = append(kinds, b.Kind)
	}
	return kinds
}

func bindingKindDefs(spec IDLSpec) map[string]string {
	defs := make(map[string]string)
	for _, b := range spec.Bindings {
		defs[b.Kind] = spec.Defs
	}
	return defs
}
